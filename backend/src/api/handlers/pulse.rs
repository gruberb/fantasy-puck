//! Pulse — personalised live dashboard.
//!
//! Post-Phase-5 the live data (my team status, series forecast, my
//! games tonight, league board) is recomputed from the NHL mirror
//! on every request. The expensive bit — the Claude narrative — is
//! cached separately in `response_cache` under
//! `pulse_narrative:{league}:{team}:{season}:{gt}:{date}`.
//!
//! Cache invalidation: the live poller (see
//! `infra::jobs::live_poller::poll_one_game`) observes each game's
//! `LIVE|CRIT -> OFF|FINAL` state transition and deletes narrative
//! cache rows for the leagues whose rostered players were in that
//! game. Next Pulse visit from those leagues re-generates the
//! narrative with the final score in view.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use axum::{
    extract::{Query, State},
    Json,
};
use chrono::Utc;
use tracing::{error, warn};

use crate::api::dtos::pulse::*;
use crate::api::response::{json_success, ApiResponse};
use crate::api::routes::AppState;
use crate::api::{game_type, season};
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

    // Narrative: cached per (league, team, day). Unlike the rest of
    // the payload (always freshly computed from the mirror), the
    // Claude call is expensive and its output only needs to change
    // when a game ends — at which point `live_poller` invalidates
    // the cache.
    if response.my_team.is_some() {
        response.narrative = resolve_narrative(
            &state,
            &league_id,
            my_team_id.unwrap_or(0),
            &today,
            &response,
        )
        .await;
    }

    Ok(json_success(response))
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

    Ok(PulseResponse {
        generated_at: Utc::now().to_rfc3339(),
        my_team,
        series_forecast,
        my_games_tonight,
        league_board,
        has_games_today,
        has_live_games,
        narrative: None,
    })
}

// ---------------------------------------------------------------------------
// Narrative cache (tiered)
// ---------------------------------------------------------------------------

async fn resolve_narrative(
    state: &Arc<AppState>,
    league_id: &str,
    my_team_id: i64,
    today: &str,
    response: &PulseResponse,
) -> Option<String> {
    let key = format!(
        "pulse_narrative:{}:{}:{}:{}:{}",
        league_id,
        my_team_id,
        season(),
        game_type(),
        today
    );
    if let Ok(Some(cached)) = state.db.cache().get_cached_response::<String>(&key).await {
        return Some(cached);
    }
    let generated = generate_pulse_narrative(response).await?;
    let _ = state
        .db
        .cache()
        .store_response(&key, today, &generated)
        .await;
    Some(generated)
}

async fn generate_pulse_narrative(response: &PulseResponse) -> Option<String> {
    let api_key = match std::env::var("ANTHROPIC_API_KEY") {
        Ok(v) => v,
        Err(_) => return None,
    };

    let payload = serde_json::to_string(response).ok()?;
    let no_playoff_scoring_yet = response
        .league_board
        .iter()
        .all(|e| e.total_points == 0 && e.points_today == 0);

    let mut headline = String::new();
    if let Some(t) = &response.my_team {
        headline.push_str(&format!(
            "Caller's team: {} · Rank #{} · {} total playoff pts · {} pts from the last completed scoring day · {}/{} players have an NHL game scheduled today.\n",
            t.team_name,
            t.rank,
            t.total_points,
            t.points_today,
            t.players_active_today,
            t.total_roster_size,
        ));
    }
    headline.push_str(&format!(
        "League has {} teams. {}.\n",
        response.league_board.len(),
        if response.has_live_games {
            "Games live right now"
        } else if response.has_games_today {
            "Games scheduled today"
        } else {
            "Off-day"
        }
    ));
    if no_playoff_scoring_yet {
        headline.push_str(
            "ZERO-STATE: no playoff games have been played yet in this league. Every team sits at 0 playoff points. Do not invent a gap, a lead, a 'last-day delta', or phrases like 'came into today with X points' — there is no scoring to reference. The only real content right now is who has how many active skaters tonight and which NHL matchups those skaters are in.\n",
        );
    }
    for entry in response.league_board.iter().take(3) {
        headline.push_str(&format!(
            "  #{} {} · {} pts\n",
            entry.rank, entry.team_name, entry.total_points
        ));
    }

    let request_body = serde_json::json!({
        "model": "claude-sonnet-4-6",
        "max_tokens": 1500,
        "system": r#"You are a veteran hockey columnist writing one personal dispatch for a friend in their fantasy league. Not a newsletter, not a pep talk — a direct read of where they stand and what matters. Think The Athletic beat column: dry, specific, opinionated, grounded in the numbers. Mix short punchy sentences with longer analytical ones.

Do not write like a marketing bot. Banned phrases and styles: "dive in", "unleash", "game-changer", "exciting journey", "let's break it down", "buckle up", "here's the scoop", bulleted listicles, exclamation points, hype adjectives. No section headers.

Rules:
- Only reference stats, names, records, and facts from the data provided.
- Never invent numbers.
- Wrap player names and fantasy-team names in **double asterisks** for bold.
- 4–7 sentences. Start on the verdict, not the weather.
- `points_today` / "pts from the last completed scoring day" is yesterday's daily total (or the last day whose games were processed), NOT live scoring from games happening right now. If today is day 1 of a new round, treat those numbers as the trailing day's work, never as "today's points". Phrases like "pulling X today" or "generating X off Y active players today" are wrong — say "came into today with X" or "closed the last day with X".

The frame: speak TO the caller (second person — "you", "your team"). This is their Pulse page, not a broadcast. Anchor on their rank, their gap to first, their closest threat, what today's slate means for them specifically, and any obvious read on which of their rostered NHL teams is carrying them. Be honest if the verdict isn't good."#,
        "messages": [
            {
                "role": "user",
                "content": format!(
                    "=== HEADLINE NUMBERS ===\n{}\n\n=== FULL PAYLOAD ===\n{}",
                    headline, payload
                )
            }
        ]
    });

    let client = match reqwest::Client::builder()
        .timeout(crate::tuning::http::CLAUDE_TIMEOUT)
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            error!("Pulse narrative: failed to build HTTP client: {}", e);
            return None;
        }
    };
    let http_response = match client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&request_body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            warn!("Pulse narrative: Claude API call failed: {}", e);
            return None;
        }
    };

    if !http_response.status().is_success() {
        let status = http_response.status();
        let body = http_response.text().await.unwrap_or_default();
        warn!("Pulse narrative: Claude returned {}: {}", status, body);
        return None;
    }

    let body: serde_json::Value = match http_response.json().await {
        Ok(v) => v,
        Err(e) => {
            warn!("Pulse narrative: failed to parse Claude response: {}", e);
            return None;
        }
    };
    body.get("content")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|block| block.get("text"))
        .and_then(|t| t.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
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
    )
    .await?;
    let totals_by_team: HashMap<i64, i32> =
        totals.into_iter().map(|r| (r.team_id, r.points as i32)).collect();

    let sparklines = state
        .db
        .get_team_sparklines(league_id, 5, crate::api::playoff_start())
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
