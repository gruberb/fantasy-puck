//! `/api/race-odds` — season-odds Monte Carlo for the fantasy race.
//!
//! Two modes:
//! - **League** (when `league_id` is supplied): per-fantasy-team win / top-3
//!   probabilities and projected-final distributions, with an optional
//!   head-to-head "rivalry card" against the caller's closest rival.
//! - **Champion** (no `league_id`): top NHL skaters by projected playoff
//!   fantasy points — the global Fantasy Champion leaderboard.
//!
//! The handler is thin: extract → fetch signals → build sim input → run
//! simulation on a blocking thread → cache → return. All heavy math lives
//! in [`crate::utils::race_sim`], which is a pure-domain module.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use axum::{
    extract::{Query, State},
    Json,
};
use chrono::Utc;
use tracing::debug;

use crate::api::dtos::race_odds::{
    RaceOddsMode, RaceOddsResponse, RivalryCard,
};
use crate::api::handlers::insights::hockey_today;
use crate::api::response::{json_success, ApiResponse};
use crate::api::routes::AppState;
use crate::api::{game_type, season};
use crate::error::Result;
use crate::models::fantasy::FantasyTeamInGame;
use crate::models::nhl::{PlayoffCarousel, StatsLeaders};
use crate::utils::player_projection::{project_players, PlayerInput, Projection};
use crate::utils::playoff_elo::compute_current_elo;
use crate::utils::race_sim::{
    simulate, BracketState, RaceSimInput, RaceSimOutput, SeriesState, SimFantasyTeam, SimPlayer,
    TeamOdds, TeamRating, DEFAULT_K_FACTOR, DEFAULT_PPG, DEFAULT_TRIALS,
};

/// Logistic scale applied when the input ratings are on the Elo scale
/// (~1500-centered, ~±300 spread). Matches the standard Elo formula
/// `p = 1 / (1 + 10^(-Δ/400))` via the identity
/// `sigmoid(x · ln10 / 400) == 1 / (1 + 10^(-x/400))`. The older
/// standings-points path still uses `DEFAULT_K_FACTOR` because its
/// ratings are ~40-point-spread; using 0.010 against Elo would pin
/// every series outcome to the favorite.
const ELO_K_FACTOR: f32 = std::f32::consts::LN_10 / 400.0;

// ---------------------------------------------------------------------------
// Public surface
// ---------------------------------------------------------------------------

/// Query parameters understood by `GET /api/race-odds`.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RaceOddsParams {
    #[serde(default)]
    pub league_id: Option<String>,
    /// Caller's fantasy team id — enables the rivalry card when in League mode.
    #[serde(default)]
    pub my_team_id: Option<i64>,
}

pub async fn get_race_odds(
    State(state): State<Arc<AppState>>,
    Query(params): Query<RaceOddsParams>,
) -> Result<Json<ApiResponse<RaceOddsResponse>>> {
    let league_id = params
        .league_id
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("");
    let response = generate_and_cache_race_odds(&state, league_id, params.my_team_id).await?;
    Ok(json_success(response))
}

/// Build-or-cache entry point, also used by the scheduler pre-warm.
pub async fn generate_and_cache_race_odds(
    state: &Arc<AppState>,
    league_id: &str,
    my_team_id: Option<i64>,
) -> Result<RaceOddsResponse> {
    let today = hockey_today();
    // Model version in the cache key: v2 reflects the P0+P1+P2 rework
    // (bracket-state-aware sim, dynamic playoff Elo, Bayesian player
    // projection). Bump when the projection model changes so a deploy
    // doesn't serve stale same-day odds under the old model.
    let cache_key = format!(
        "race_odds:v2:{}:{}:{}:{}",
        if league_id.is_empty() { "global" } else { league_id },
        season(),
        game_type(),
        today
    );

    if let Some(cached) = state
        .db
        .cache()
        .get_cached_response::<RaceOddsResponse>(&cache_key)
        .await?
    {
        // Rivalry depends on the caller — recompute it against the cached
        // league standings rather than serving a stale "me vs X" card.
        return Ok(attach_rivalry(cached, my_team_id));
    }

    let response = build_response(state, league_id, my_team_id).await?;

    let _ = state
        .db
        .cache()
        .store_response(&cache_key, &today, &clone_without_rivalry(&response))
        .await;

    Ok(response)
}

