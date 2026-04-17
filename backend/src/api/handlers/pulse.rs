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
use crate::api::{game_type, season};
use crate::auth::middleware::AuthUser;
use crate::error::Result;
use crate::models::fantasy::{FantasyTeamInGame, PlayerInGame};
use crate::nhl_api::nhl_constants::team_names;
use crate::utils::series_projection::{
    classify, games_remaining, probability_to_advance, SeriesStateCode,
};

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

    // Resolve the caller's fantasy team inside this league (if any).
    let my_team_id = resolve_my_team_id(&state, &league_id, &auth_user.id).await;

    let today = hockey_today();
    let cache_key = format!(
        "pulse:{}:{}:{}:{}",
        league_id,
        season(),
        game_type(),
        today
    );

    if let Some(cached) = state
        .db
        .cache()
        .get_cached_response::<PulseResponse>(&cache_key)
        .await?
    {
        // If the cache was generated earlier and no games were live back then,
        // it's safe to reuse regardless of time of day. If live games are
        // possible (checked on miss), return the cached value too — the
        // frontend auto-refresh pattern will invalidate us.
        return Ok(json_success(cached));
    }

    let response = generate_pulse(&state, &league_id, my_team_id, &today).await?;

    // Cache for today; scheduler cleans stale keys after 7 days. Short TTL is
    // handled implicitly by date-keyed invalidation.
    let _ = state
        .db
        .cache()
        .store_response(&cache_key, &today, &response)
        .await;

    Ok(json_success(response))
}

// ---------------------------------------------------------------------------
// Top-level orchestrator
// ---------------------------------------------------------------------------

async fn generate_pulse(
    state: &Arc<AppState>,
    league_id: &str,
    my_team_id: Option<i64>,
    today: &str,
) -> Result<PulseResponse> {
    // Fetch the foundational data once, then compose the response.
    let teams_with_players = state.db.get_all_teams_with_players(league_id).await?;
    let carousel = state
        .nhl_client
        .get_playoff_carousel(season().to_string())
        .await
        .ok()
        .flatten();
    let schedule = state
        .nhl_client
        .get_schedule_by_date(today)
        .await
        .ok();
    let games_today = schedule
        .as_ref()
        .map(|s| s.games_for_date(today))
        .unwrap_or_default();

    let series_forecast = build_series_forecast(&teams_with_players, carousel.as_ref());
    let league_board = build_league_board(state, league_id, &teams_with_players, today, my_team_id, &games_today).await?;
    let my_team = my_team_id.and_then(|id| compose_my_team(&league_board, &teams_with_players, id));
    let my_games_tonight = if let Some(id) = my_team_id {
        compute_my_games_tonight(state, &teams_with_players, id, &games_today).await
    } else {
        Vec::new()
    };
    let has_games_today = !games_today.is_empty();
    let has_live_games = games_today.iter().any(|g| g.game_state.is_live());

    Ok(PulseResponse {
        generated_at: Utc::now().to_rfc3339(),
        my_team,
        series_forecast,
        my_games_tonight,
        league_board,
        has_games_today,
        has_live_games,
    })
}

// ---------------------------------------------------------------------------
// Series Forecast (flagship)
// ---------------------------------------------------------------------------

/// Map team_abbrev -> (wins, opp_wins, opponent_abbrev, round, is_active_round).
struct TeamSeriesState {
    wins: u32,
    opp_wins: u32,
    opponent_abbrev: String,
}

