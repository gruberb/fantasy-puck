//! Pulse — personalised live dashboard.
//!
//! The live data (my team status, series forecast, my games
//! tonight, per-team breakdown, league outlook) is recomputed from
//! the NHL mirror on every request. The expensive bits — the
//! team-diagnosis narrative (one Claude round-trip per caller's
//! team per hockey-date) and the Monte Carlo race-odds payload —
//! are cached separately in `response_cache` under
//! `team_diagnosis:{league}:{team}:{season}:{gt}:{date}:v2` and
//! `race_odds:v4:{league}:{season}:{gt}:{date}`.
//!
//! Cache invalidation: the live poller (see
//! `infra::jobs::live_poller::poll_one_game`) observes each game's
//! `LIVE|CRIT -> OFF|FINAL` state transition and deletes the
//! `team_diagnosis:{league}:*` rows for leagues whose rostered
//! players were in that game. Next Pulse visit regenerates the
//! narrative with the final score in view.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use axum::{
    extract::{Query, State},
    Json,
};
use chrono::Utc;
use tracing::warn;

use crate::api::dtos::pulse::*;
use crate::api::response::{json_success, ApiResponse};
use crate::api::routes::AppState;
use crate::api::{current_date_window, game_type, season};
use crate::auth::middleware::AuthUser;
use crate::domain::models::fantasy::{FantasyTeamInGame, PlayerInGame};
use crate::domain::models::nhl::{GameState, SeriesStatus};
use crate::domain::prediction::series_projection::{
    classify, games_remaining, probability_to_advance, SeriesStateCode,
};
use crate::error::Result;
use crate::infra::db::nhl_mirror::{self, NhlGameRow, PlayerGameStatRow};
use crate::infra::nhl::constants::team_names;

// ---------------------------------------------------------------------------
// Public handler
// ---------------------------------------------------------------------------

pub async fn get_pulse(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<PulseResponse>>> {
    let league_id = params
        .get("league_id")
        .cloned()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| crate::error::Error::Validation("league_id is required".into()))?;

    state
        .db
        .verify_user_in_league(&league_id, &auth_user.id)
        .await?;

    let my_team_id = resolve_my_team_id(&state, &league_id, &auth_user.id).await;
    let today = hockey_today();

    let mut response = generate_pulse(&state, &league_id, my_team_id, &today).await?;

    // Your Read block — the per-player breakdown + descriptive
    // diagnosis narrative. Shares the same composition helper and
    // cache key (`team_diagnosis:*`) used by the Fantasy Team Detail
    // handler; the live poller invalidates both on game-end.
    if let Some(team_id) = my_team_id {
        response.my_team_diagnosis =
            build_my_team_diagnosis(&state, &league_id, team_id).await;
    }

    // Your League block — leader, distribution, and top-3 projected
    // finishers from the cached race-odds payload. Best-effort: if
    // the cache is cold, this returns None and the UI hides the block.
    response.league_outlook =
        build_league_outlook(&state, &league_id, &today, my_team_id).await;

    Ok(json_success(response))
}

async fn build_my_team_diagnosis(
    state: &Arc<AppState>,
    league_id: &str,
    team_id: i64,
) -> Option<crate::api::dtos::pulse::MyTeamDiagnosis> {
    if game_type() != 3 {
        return None;
    }
    let team = state.db.get_team(team_id, league_id).await.ok()?;
    let players = state.db.get_team_players(team_id).await.ok()?;
    let bundle = crate::api::handlers::team_breakdown::compose_team_breakdown(
        state,
        league_id,
        team_id,
        &team.name,
        &players,
    )
    .await
    .ok()?;
    Some(crate::api::dtos::pulse::MyTeamDiagnosis {
        team_id,
        team_name: team.name,
        total_points: bundle.team_totals.total_points,
        diagnosis: bundle.diagnosis,
        players: bundle.players,
    })
}

fn largest_stack(nhl_teams: &[String]) -> Option<(String, u32)> {
    let mut counts: std::collections::HashMap<&str, u32> = std::collections::HashMap::new();
    for t in nhl_teams {
        *counts.entry(t.as_str()).or_insert(0) += 1;
    }
    counts
        .into_iter()
        .max_by_key(|(_, c)| *c)
        .map(|(a, c)| (a.to_string(), c))
}