// ---------------------------------------------------------------------------
// Orchestrator
// ---------------------------------------------------------------------------

async fn build_response(
    state: &Arc<AppState>,
    league_id: &str,
    my_team_id: Option<i64>,
) -> Result<RaceOddsResponse> {
    let input = if league_id.is_empty() {
        build_champion_input(state).await?
    } else {
        build_league_input(state, league_id).await?
    };

    debug!(
        league_id = %league_id,
        teams = input.fantasy_teams.len(),
        bracket_depth = input.bracket.depth(),
        "running race-odds simulation"
    );

    // Monte Carlo is CPU-heavy (tens of ms for default trials × roster size).
    // Keep it off the async runtime.
    let trials = DEFAULT_TRIALS;
    let sim_input = input.clone();
    let output: RaceSimOutput = tokio::task::spawn_blocking(move || simulate(&sim_input, trials))
        .await
        .map_err(|e| crate::Error::Internal(format!("race sim join error: {}", e)))?;

    let mode = if league_id.is_empty() {
        RaceOddsMode::Champion
    } else {
        RaceOddsMode::League
    };

    // Champion mode: rank players by projected mean, keep the top 20.
    let mut champion_leaderboard = if mode == RaceOddsMode::Champion {
        let mut players = output.players.clone();
        players.sort_by(|a, b| {
            b.projected_final_mean
                .partial_cmp(&a.projected_final_mean)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        players.truncate(20);
        players
    } else {
        Vec::new()
    };
    // Guard against Champion mode returning nothing (e.g. API empty) by
    // leaving the vec empty rather than returning garbage.
    if mode != RaceOddsMode::Champion {
        champion_leaderboard.clear();
    }

    let team_odds = if mode == RaceOddsMode::League {
        let mut teams = output.teams.clone();
        teams.sort_by(|a, b| {
            b.win_prob
                .partial_cmp(&a.win_prob)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        teams
    } else {
        Vec::new()
    };

    let response = RaceOddsResponse {
        generated_at: Utc::now().to_rfc3339(),
        mode,
        trials,
        k_factor: input.k_factor,
        team_odds,
        champion_leaderboard,
        nhl_teams: {
            // Sort by cup_win_prob descending so frontends can take(N) without
            // re-sorting. Ties broken alphabetically by abbrev for stability.
            let mut v = output.nhl_teams.clone();
            v.sort_by(|a, b| {
                b.cup_win_prob
                    .partial_cmp(&a.cup_win_prob)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.abbrev.cmp(&b.abbrev))
            });
            v
        },
        rivalry: None, // filled in by attach_rivalry below
    };

    Ok(attach_rivalry(response, my_team_id))
}

// ---------------------------------------------------------------------------
// Input builders
// ---------------------------------------------------------------------------

async fn build_league_input(
    state: &Arc<AppState>,
    league_id: &str,
) -> Result<RaceSimInput> {
    // Bind season to a local so the reference we hand to `get_skater_stats`
    // outlives the `tokio::join!`-produced future. The functions `season()`
    // and `game_type()` return primitives by value, so the `&season()` form
    // used elsewhere only works in synchronous contexts.
    let season_val = season();
    let game_type_val = game_type();
    let (teams_res, carousel_res, playoff_stats_res, regular_stats_res, standings_res) = tokio::join!(
        state.db.get_all_teams_with_players(league_id),
        state.nhl_client.get_playoff_carousel(season_val.to_string()),
        state.nhl_client.get_skater_stats(&season_val, game_type_val),
        state.nhl_client.get_skater_stats(&season_val, 2),
        state.nhl_client.get_standings_raw(),
    );

    let teams = teams_res?;
    let carousel = carousel_res.ok().flatten();
    let playoff_stats = playoff_stats_res.ok();
    let regular_stats = regular_stats_res.ok();
    let standings_json = standings_res.ok();

    let bracket = bracket_from_carousel(carousel.as_ref());
    // Still needed by the pre-playoff `player_ppg` path and as the team-
    // games-played input for player_projection. Has nothing to do with
    // the sim's remaining-games logic, which derives from the bracket.
    let games_played_so_far = games_played_from_carousel(carousel.as_ref());

    let is_playoffs = game_type_val == 3;
    let (ratings, k_factor) = resolve_ratings(
        &state.db,
        season_val,
        is_playoffs,
        standings_json.as_ref(),
    )
    .await;

    let fantasy_teams = if is_playoffs {
        build_fantasy_teams_playoff(
            &state.db,
            season_val,
            teams,
            playoff_stats.as_ref(),
            regular_stats.as_ref(),
            &games_played_so_far,
        )
        .await?
    } else {
        teams
            .into_iter()
            .map(|t| {
                fantasy_team_to_sim(
                    t,
                    playoff_stats.as_ref(),
                    regular_stats.as_ref(),
                    &games_played_so_far,
                )
            })
            .collect()
    };

    Ok(RaceSimInput {
        bracket,
        ratings,
        k_factor,
        fantasy_teams,
    })
}

/// Compute the ratings map + matching logistic k-factor. Playoff path
/// uses the game-log-driven Elo; pre-playoff falls back to the blended
/// standings rating that the codebase has always used.
async fn resolve_ratings(
    db: &crate::db::FantasyDb,
    season_val: u32,
    is_playoffs: bool,
    standings_json: Option<&serde_json::Value>,
) -> (HashMap<String, TeamRating>, f32) {
    if is_playoffs {
        if let Some(standings) = standings_json {
            match compute_current_elo(db, standings, season_val).await {
                Ok(elo) => {
                    let map: HashMap<String, TeamRating> =
                        elo.into_iter().map(|(k, v)| (k, TeamRating(v))).collect();
                    return (map, ELO_K_FACTOR);
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "playoff Elo fetch failed; falling back to standings blend"
                    );
                }
            }
        } else {
            tracing::warn!("standings fetch empty; cannot compute playoff Elo");
        }
    }
    (ratings_from_standings(standings_json), DEFAULT_K_FACTOR)
}

/// Build SimFantasyTeams using the Bayesian `player_projection` path.
/// Batches all rostered players into one DB round-trip and assembles the
/// team structures off the returned projection map.
async fn build_fantasy_teams_playoff(
    db: &crate::db::FantasyDb,
    season_val: u32,
    teams: Vec<FantasyTeamInGame>,
    playoff_stats: Option<&StatsLeaders>,
    regular_stats: Option<&StatsLeaders>,
    games_played_so_far: &HashMap<String, u32>,
) -> Result<Vec<SimFantasyTeam>> {
    // Flatten rostered players across all fantasy teams into one input
    // list, deduped by nhl_id. Goalies are skipped here to match the
    // pre-existing pipeline.
    let mut inputs: Vec<PlayerInput> = Vec::new();
    let mut seen: HashSet<i64> = HashSet::new();
    for team in &teams {
        for p in &team.players {
            if p.position.eq_ignore_ascii_case("G") {
                continue;
            }
            if !seen.insert(p.nhl_id) {
                continue;
            }
            let rs_points = regular_stats
                .and_then(|s| s.points.iter().find(|x| x.id as i64 == p.nhl_id))
                .map(|x| x.value as i32)
                .unwrap_or(0);
            inputs.push(PlayerInput {
                nhl_id: p.nhl_id,
                player_name: p.player_name.clone(),
                nhl_team: p.nhl_team.clone(),
                rs_points,
            });
        }
    }

    let projections: HashMap<i64, Projection> =
        project_players(db, season_val, &inputs, games_played_so_far).await?;

    Ok(teams
        .into_iter()
        .map(|team| {
            let mut local_seen: HashSet<i64> = HashSet::new();
            let players = team
                .players
                .into_iter()
                .filter(|p| local_seen.insert(p.nhl_id))
                .filter(|p| !p.position.eq_ignore_ascii_case("G"))
                .map(|p| {
                    let ppg = projections
                        .get(&p.nhl_id)
                        .map(|proj| proj.ppg)
                        .unwrap_or(DEFAULT_PPG);
                    SimPlayer {
                        nhl_id: p.nhl_id,
                        name: p.player_name.clone(),
                        nhl_team: p.nhl_team.clone(),
                        position: p.position.clone(),
                        playoff_points_so_far: playoff_points_for(playoff_stats, p.nhl_id),
                        ppg,
                        image_url: None,
                    }
                })
                .collect();
            SimFantasyTeam {
                team_id: team.team_id,
                team_name: team.team_name,
                players,
            }
        })
        .collect())
}

async fn build_champion_input(state: &Arc<AppState>) -> Result<RaceSimInput> {
    let season_val = season();
    let game_type_val = game_type();
    let (carousel_res, playoff_stats_res, regular_stats_res, standings_res) = tokio::join!(
        state.nhl_client.get_playoff_carousel(season_val.to_string()),
        state.nhl_client.get_skater_stats(&season_val, game_type_val),
        state.nhl_client.get_skater_stats(&season_val, 2),
        state.nhl_client.get_standings_raw(),
    );

    let carousel = carousel_res.ok().flatten();
    let playoff_stats = playoff_stats_res.ok();
    let regular_stats = regular_stats_res.ok();
    let standings_json = standings_res.ok();

    let bracket = bracket_from_carousel(carousel.as_ref());
    let games_played_so_far = games_played_from_carousel(carousel.as_ref());

    let is_playoffs = game_type_val == 3;
    let (ratings, k_factor) =
        resolve_ratings(&state.db, season_val, is_playoffs, standings_json.as_ref()).await;

    // Build a flat Fantasy Champion pool: top 40 regular-season skaters by
    // points. Treat each as its own one-player "team" so the simulator's
    // per-team outputs map one-to-one to players.
    let Some(regular) = regular_stats.as_ref() else {
        return Ok(RaceSimInput {
            bracket,
            ratings,
            k_factor,
            fantasy_teams: Vec::new(),
        });
    };

    // Skip goalies — this app drafts skaters only.
    let mut leaders: Vec<_> = regular
        .points
        .iter()
        .filter(|p| !p.position.eq_ignore_ascii_case("G"))
        .collect();
    leaders.sort_by(|a, b| b.value.partial_cmp(&a.value).unwrap_or(std::cmp::Ordering::Equal));
    leaders.truncate(40);

    // Playoff path: batch-project all 40 leaders through the Bayesian
    // blend. Off-playoff path retains the legacy `player_ppg` fallback.
    let projections: HashMap<i64, Projection> = if is_playoffs {
        let inputs: Vec<PlayerInput> = leaders
            .iter()
            .map(|p| PlayerInput {
                nhl_id: p.id as i64,
                player_name: format!(
                    "{} {}",
                    p.first_name.get("default").cloned().unwrap_or_default(),
                    p.last_name.get("default").cloned().unwrap_or_default(),
                ),
                nhl_team: p.team_abbrev.clone(),
                rs_points: p.value as i32,
            })
            .collect();
        project_players(&state.db, season_val, &inputs, &games_played_so_far).await?
    } else {
        HashMap::new()
    };

    let fantasy_teams = leaders
        .into_iter()
        .map(|p| {
            let nhl_id = p.id as i64;
            let name = format!(
                "{} {}",
                p.first_name.get("default").cloned().unwrap_or_default(),
                p.last_name.get("default").cloned().unwrap_or_default(),
            );
            let playoff_points_so_far = playoff_points_for(playoff_stats.as_ref(), nhl_id);
            let ppg = if is_playoffs {
                projections
                    .get(&nhl_id)
                    .map(|proj| proj.ppg)
                    .unwrap_or(DEFAULT_PPG)
            } else {
                player_ppg(
                    nhl_id,
                    &p.team_abbrev,
                    playoff_stats.as_ref(),
                    regular_stats.as_ref(),
                    &games_played_so_far,
                )
            };
            let sim_player = SimPlayer {
                nhl_id,
                name: name.clone(),
                nhl_team: p.team_abbrev.clone(),
                position: p.position.clone(),
                playoff_points_so_far,
                ppg,
                image_url: Some(state.nhl_client.get_player_image_url(nhl_id)),
            };
            SimFantasyTeam {
                team_id: nhl_id,
                team_name: name,
                players: vec![sim_player],
            }
        })
        .collect();

    Ok(RaceSimInput {
        bracket,
        ratings,
        k_factor,
        fantasy_teams,
    })
}

// ---------------------------------------------------------------------------
// Carousel / roster helpers
// ---------------------------------------------------------------------------

/// Build the full playoff bracket from the NHL `/playoff-series/carousel`
/// feed. Every round is represented, padded with `SeriesState::Future` when
/// the carousel hasn't populated a slot yet.
///
/// Classification per series:
/// - `top_wins == 4` or `bottom_wins == 4` → `Completed` (winner fixed).
/// - Both teams present → `InProgress` (may be 0-0 if the series hasn't
///   started but the opponents are seeded, or live with real wins).
/// - Missing entirely → `Future`.
///
/// Slot alignment: the carousel lists each round's series in bracket order
/// (A-H for R1, AB/CD/EF/GH-style concatenations for R2, etc.) — we trust
/// that order and pad from the right with `Future`.
fn bracket_from_carousel(carousel: Option<&PlayoffCarousel>) -> BracketState {
    const SLOT_COUNTS: [usize; 4] = [8, 4, 2, 1];
    let Some(c) = carousel else {
        return BracketState::default();
    };

    let mut rounds: Vec<Vec<SeriesState>> = Vec::with_capacity(SLOT_COUNTS.len());
    for (idx, &slot_count) in SLOT_COUNTS.iter().enumerate() {
        let round_number = (idx + 1) as i64;
        let carousel_round = c.rounds.iter().find(|r| r.round_number == round_number);
        let mut slots: Vec<SeriesState> = Vec::with_capacity(slot_count);
        if let Some(round) = carousel_round {
            for s in &round.series {
                let top_wins = s.top_seed.wins.max(0) as u32;
                let bottom_wins = s.bottom_seed.wins.max(0) as u32;
                let state = if top_wins >= 4 {
                    SeriesState::Completed {
                        winner: s.top_seed.abbrev.clone(),
                        loser: s.bottom_seed.abbrev.clone(),
                        total_games: top_wins + bottom_wins,
                    }
                } else if bottom_wins >= 4 {
                    SeriesState::Completed {
                        winner: s.bottom_seed.abbrev.clone(),
                        loser: s.top_seed.abbrev.clone(),
                        total_games: top_wins + bottom_wins,
                    }
                } else if s.top_seed.abbrev.is_empty() || s.bottom_seed.abbrev.is_empty() {
                    // Carousel has the slot but hasn't populated teams yet.
                    SeriesState::Future
                } else {
                    SeriesState::InProgress {
                        top_team: s.top_seed.abbrev.clone(),
                        top_wins,
                        bottom_team: s.bottom_seed.abbrev.clone(),
                        bottom_wins,
                    }
                };
                slots.push(state);
                if slots.len() >= slot_count {
                    break;
                }
            }
        }
        while slots.len() < slot_count {
            slots.push(SeriesState::Future);
        }
        rounds.push(slots);
    }
    BracketState { rounds }
}

fn games_played_from_carousel(carousel: Option<&PlayoffCarousel>) -> HashMap<String, u32> {
    let mut map = HashMap::new();
    let Some(c) = carousel else {
        return map;
    };
    for round in &c.rounds {
        for s in &round.series {
            let games = (s.top_seed.wins + s.bottom_seed.wins).max(0) as u32;
            // Each team in a series has played the same number of games.
            *map.entry(s.top_seed.abbrev.clone()).or_insert(0) += games;
            *map.entry(s.bottom_seed.abbrev.clone()).or_insert(0) += games;
        }
    }
    map
}

fn ratings_from_standings(standings: Option<&serde_json::Value>) -> HashMap<String, TeamRating> {
    let Some(root) = standings else {
        return HashMap::new();
    };
    // Shared helper blends season points with L10 form so race-sim and
    // Insights report the same strength numbers.
    crate::utils::team_ratings::from_standings(root)
        .into_iter()
        .map(|(abbrev, rating)| (abbrev, TeamRating(rating)))
        .collect()
}

fn fantasy_team_to_sim(
    team: FantasyTeamInGame,
    playoff_stats: Option<&StatsLeaders>,
    regular_stats: Option<&StatsLeaders>,
    games_played_so_far: &HashMap<String, u32>,
) -> SimFantasyTeam {
    // Dedupe by nhl_id in case a team lists the same player twice.
    let mut seen: HashSet<i64> = HashSet::new();
    let players = team
        .players
        .into_iter()
        .filter(|p| seen.insert(p.nhl_id))
        .filter(|p| !p.position.eq_ignore_ascii_case("G"))
        .map(|p| SimPlayer {
            nhl_id: p.nhl_id,
            name: p.player_name.clone(),
            nhl_team: p.nhl_team.clone(),
            position: p.position.clone(),
            playoff_points_so_far: playoff_points_for(playoff_stats, p.nhl_id),
            ppg: player_ppg(
                p.nhl_id,
                &p.nhl_team,
                playoff_stats,
                regular_stats,
                games_played_so_far,
            ),
            image_url: None,
        })
        .collect();

    SimFantasyTeam {
        team_id: team.team_id,
        team_name: team.team_name,
        players,
    }
}

fn playoff_points_for(stats: Option<&StatsLeaders>, nhl_id: i64) -> i32 {
    let Some(s) = stats else { return 0 };
    s.points
        .iter()
        .find(|p| p.id as i64 == nhl_id)
        .map(|p| p.value as i32)
        .unwrap_or(0)
}

/// Estimate fantasy points-per-game for a skater.
///
/// Priority:
/// 1. Playoff PPG if the player's NHL team has played ≥3 playoff games and
///    the player is in the playoff points leaderboard. Grounded in current
///    form, noisy early.
/// 2. Regular-season PPG using `points / 82` from the regular-season leader
///    list.
/// 3. `DEFAULT_PPG` prior.
fn player_ppg(
    nhl_id: i64,
    nhl_team: &str,
    playoff_stats: Option<&StatsLeaders>,
    regular_stats: Option<&StatsLeaders>,
    games_played_so_far: &HashMap<String, u32>,
) -> f32 {
    let team_games = games_played_so_far.get(nhl_team).copied().unwrap_or(0);
    if team_games >= 3 {
        if let Some(pts) = playoff_stats
            .and_then(|s| s.points.iter().find(|p| p.id as i64 == nhl_id))
        {
            let ppg = pts.value as f32 / team_games as f32;
            if ppg > 0.0 {
                return ppg;
            }
        }
    }
    if let Some(pts) = regular_stats
        .and_then(|s| s.points.iter().find(|p| p.id as i64 == nhl_id))
    {
        let ppg = pts.value as f32 / 82.0;
        if ppg > 0.0 {
            return ppg;
        }
    }
    DEFAULT_PPG
}

// ---------------------------------------------------------------------------
// Rivalry card
// ---------------------------------------------------------------------------

fn attach_rivalry(mut response: RaceOddsResponse, my_team_id: Option<i64>) -> RaceOddsResponse {
    if response.mode != RaceOddsMode::League {
        response.rivalry = None;
        return response;
    }
    let Some(me_id) = my_team_id else {
        response.rivalry = None;
        return response;
    };
    response.rivalry = compute_rivalry(me_id, &response.team_odds);
    response
}

fn clone_without_rivalry(response: &RaceOddsResponse) -> RaceOddsResponse {
    let mut clone = response.clone();
    clone.rivalry = None;
    clone
}

/// Pick the fantasy team closest to `me` by projected mean and read the
/// exact MC head-to-head probability directly off the `TeamOdds.head_to_head`
/// map. No normal approximation — this value comes from counting trials
/// where `my_total > rival_total`, so it's guaranteed consistent with the
/// Insights Race Odds bar (the caller will see the same number on both
/// surfaces in a 2-team league).
fn compute_rivalry(my_team_id: i64, teams: &[TeamOdds]) -> Option<RivalryCard> {
    let me = teams.iter().find(|t| t.team_id == my_team_id)?;
    let rival = teams
        .iter()
        .filter(|t| t.team_id != my_team_id)
        .min_by(|a, b| {
            let da = (a.projected_final_mean - me.projected_final_mean).abs();
            let db = (b.projected_final_mean - me.projected_final_mean).abs();
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })?;

    let h2h = me
        .head_to_head
        .get(&rival.team_id)
        .copied()
        .unwrap_or_else(|| {
            // Shouldn't happen — MC always populates pairwise for every
            // other team — but fall back to the sort-based winner rather
            // than a mystery 0%.
            if me.projected_final_mean >= rival.projected_final_mean {
                1.0
            } else {
                0.0
            }
        });

    Some(RivalryCard {
        my_team_name: me.team_name.clone(),
        rival_team_name: rival.team_name.clone(),
        my_win_prob: me.win_prob,
        rival_win_prob: rival.win_prob,
        my_head_to_head_prob: h2h,
        my_projected_mean: me.projected_final_mean,
        rival_projected_mean: rival.projected_final_mean,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::nhl::{BottomSeed, PlayoffCarousel, Round, Series, TopSeed};
    use crate::utils::race_sim::TeamOdds;
    use std::collections::HashMap;

    fn mk_series(letter: &str, top: &str, top_w: i64, bot: &str, bot_w: i64) -> Series {
        Series {
            series_letter: letter.into(),
            round_number: 1,
            series_label: String::new(),
            series_link: String::new(),
            top_seed: TopSeed {
                id: 0,
                abbrev: top.into(),
                wins: top_w,
                logo: String::new(),
                dark_logo: String::new(),
            },
            bottom_seed: BottomSeed {
                id: 0,
                abbrev: bot.into(),
                wins: bot_w,
                logo: String::new(),
                dark_logo: String::new(),
            },
            needed_to_win: 4,
        }
    }

    #[test]
    fn bracket_from_carousel_pads_missing_rounds_with_future() {
        // Carousel has only R1 populated (the real shape on day 1 of
        // playoffs). Bracket must still have depth 4 with later rounds
        // filled with Future slots.
        let carousel = PlayoffCarousel {
            season_id: 20252026,
            current_round: 1,
            rounds: vec![Round {
                round_number: 1,
                round_label: "First Round".into(),
                round_abbrev: "R1".into(),
                series: (0..8)
                    .map(|i| {
                        mk_series(
                            &format!("{}", (b'A' + i as u8) as char),
                            &format!("T{}", i),
                            0,
                            &format!("B{}", i),
                            0,
                        )
                    })
                    .collect(),
            }],
        };
        let bracket = bracket_from_carousel(Some(&carousel));
        assert_eq!(bracket.depth(), 4);
        assert_eq!(bracket.rounds[0].len(), 8);
        assert_eq!(bracket.rounds[1].len(), 4);
        assert_eq!(bracket.rounds[2].len(), 2);
        assert_eq!(bracket.rounds[3].len(), 1);
        for slot in &bracket.rounds[1] {
            assert!(
                matches!(slot, SeriesState::Future),
                "R2 should be Future before any R1 series completes"
            );
        }
    }

    #[test]
    fn bracket_from_carousel_classifies_series_state() {
        // Mix: one completed (4-1), one in-progress (2-1), one 0-0
        // (participants known but not played).
        let carousel = PlayoffCarousel {
            season_id: 20252026,
            current_round: 1,
            rounds: vec![Round {
                round_number: 1,
                round_label: "First Round".into(),
                round_abbrev: "R1".into(),
                series: vec![
                    mk_series("A", "BOS", 4, "BUF", 1),
                    mk_series("B", "TBL", 2, "MTL", 1),
                    mk_series("C", "CAR", 0, "OTT", 0),
                ],
            }],
        };
        let bracket = bracket_from_carousel(Some(&carousel));
        match &bracket.rounds[0][0] {
            SeriesState::Completed {
                winner,
                loser,
                total_games,
            } => {
                assert_eq!(winner, "BOS");
                assert_eq!(loser, "BUF");
                assert_eq!(*total_games, 5);
            }
            other => panic!("expected Completed, got {:?}", other),
        }
        match &bracket.rounds[0][1] {
            SeriesState::InProgress {
                top_team,
                top_wins,
                bottom_team,
                bottom_wins,
            } => {
                assert_eq!(top_team, "TBL");
                assert_eq!(*top_wins, 2);
                assert_eq!(bottom_team, "MTL");
                assert_eq!(*bottom_wins, 1);
            }
            other => panic!("expected InProgress, got {:?}", other),
        }
        // Third entry is 0-0 but both teams populated → InProgress (about
        // to start), not Future.
        assert!(matches!(bracket.rounds[0][2], SeriesState::InProgress { .. }));
        // Remaining R1 slots padded with Future.
        for i in 3..8 {
            assert!(
                matches!(bracket.rounds[0][i], SeriesState::Future),
                "slot {} should be Future (padded)",
                i
            );
        }
    }

    #[test]
    fn bracket_from_carousel_absent_returns_empty() {
        let bracket = bracket_from_carousel(None);
        assert_eq!(bracket.depth(), 0);
    }

    fn odds_with_h2h(
        id: i64,
        name: &str,
        mean: f32,
        p10: f32,
        p90: f32,
        win_prob: f32,
        h2h: &[(i64, f32)],
    ) -> TeamOdds {
        TeamOdds {
            team_id: id,
            team_name: name.into(),
            current_points: 0,
            projected_final_mean: mean,
            projected_final_median: mean,
            p10,
            p90,
            win_prob,
            top3_prob: win_prob,
            head_to_head: h2h.iter().copied().collect::<HashMap<_, _>>(),
        }
    }

    fn odds(id: i64, name: &str, mean: f32, p10: f32, p90: f32, win_prob: f32) -> TeamOdds {
        odds_with_h2h(id, name, mean, p10, p90, win_prob, &[])
    }

    #[test]
    fn rivalry_picks_closest_team_and_uses_exact_h2h() {
        let teams = vec![
            odds_with_h2h(1, "Me", 100.0, 80.0, 120.0, 0.40, &[(2, 0.62), (3, 0.95)]),
            odds_with_h2h(2, "Close", 95.0, 75.0, 115.0, 0.35, &[(1, 0.38), (3, 0.90)]),
            odds_with_h2h(3, "Far", 50.0, 30.0, 70.0, 0.05, &[(1, 0.05), (2, 0.10)]),
        ];
        let card = compute_rivalry(1, &teams).expect("rivalry should be computed");
        assert_eq!(card.rival_team_name, "Close");
        // Must use the exact pairwise value, not a normal approximation.
        assert!(
            (card.my_head_to_head_prob - 0.62).abs() < 1e-6,
            "expected exact pairwise 0.62, got {}",
            card.my_head_to_head_prob
        );
    }

    #[test]
    fn rivalry_returns_none_for_solo_team() {
        let teams = vec![odds(1, "Alone", 100.0, 80.0, 120.0, 1.0)];
        assert!(compute_rivalry(1, &teams).is_none());
    }

    #[test]
    fn attach_rivalry_noop_in_champion_mode() {
        let teams = vec![odds(1, "A", 100.0, 80.0, 120.0, 0.6)];
        let response = RaceOddsResponse {
            generated_at: "now".into(),
            mode: RaceOddsMode::Champion,
            trials: 100,
            k_factor: 0.03,
            team_odds: teams,
            champion_leaderboard: Vec::new(),
            nhl_teams: Vec::new(),
            rivalry: Some(RivalryCard {
                my_team_name: "x".into(),
                rival_team_name: "y".into(),
                my_win_prob: 0.5,
                rival_win_prob: 0.5,
                my_head_to_head_prob: 0.5,
                my_projected_mean: 0.0,
                rival_projected_mean: 0.0,
            }),
        };
        let out = attach_rivalry(response, Some(1));
        assert!(out.rivalry.is_none(), "champion mode must not carry rivalry");
    }

}