fn build_team_states(
    carousel: &crate::models::nhl::PlayoffCarousel,
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
    carousel: Option<&crate::models::nhl::PlayoffCarousel>,
) -> Vec<FantasyTeamForecast> {
    let team_states = carousel
        .map(build_team_states)
        .unwrap_or_default();

    teams
        .iter()
        .map(|team| {
            let mut cells: Vec<PlayerForecastCell> = Vec::new();
            let mut eliminated = 0usize;
            let mut facing_elim = 0usize;
            let mut trailing = 0usize;
            let mut leading = 0usize;
            let mut advanced = 0usize;

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
                    SeriesStateCode::Tied => trailing += 1,
                    SeriesStateCode::Leading => leading += 1,
                    SeriesStateCode::AboutToAdvance => leading += 1,
                    SeriesStateCode::Advanced => advanced += 1,
                }

                cells.push(PlayerForecastCell {
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
                players_leading: leading,
                players_advanced: advanced,
                cells,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// League Live Board (with sparklines + today deltas)
// ---------------------------------------------------------------------------

async fn build_league_board(
    state: &Arc<AppState>,
    league_id: &str,
    teams: &[FantasyTeamInGame],
    today: &str,
    my_team_id: Option<i64>,
    games_today: &[crate::models::nhl::TodayGame],
) -> Result<Vec<LeagueBoardEntry>> {
    // Playoff totals per team via existing skater-stats pipeline.
    let stats = state
        .nhl_client
        .get_skater_stats(&season(), game_type())
        .await
        .ok();

    // Build player_id -> points (playoff total) map.
    let mut player_points: HashMap<i64, i32> = HashMap::new();
    if let Some(s) = stats {
        for group in [&s.points, &s.goals, &s.assists] {
            for p in group {
                let value = p.value as i32;
                let current = player_points.entry(p.id as i64).or_insert(0);
                // Use the max across categories — the points category gives the
                // correct total but may be missing for some; fall back to any.
                if value > *current {
                    *current = value;
                }
            }
        }
        // Overwrite with canonical points category when available.
        for p in &s.points {
            player_points.insert(p.id as i64, p.value as i32);
        }
    }

    // Sparklines: last 5 days of daily_rankings per team.
    let sparklines = state.db.get_team_sparklines(league_id, 5).await.unwrap_or_default();

    // Today's points — use yesterday's daily_ranking if scheduler already
    // processed; otherwise 0. Since the scheduler runs on yesterday's games,
    // today's points only become visible after 9am UTC the next day. For live
    // games we defer to the boxscore-driven Pulse section which is independent.
    let nhl_teams_today: HashSet<String> = games_today
        .iter()
        .flat_map(|g| vec![g.home_team.abbrev.clone(), g.away_team.abbrev.clone()])
        .collect();

    // Compute total_points + active-today count per team.
    let mut entries: Vec<LeagueBoardEntry> = teams
        .iter()
        .map(|team| {
            let total_points: i32 = team
                .players
                .iter()
                .map(|p| *player_points.get(&p.nhl_id).unwrap_or(&0))
                .sum();
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

    // Rank descending by total_points, then by team_name for stability.
    entries.sort_by(|a, b| {
        b.total_points
            .cmp(&a.total_points)
            .then_with(|| a.team_name.cmp(&b.team_name))
    });
    for (i, e) in entries.iter_mut().enumerate() {
        e.rank = i + 1;
    }

    // Points-today from yesterday's daily_rankings. If today's games haven't
    // been processed yet, the most recent entry in the sparkline reflects
    // yesterday's total — treat that as "latest day" delta.
    // (For in-progress games the real points will be picked up by the
    // boxscore-driven My Games Tonight section, not the board.)
    for entry in &mut entries {
        if let Some(&last) = entry.sparkline.last() {
            entry.points_today = last;
        }
        let _ = today; // unused today marker; future: live aggregation
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
// My Games Tonight
// ---------------------------------------------------------------------------

async fn compute_my_games_tonight(
    state: &Arc<AppState>,
    teams: &[FantasyTeamInGame],
    my_team_id: i64,
    games_today: &[crate::models::nhl::TodayGame],
) -> Vec<MyGameTonight> {
    let my_team = match teams.iter().find(|t| t.team_id == my_team_id) {
        Some(t) => t,
        None => return Vec::new(),
    };

    // Group your players by the game they're in.
    let mut out = Vec::new();
    for game in games_today {
        let home = &game.home_team.abbrev;
        let away = &game.away_team.abbrev;
        let my_players: Vec<&PlayerInGame> = my_team
            .players
            .iter()
            .filter(|p| &p.nhl_team == home || &p.nhl_team == away)
            .collect();
        if my_players.is_empty() {
            continue;
        }

        // Live stats via boxscore when live/completed.
        let boxscore = if game.game_state.is_live() || game.game_state.is_completed() {
            state.nhl_client.get_game_boxscore(game.id).await.ok()
        } else {
            None
        };

        let mut players_signal = Vec::new();
        for p in &my_players {
            let (goals, assists) = match &boxscore {
                Some(bs) => crate::utils::nhl::find_player_stats_by_name(
                    bs,
                    &p.nhl_team,
                    &p.player_name,
                    Some(p.nhl_id),
                ),
                None => (0, 0),
            };
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

        let (series_context, is_elimination) = match &game.series_status {
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

        let (home_score, away_score) = match &game.game_score {
            Some(s) => (Some(s.home), Some(s.away)),
            None => (None, None),
        };
        let period = game.period_descriptor.as_ref().and_then(|p| {
            let n = p.number.unwrap_or(0);
            p.period_type
                .as_ref()
                .map(|pt| format!("{} {}", n, pt))
        });

        out.push(MyGameTonight {
            game_id: game.id,
            home_team: home.clone(),
            home_team_name: team_names::get_team_name(home).to_string(),
            home_team_logo: state.nhl_client.get_team_logo_url(home),
            away_team: away.clone(),
            away_team_name: team_names::get_team_name(away).to_string(),
            away_team_logo: state.nhl_client.get_team_logo_url(away),
            start_time_utc: game.start_time_utc.clone(),
            venue: game.venue.default.clone(),
            game_state: format!("{:?}", game.game_state),
            home_score,
            away_score,
            period,
            series_context,
            is_elimination,
            my_players: players_signal,
        });
    }
    out
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
    // Look up via league members, matching on user_id.
    match state.db.get_league_members(league_id).await {
        Ok(members) => {
            for m in members {
                if m.user_id == user_id {
                    return Some(m.fantasy_team_id);
                }
            }
            None
        }
        Err(e) => {
            warn!("Failed to look up league members for my_team_id: {}", e);
            None
        }
    }
}