async fn build_league_outlook(
    state: &Arc<AppState>,
    league_id: &str,
    today: &str,
    _my_team_id: Option<i64>,
) -> Option<crate::api::dtos::pulse::LeagueOutlook> {
    use crate::api::dtos::pulse::{LeagueOutlook, LeagueOutlookEntry, LeagueOutlookStack};
    use crate::infra::prediction::race_odds_cache;

    if game_type() != 3 {
        return None;
    }
    let pool = state.db.pool();

    let league_totals = nhl_mirror::list_league_team_season_totals(
        pool,
        league_id,
        season() as i32,
        3,
        current_date_window(),
    )
    .await
    .ok()?;
    if league_totals.is_empty() {
        return None;
    }

    let leader = league_totals.first()?;
    let mut points_distribution: Vec<i32> =
        league_totals.iter().map(|r| r.points as i32).collect();
    points_distribution.sort_unstable_by(|a, b| b.cmp(a));
    let median_points = {
        let n = points_distribution.len();
        if n == 0 {
            0.0
        } else if n % 2 == 0 {
            (points_distribution[n / 2 - 1] + points_distribution[n / 2]) as f32 / 2.0
        } else {
            points_distribution[n / 2] as f32
        }
    };

    let race_payload = state
        .db
        .cache()
        .get_cached_response::<crate::api::dtos::race_odds::RaceOddsResponse>(
            &race_odds_cache::cache_key(league_id, season(), game_type(), today),
        )
        .await
        .ok()
        .flatten();

    let nhl_team_odds =
        race_odds_cache::load_nhl_team_odds(state, league_id, season(), game_type(), today).await;

    let teams_with_players = state
        .db
        .get_all_teams_with_players(league_id)
        .await
        .unwrap_or_default();
    let rosters_by_team: std::collections::HashMap<i64, Vec<String>> = teams_with_players
        .iter()
        .map(|t| {
            (
                t.team_id,
                t.players.iter().map(|p| p.nhl_team.clone()).collect(),
            )
        })
        .collect();
    let team_totals_by_id: std::collections::HashMap<i64, i32> = league_totals
        .iter()
        .map(|r| (r.team_id, r.points as i32))
        .collect();

    let top_three: Vec<LeagueOutlookEntry> = match &race_payload {
        Some(payload) if !payload.team_odds.is_empty() => {
            let mut ranked: Vec<&crate::domain::prediction::race_sim::TeamOdds> =
                payload.team_odds.iter().collect();
            ranked.sort_by(|a, b| {
                b.projected_final_mean
                    .partial_cmp(&a.projected_final_mean)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            ranked
                .into_iter()
                .take(3)
                .map(|t| {
                    let top_stack = rosters_by_team
                        .get(&t.team_id)
                        .and_then(|nhl_teams| largest_stack(nhl_teams))
                        .map(|(abbrev, rostered)| LeagueOutlookStack {
                            cup_win_prob: nhl_team_odds
                                .get(&abbrev)
                                .map(|o| o.cup_win_prob)
                                .unwrap_or(0.0),
                            nhl_team: abbrev,
                            rostered: rostered as i32,
                        });
                    LeagueOutlookEntry {
                        team_id: t.team_id,
                        team_name: t.team_name.clone(),
                        current_points: team_totals_by_id.get(&t.team_id).copied().unwrap_or(0),
                        projected_final_mean: t.projected_final_mean,
                        win_prob: t.win_prob,
                        top3_prob: t.top3_prob,
                        top_stack,
                    }
                })
                .collect()
        }
        _ => Vec::new(),
    };

    Some(LeagueOutlook {
        total_teams: league_totals.len() as i32,
        leader_team_id: leader.team_id,
        leader_name: leader.team_name.clone(),
        leader_points: leader.points as i32,
        points_distribution,
        median_points,
        top_three,
    })
}

// ---------------------------------------------------------------------------
// Pulse assembly (live data — every request)
// ---------------------------------------------------------------------------

async fn generate_pulse(
    state: &Arc<AppState>,
    league_id: &str,
    my_team_id: Option<i64>,
    today: &str,
) -> Result<PulseResponse> {
    let pool = state.db.pool();

    let teams_with_players = state.db.get_all_teams_with_players(league_id).await?;
    let carousel = nhl_mirror::get_playoff_carousel(pool, season() as i32)
        .await
        .unwrap_or(None);
    let games_today: Vec<NhlGameRow> = nhl_mirror::list_games_for_date(pool, today).await?;

    let series_forecast = build_series_forecast(&teams_with_players, carousel.as_ref());
    let league_board =
        build_league_board(state, league_id, &teams_with_players, my_team_id, &games_today)
            .await?;
    let my_team = my_team_id.and_then(|id| compose_my_team(&league_board, &teams_with_players, id));
    let my_games_tonight = if let Some(id) = my_team_id {
        compute_my_games_tonight(state, &teams_with_players, id, &games_today).await?
    } else {
        Vec::new()
    };

    let has_games_today = !games_today.is_empty();
    let has_live_games = games_today
        .iter()
        .any(|g| matches!(g.game_state.as_str(), "LIVE" | "CRIT"));

    // Best-effort lift of per-NHL-team cup odds from the cached
    // race-odds payload. Empty map when the cache hasn't warmed yet
    // (e.g. before the morning Monte Carlo cron) — the narrator simply
    // skips any cup-odds phrasing in that case.
    let nhl_team_cup_odds = crate::infra::prediction::race_odds_cache::load_cached_cup_odds(
        state,
        league_id,
        season(),
        game_type(),
        today,
    )
    .await;

    let games_today_matchups: Vec<crate::api::dtos::pulse::GameMatchup> = games_today
        .iter()
        .map(|g| crate::api::dtos::pulse::GameMatchup {
            home_team: g.home_team.clone(),
            away_team: g.away_team.clone(),
        })
        .collect();

    Ok(PulseResponse {
        generated_at: Utc::now().to_rfc3339(),
        my_team,
        series_forecast,
        my_games_tonight,
        league_board,
        has_games_today,
        has_live_games,
        games_today: games_today_matchups,
        nhl_team_cup_odds,
        my_team_diagnosis: None,
        league_outlook: None,
    })
}

// ---------------------------------------------------------------------------
// Series Forecast (pure compute against carousel + rosters)
// ---------------------------------------------------------------------------

struct TeamSeriesState {
    wins: u32,
    opp_wins: u32,
    opponent_abbrev: String,
}

fn build_team_states(
    carousel: &crate::domain::models::nhl::PlayoffCarousel,
) -> HashMap<String, TeamSeriesState> {
    let mut map = HashMap::new();
    for round in &carousel.rounds {
        for series in &round.series {
            let top = &series.top_seed;
            let bot = &series.bottom_seed;
            map.insert(
                top.abbrev.clone(),
                TeamSeriesState {
                    wins: top.wins as u32,
                    opp_wins: bot.wins as u32,
                    opponent_abbrev: bot.abbrev.clone(),
                },
            );
            map.insert(
                bot.abbrev.clone(),
                TeamSeriesState {
                    wins: bot.wins as u32,
                    opp_wins: top.wins as u32,
                    opponent_abbrev: top.abbrev.clone(),
                },
            );
        }
    }
    map
}

fn build_series_forecast(
    teams: &[FantasyTeamInGame],
    carousel: Option<&crate::domain::models::nhl::PlayoffCarousel>,
) -> Vec<FantasyTeamForecast> {
    let team_states = carousel.map(build_team_states).unwrap_or_default();

    teams
        .iter()
        .map(|team| {
            let mut cells: Vec<PlayerForecastCell> = Vec::new();
            let (mut eliminated, mut facing_elim, mut trailing, mut tied, mut leading, mut advanced) =
                (0usize, 0usize, 0usize, 0usize, 0usize, 0usize);

            for p in &team.players {
                let (wins, opp_wins, opp_abbrev) = team_states
                    .get(&p.nhl_team)
                    .map(|s| (s.wins, s.opp_wins, Some(s.opponent_abbrev.clone())))
                    .unwrap_or((0, 0, None));

                let state = classify(wins, opp_wins);
                match state {
                    SeriesStateCode::Eliminated => eliminated += 1,
                    SeriesStateCode::FacingElim => facing_elim += 1,
                    SeriesStateCode::Trailing => trailing += 1,
                    SeriesStateCode::Tied => tied += 1,
                    SeriesStateCode::Leading => leading += 1,
                    SeriesStateCode::AboutToAdvance => leading += 1,
                    SeriesStateCode::Advanced => advanced += 1,
                }

                cells.push(PlayerForecastCell {
                    nhl_id: p.nhl_id,
                    player_name: p.player_name.clone(),
                    position: p.position.clone(),
                    nhl_team: p.nhl_team.clone(),
                    nhl_team_name: team_names::get_team_name(&p.nhl_team).to_string(),
                    opponent_abbrev: opp_abbrev.clone(),
                    opponent_name: opp_abbrev
                        .as_ref()
                        .map(|a| team_names::get_team_name(a).to_string()),
                    series_state: state,
                    series_label: state.label(wins, opp_wins),
                    wins,
                    opponent_wins: opp_wins,
                    odds_to_advance: probability_to_advance(wins, opp_wins),
                    games_remaining: games_remaining(wins, opp_wins),
                    headshot_url: format!(
                        "https://assets.nhle.com/mugs/nhl/latest/{}.png",
                        p.nhl_id
                    ),
                });
            }

            FantasyTeamForecast {
                team_id: team.team_id,
                team_name: team.team_name.clone(),
                total_players: team.players.len(),
                players_eliminated: eliminated,
                players_facing_elimination: facing_elim,
                players_trailing: trailing,
                players_tied: tied,
                players_leading: leading,
                players_advanced: advanced,
                cells,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// League Live Board (mirror-backed)
// ---------------------------------------------------------------------------

async fn build_league_board(
    state: &Arc<AppState>,
    league_id: &str,
    teams: &[FantasyTeamInGame],
    my_team_id: Option<i64>,
    games_today: &[NhlGameRow],
) -> Result<Vec<LeagueBoardEntry>> {
    let pool = state.db.pool();

    // Season totals per fantasy team — sum over nhl_player_game_stats
    // so depth scorers outside the leaderboard are counted.
    let totals = nhl_mirror::list_league_team_season_totals(
        pool,
        league_id,
        season() as i32,
        game_type() as i16,
        current_date_window(),
    )
    .await?;
    let totals_by_team: HashMap<i64, i32> =
        totals.into_iter().map(|r| (r.team_id, r.points as i32)).collect();

    // Live-aware sparkline: UNIONs daily_rankings (finalized rollups)
    // with v_daily_fantasy_totals (today's running total from the
    // mirror). Without the view leg the chart is blank on day 1 of
    // any round and on every day before the afternoon cron has
    // processed that day — today's scoring is in
    // nhl_player_game_stats immediately but not yet in daily_rankings.
    let sparklines = state
        .db
        .get_team_sparklines_with_live(league_id, 5, crate::api::playoff_start())
        .await
        .unwrap_or_default();

    let nhl_teams_today: HashSet<String> = games_today
        .iter()
        .flat_map(|g| [g.home_team.clone(), g.away_team.clone()])
        .collect();

    let mut entries: Vec<LeagueBoardEntry> = teams
        .iter()
        .map(|team| {
            let total_points = totals_by_team.get(&team.team_id).copied().unwrap_or(0);
            let active_today = team
                .players
                .iter()
                .filter(|p| nhl_teams_today.contains(&p.nhl_team))
                .count();
            let sparkline = sparklines.get(&team.team_id).cloned().unwrap_or_default();

            LeagueBoardEntry {
                rank: 0,
                team_id: team.team_id,
                team_name: team.team_name.clone(),
                total_points,
                points_today: 0,
                players_active_today: active_today,
                sparkline,
                is_my_team: my_team_id == Some(team.team_id),
            }
        })
        .collect();

    entries.sort_by(|a, b| {
        b.total_points
            .cmp(&a.total_points)
            .then_with(|| a.team_name.cmp(&b.team_name))
    });
    for (i, e) in entries.iter_mut().enumerate() {
        e.rank = i + 1;
    }

    // "Points from last completed scoring day" — last sparkline entry.
    for entry in &mut entries {
        if let Some(&last) = entry.sparkline.last() {
            entry.points_today = last;
        }
    }

    Ok(entries)
}

fn compose_my_team(
    board: &[LeagueBoardEntry],
    teams: &[FantasyTeamInGame],
    my_team_id: i64,
) -> Option<MyTeamStatus> {
    let entry = board.iter().find(|e| e.team_id == my_team_id)?;
    let roster_size = teams
        .iter()
        .find(|t| t.team_id == my_team_id)
        .map(|t| t.players.len())
        .unwrap_or(0);
    Some(MyTeamStatus {
        team_id: entry.team_id,
        team_name: entry.team_name.clone(),
        rank: entry.rank,
        total_points: entry.total_points,
        points_today: entry.points_today,
        players_active_today: entry.players_active_today,
        total_roster_size: roster_size,
    })
}

// ---------------------------------------------------------------------------
// My Games Tonight (mirror-backed)
// ---------------------------------------------------------------------------

async fn compute_my_games_tonight(
    state: &Arc<AppState>,
    teams: &[FantasyTeamInGame],
    my_team_id: i64,
    games_today: &[NhlGameRow],
) -> Result<Vec<MyGameTonight>> {
    let my_team = match teams.iter().find(|t| t.team_id == my_team_id) {
        Some(t) => t,
        None => return Ok(Vec::new()),
    };

    // Pre-load player game stats for every game in today's slate in
    // one SQL round-trip.
    let game_ids: Vec<i64> = games_today.iter().map(|g| g.game_id).collect();
    let all_player_rows =
        nhl_mirror::list_player_game_stats_for_games(state.db.pool(), &game_ids).await?;
    let mut by_game: HashMap<i64, Vec<PlayerGameStatRow>> = HashMap::new();
    for row in all_player_rows {
        by_game.entry(row.game_id).or_default().push(row);
    }

    let mut out = Vec::new();
    for game in games_today {
        let my_players: Vec<&PlayerInGame> = my_team
            .players
            .iter()
            .filter(|p| p.nhl_team == game.home_team || p.nhl_team == game.away_team)
            .collect();
        if my_players.is_empty() {
            continue;
        }

        let stats_by_id: HashMap<i64, &PlayerGameStatRow> = by_game
            .get(&game.game_id)
            .map(|rows| rows.iter().map(|r| (r.player_id, r)).collect())
            .unwrap_or_default();

        let mut players_signal = Vec::new();
        for p in &my_players {
            let (goals, assists) = stats_by_id
                .get(&p.nhl_id)
                .map(|r| (r.goals, r.assists))
                .unwrap_or((0, 0));
            players_signal.push(MyPlayerInGame {
                nhl_id: p.nhl_id,
                name: p.player_name.clone(),
                position: p.position.clone(),
                nhl_team: p.nhl_team.clone(),
                headshot_url: format!(
                    "https://assets.nhle.com/mugs/nhl/latest/{}.png",
                    p.nhl_id
                ),
                goals,
                assists,
                points: goals + assists,
            });
        }

        let series = game
            .series_status
            .as_ref()
            .and_then(|v| serde_json::from_value::<SeriesStatus>(v.clone()).ok());
        let (series_context, is_elimination) = match series {
            Some(ss) => {
                let label = format!(
                    "{} - {} leads {}-{}",
                    ss.series_title,
                    if ss.top_seed_wins >= ss.bottom_seed_wins {
                        &ss.top_seed_team_abbrev
                    } else {
                        &ss.bottom_seed_team_abbrev
                    },
                    ss.top_seed_wins.max(ss.bottom_seed_wins),
                    ss.top_seed_wins.min(ss.bottom_seed_wins)
                );
                let elim = ss.top_seed_wins == 3 || ss.bottom_seed_wins == 3;
                (Some(label), elim)
            }
            None => (None, false),
        };

        let period = format_period(game.period_number, game.period_type.as_deref());

        out.push(MyGameTonight {
            game_id: game.game_id as u32,
            home_team: game.home_team.clone(),
            home_team_name: team_names::get_team_name(&game.home_team).to_string(),
            home_team_logo: state.nhl_client.get_team_logo_url(&game.home_team),
            away_team: game.away_team.clone(),
            away_team_name: team_names::get_team_name(&game.away_team).to_string(),
            away_team_logo: state.nhl_client.get_team_logo_url(&game.away_team),
            start_time_utc: game.start_time_utc.to_rfc3339(),
            venue: game.venue.clone().unwrap_or_default(),
            game_state: format_game_state(&game.game_state),
            home_score: game.home_score,
            away_score: game.away_score,
            period,
            series_context,
            is_elimination,
            my_players: players_signal,
        });
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn hockey_today() -> String {
    use chrono_tz::America::New_York;
    Utc::now()
        .with_timezone(&New_York)
        .format("%Y-%m-%d")
        .to_string()
}

async fn resolve_my_team_id(
    state: &Arc<AppState>,
    league_id: &str,
    user_id: &str,
) -> Option<i64> {
    match state.db.get_league_members(league_id).await {
        Ok(members) => members
            .into_iter()
            .find(|m| m.user_id == user_id)
            .map(|m| m.fantasy_team_id),
        Err(e) => {
            warn!("Failed to look up league members for my_team_id: {}", e);
            None
        }
    }
}

fn format_period(number: Option<i16>, period_type: Option<&str>) -> Option<String> {
    let n = number?;
    let label = period_type.unwrap_or("");
    Some(format!("{} {}", n, label))
}

/// Map the string stored in `nhl_games.game_state` to the debug-form
/// variant name the existing Pulse DTO uses (`Live`, `Final`, `Off`,
/// `Fut`, `Preview`, `Crit`, `Unknown`). We round-trip via the typed
/// `GameState` enum so the string shape matches what the handler
/// produced before.
fn format_game_state(raw: &str) -> String {
    use std::str::FromStr;
    let state: GameState = GameState::from_str(raw).unwrap_or_default();
    format!("{:?}", state)
}
