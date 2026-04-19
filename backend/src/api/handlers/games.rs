//! Games / Match-Day handlers.
//!
//! Post-Phase-4, all three code paths are pure database reads:
//!
//! - `list_games` with `detail=basic` or no league — returns tonight's
//!   slate + fantasy player overlays for each team. Reads
//!   `nhl_games` and `nhl_player_game_stats`.
//! - `list_games` with `detail=extended` + league — adds each
//!   rostered player's last-5-games form and playoff totals.
//!   Reads the same tables plus `list_player_form` /
//!   `list_player_playoff_totals`.
//! - `get_match_day` — the dashboard Match Day widget. Similar
//!   shape to extended. Reads the same three tables.
//!
//! No `response_cache` — the live poller keeps the underlying
//! mirror tables fresh every 60 s, so stale reads aren't possible.
//! No `state.nhl_client` — the only code that talks to NHL is the
//! poller in `infra/jobs`.

use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::Arc;

use axum::{
    extract::{Query, State},
    Json,
};

use crate::api::dtos::*;
use crate::api::response::{json_success, ApiResponse};
use crate::api::routes::AppState;
use crate::api::season as cfg_season;
use crate::domain::models::fantasy::{FantasyTeamInGame, PlayerInGame};
use crate::domain::models::nhl::{GameState, SeriesStatus};
use crate::error::Result;
use crate::infra::db::nhl_mirror::{
    self, NhlGameRow, PlayerFormRow, PlayerGameStatRow, PlayerPlayoffTotalsRow,
};
use crate::infra::nhl::client::NhlClient;

// ---------------------------------------------------------------------
// Query helpers
// ---------------------------------------------------------------------

fn parse_date(params: &HashMap<String, String>) -> Result<String> {
    match params.get("date") {
        Some(d) if d.len() == 10 && d.chars().all(|c| c == '-' || c.is_ascii_digit()) => {
            Ok(d.clone())
        }
        Some(_) => Err(crate::error::Error::Validation(
            "Invalid date format. Use YYYY-MM-DD".into(),
        )),
        None => Err(crate::error::Error::Validation(
            "Date parameter is required (format: YYYY-MM-DD)".into(),
        )),
    }
}

// ---------------------------------------------------------------------
// Public handlers
// ---------------------------------------------------------------------

pub async fn list_games(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<TodaysGamesResponse>>> {
    let league_id = params.get("league_id").cloned().unwrap_or_default();
    let detail = params.get("detail").cloned().unwrap_or_default();
    let date = parse_date(&params)?;

    if detail == "extended" && !league_id.is_empty() {
        process_extended(&state, &date, &league_id).await
    } else {
        process_basic(&state, &date, &league_id).await
    }
}

pub async fn get_match_day(
    State(state): State<Arc<AppState>>,
    Query(league_params): Query<LeagueParams>,
) -> Result<Json<ApiResponse<MatchDayResponse>>> {
    let league_id = &league_params.league_id;
    let now_et = chrono::Utc::now().with_timezone(&chrono_tz::America::New_York);
    let hockey_today = now_et.format("%Y-%m-%d").to_string();

    // Early-morning carry-over: if a west-coast game from yesterday
    // is still LIVE, include it in today's response. Rare on playoff
    // nights but legitimate for long OT games.
    let include_yesterday = now_et.time().format("%H").to_string().parse::<u32>().unwrap_or(12) < 12;
    let hockey_yesterday = (now_et - chrono::Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();

    let mut games: Vec<NhlGameRow> =
        nhl_mirror::list_games_for_date(state.db.pool(), &hockey_today).await?;
    if include_yesterday {
        let yest = nhl_mirror::list_games_for_date(state.db.pool(), &hockey_yesterday).await?;
        games.extend(yest.into_iter().filter(|g| state_str_is_live(&g.game_state)));
    }

    if games.is_empty() {
        return Ok(json_success(MatchDayResponse {
            date: hockey_today,
            games: Vec::new(),
            fantasy_teams: Vec::new(),
            summary: GamesSummaryResponse {
                total_games: 0,
                total_teams_playing: 0,
                team_players_count: Vec::new(),
            },
        }));
    }

    let MatchDayBundle { game_responses, fantasy_teams, summary } =
        assemble_match_day(&state, &games, league_id).await?;

    Ok(json_success(MatchDayResponse {
        date: hockey_today,
        games: game_responses,
        fantasy_teams,
        summary,
    }))
}

// ---------------------------------------------------------------------
// Basic mode (no fantasy overlays OR brief per-team overlays)
// ---------------------------------------------------------------------

async fn process_basic(
    state: &Arc<AppState>,
    date: &str,
    league_id: &str,
) -> Result<Json<ApiResponse<TodaysGamesResponse>>> {
    let games = nhl_mirror::list_games_for_date(state.db.pool(), date).await?;
    if games.is_empty() {
        return Ok(json_success(TodaysGamesResponse {
            date: date.to_string(),
            games: Vec::new(),
            summary: GamesSummaryResponse {
                total_games: 0,
                total_teams_playing: 0,
                team_players_count: Vec::new(),
            },
            fantasy_teams: None,
        }));
    }

    let nhl_teams = collect_nhl_teams(&games);
    let fantasy_teams = if league_id.is_empty() {
        Vec::new()
    } else {
        state
            .db
            .get_fantasy_teams_for_nhl_teams(
                &nhl_teams.iter().map(String::as_str).collect::<Vec<_>>(),
                league_id,
            )
            .await?
    };
    let nhl_team_players = index_players_by_nhl_team(&fantasy_teams);

    // Load per-player boxscore rows for every game in one query.
    let game_ids: Vec<i64> = games.iter().map(|g| g.game_id).collect();
    let player_stats =
        nhl_mirror::list_player_game_stats_for_games(state.db.pool(), &game_ids).await?;
    let by_game: HashMap<i64, Vec<PlayerGameStatRow>> = group_by_game(player_stats);

    let mut game_responses = Vec::with_capacity(games.len());
    for game in &games {
        let boxscore_rows = by_game.get(&game.game_id).cloned().unwrap_or_default();

        let home_team_players = build_basic_players(
            &state.nhl_client,
            &nhl_team_players,
            &game.home_team,
            &fantasy_teams,
            &boxscore_rows,
        );
        let away_team_players = build_basic_players(
            &state.nhl_client,
            &nhl_team_players,
            &game.away_team,
            &fantasy_teams,
            &boxscore_rows,
        );

        game_responses.push(game_response_from_row(
            &state.nhl_client,
            game,
            home_team_players,
            away_team_players,
        ));
    }

    let summary = summary_from_games(&games, &nhl_team_players);

    Ok(json_success(TodaysGamesResponse {
        date: date.to_string(),
        games: game_responses,
        summary,
        fantasy_teams: None,
    }))
}

// ---------------------------------------------------------------------
// Extended mode (adds form + playoff totals per player)
// ---------------------------------------------------------------------

async fn process_extended(
    state: &Arc<AppState>,
    date: &str,
    league_id: &str,
) -> Result<Json<ApiResponse<TodaysGamesResponse>>> {
    let mut games = nhl_mirror::list_games_for_date(state.db.pool(), date).await?;

    // Early-morning carry-over for yesterday's still-live games.
    let now_et = chrono::Utc::now().with_timezone(&chrono_tz::America::New_York);
    let hockey_today = now_et.format("%Y-%m-%d").to_string();
    let is_today = date == hockey_today;
    if is_today && now_et.time().format("%H").to_string().parse::<u32>().unwrap_or(12) < 12 {
        let hockey_yesterday = (now_et - chrono::Duration::days(1))
            .format("%Y-%m-%d")
            .to_string();
        let yest = nhl_mirror::list_games_for_date(state.db.pool(), &hockey_yesterday).await?;
        games.extend(yest.into_iter().filter(|g| state_str_is_live(&g.game_state)));
    }

    if games.is_empty() {
        return Ok(json_success(TodaysGamesResponse {
            date: date.to_string(),
            games: Vec::new(),
            summary: GamesSummaryResponse {
                total_games: 0,
                total_teams_playing: 0,
                team_players_count: Vec::new(),
            },
            fantasy_teams: Some(Vec::new()),
        }));
    }

    let MatchDayBundle {
        game_responses: match_game_responses,
        fantasy_teams,
        summary,
    } = assemble_match_day(state, &games, league_id).await?;

    // The TodaysGames endpoint uses a slightly different per-game DTO
    // (`GameResponse` with basic per-team player blocks). Build that
    // from the same data source as match day but without the extended
    // per-team form block inside the game response.
    let nhl_teams_raw = collect_nhl_teams(&games);
    let fantasy_teams_full = state
        .db
        .get_fantasy_teams_for_nhl_teams(
            &nhl_teams_raw.iter().map(String::as_str).collect::<Vec<_>>(),
            league_id,
        )
        .await?;
    let nhl_team_players = index_players_by_nhl_team(&fantasy_teams_full);

    let game_ids: Vec<i64> = games.iter().map(|g| g.game_id).collect();
    let by_game: HashMap<i64, Vec<PlayerGameStatRow>> = group_by_game(
        nhl_mirror::list_player_game_stats_for_games(state.db.pool(), &game_ids).await?,
    );

    let mut games_dto = Vec::with_capacity(match_game_responses.len());
    for game in &games {
        let boxscore_rows = by_game.get(&game.game_id).cloned().unwrap_or_default();
        let home_team_players = build_basic_players(
            &state.nhl_client,
            &nhl_team_players,
            &game.home_team,
            &fantasy_teams_full,
            &boxscore_rows,
        );
        let away_team_players = build_basic_players(
            &state.nhl_client,
            &nhl_team_players,
            &game.away_team,
            &fantasy_teams_full,
            &boxscore_rows,
        );
        games_dto.push(game_response_from_row(
            &state.nhl_client,
            game,
            home_team_players,
            away_team_players,
        ));
    }

    Ok(json_success(TodaysGamesResponse {
        date: date.to_string(),
        games: games_dto,
        summary,
        fantasy_teams: Some(fantasy_teams),
    }))
}

// ---------------------------------------------------------------------
// Match-day assembly shared between get_match_day and process_extended
// ---------------------------------------------------------------------

struct MatchDayBundle {
    game_responses: Vec<MatchDayGameResponse>,
    fantasy_teams: Vec<MatchDayFantasyTeamResponse>,
    summary: GamesSummaryResponse,
}

async fn assemble_match_day(
    state: &Arc<AppState>,
    games: &[NhlGameRow],
    league_id: &str,
) -> Result<MatchDayBundle> {
    let nhl_teams = collect_nhl_teams(games);
    let fantasy_teams = state
        .db
        .get_fantasy_teams_for_nhl_teams(
            &nhl_teams.iter().map(String::as_str).collect::<Vec<_>>(),
            league_id,
        )
        .await?;
    let nhl_team_players = index_players_by_nhl_team(&fantasy_teams);

    // Per-game boxscore rows, indexed by game_id.
    let game_ids: Vec<i64> = games.iter().map(|g| g.game_id).collect();
    let by_game: HashMap<i64, Vec<PlayerGameStatRow>> = group_by_game(
        nhl_mirror::list_player_game_stats_for_games(state.db.pool(), &game_ids).await?,
    );

    // Pre-fetch form + playoff totals for every rostered player on
    // tonight's slate in two batch queries — O(2) round-trips total.
    let all_rostered_ids: Vec<i64> = fantasy_teams
        .iter()
        .flat_map(|t| t.players.iter().map(|p| p.nhl_id))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let form_rows = nhl_mirror::list_player_form(state.db.pool(), &all_rostered_ids, 5).await?;
    let playoff_rows = nhl_mirror::list_player_playoff_totals(
        state.db.pool(),
        &all_rostered_ids,
        cfg_season() as i32,
    )
    .await?;
    let form_by_player: HashMap<i64, PlayerFormRow> =
        form_rows.into_iter().map(|r| (r.player_id, r)).collect();
    let playoff_by_player: HashMap<i64, PlayerPlayoffTotalsRow> =
        playoff_rows.into_iter().map(|r| (r.player_id, r)).collect();

    // Build MatchDayGameResponse per game.
    let mut game_responses = Vec::with_capacity(games.len());
    for game in games {
        game_responses.push(match_day_game_response(&state.nhl_client, game));
    }

    // Build FantasyPlayerExtendedResponse per (fantasy_team, player).
    let mut by_fantasy_team: HashMap<i64, Vec<FantasyPlayerExtendedResponse>> = HashMap::new();
    let mut processed: HashSet<(i64, i64)> = HashSet::new();
    for game in games {
        let state_live_or_done = state_str_is_live_or_done(&game.game_state);
        let boxscore_rows = by_game.get(&game.game_id).cloned().unwrap_or_default();
        for team_abbrev in [&game.home_team, &game.away_team] {
            push_extended_for_team(
                &state.nhl_client,
                team_abbrev,
                &nhl_team_players,
                &fantasy_teams,
                state_live_or_done,
                &boxscore_rows,
                &form_by_player,
                &playoff_by_player,
                &mut processed,
                &mut by_fantasy_team,
            );
        }
    }

    let mut fantasy_team_responses = Vec::new();
    for (team_id, mut players) in by_fantasy_team {
        let team_name = fantasy_teams
            .iter()
            .find(|t| t.team_id == team_id)
            .map(|t| t.team_name.clone())
            .unwrap_or_else(|| format!("Team {}", team_id));
        players.sort_by(|a, b| b.points.cmp(&a.points));
        let total = players.len();
        fantasy_team_responses.push(MatchDayFantasyTeamResponse {
            team_id,
            team_name,
            players_in_action: players,
            total_players_today: total,
        });
    }
    fantasy_team_responses.sort_by(|a, b| b.total_players_today.cmp(&a.total_players_today));

    let summary = summary_from_games(games, &nhl_team_players);

    Ok(MatchDayBundle {
        game_responses,
        fantasy_teams: fantasy_team_responses,
        summary,
    })
}

// ---------------------------------------------------------------------
// Row → DTO helpers
// ---------------------------------------------------------------------

fn collect_nhl_teams(games: &[NhlGameRow]) -> Vec<String> {
    let mut v = Vec::with_capacity(games.len() * 2);
    for g in games {
        v.push(g.home_team.clone());
        v.push(g.away_team.clone());
    }
    v
}

fn index_players_by_nhl_team(
    fantasy_teams: &[FantasyTeamInGame],
) -> HashMap<String, HashMap<String, Vec<PlayerInGame>>> {
    let mut out: HashMap<String, HashMap<String, Vec<PlayerInGame>>> = HashMap::new();
    for team in fantasy_teams {
        for player in &team.players {
            out.entry(player.nhl_team.clone())
                .or_default()
                .entry(team.team_name.clone())
                .or_default()
                .push(player.clone());
        }
    }
    out
}

fn group_by_game(rows: Vec<PlayerGameStatRow>) -> HashMap<i64, Vec<PlayerGameStatRow>> {
    let mut out: HashMap<i64, Vec<PlayerGameStatRow>> = HashMap::new();
    for r in rows {
        out.entry(r.game_id).or_default().push(r);
    }
    out
}

fn build_basic_players(
    nhl_client: &NhlClient,
    nhl_team_players: &HashMap<String, HashMap<String, Vec<PlayerInGame>>>,
    nhl_team: &str,
    fantasy_teams: &[FantasyTeamInGame],
    boxscore_rows: &[PlayerGameStatRow],
) -> Vec<FantasyPlayerResponse> {
    let mut out = Vec::new();
    let Some(fantasy_map) = nhl_team_players.get(nhl_team) else {
        return out;
    };
    let stats_by_player_id: HashMap<i64, &PlayerGameStatRow> =
        boxscore_rows.iter().map(|r| (r.player_id, r)).collect();

    for (fantasy_team_name, fantasy_players) in fantasy_map {
        let fantasy_team_id = fantasy_teams
            .iter()
            .find(|t| t.team_name == fantasy_team_name.as_str())
            .map(|t| t.team_id)
            .unwrap_or(0);
        for player in fantasy_players {
            let (goals, assists, points) =
                stats_by_player_id
                    .get(&player.nhl_id)
                    .map(|r| (r.goals, r.assists, r.points))
                    .unwrap_or((0, 0, 0));
            out.push(FantasyPlayerResponse {
                fantasy_team: fantasy_team_name.clone(),
                fantasy_team_id,
                player_name: player.player_name.clone(),
                position: player.position.clone(),
                nhl_id: player.nhl_id,
                image_url: nhl_client.get_player_image_url(player.nhl_id),
                goals,
                assists,
                points,
            });
        }
    }
    out.sort_by(|a, b| a.fantasy_team.cmp(&b.fantasy_team));
    out
}

#[allow(clippy::too_many_arguments)]
fn push_extended_for_team(
    nhl_client: &NhlClient,
    nhl_team: &str,
    nhl_team_players: &HashMap<String, HashMap<String, Vec<PlayerInGame>>>,
    fantasy_teams: &[FantasyTeamInGame],
    state_live_or_done: bool,
    boxscore_rows: &[PlayerGameStatRow],
    form_by_player: &HashMap<i64, PlayerFormRow>,
    playoff_by_player: &HashMap<i64, PlayerPlayoffTotalsRow>,
    processed: &mut HashSet<(i64, i64)>,
    by_fantasy_team: &mut HashMap<i64, Vec<FantasyPlayerExtendedResponse>>,
) {
    let Some(fantasy_map) = nhl_team_players.get(nhl_team) else {
        return;
    };
    let stats_by_player_id: HashMap<i64, &PlayerGameStatRow> =
        boxscore_rows.iter().map(|r| (r.player_id, r)).collect();

    for (fantasy_team_name, fantasy_players) in fantasy_map {
        let fantasy_team_id = fantasy_teams
            .iter()
            .find(|t| t.team_name == fantasy_team_name.as_str())
            .map(|t| t.team_id)
            .unwrap_or(0);
        for player in fantasy_players {
            if !processed.insert((fantasy_team_id, player.nhl_id)) {
                continue;
            }

            let (goals, assists, points) = if state_live_or_done {
                stats_by_player_id
                    .get(&player.nhl_id)
                    .map(|r| (r.goals, r.assists, r.points))
                    .unwrap_or((0, 0, 0))
            } else {
                (0, 0, 0)
            };

            let form = form_by_player.get(&player.nhl_id).map(|r| PlayerForm {
                games: r.games as usize,
                goals: r.goals as i32,
                assists: r.assists as i32,
                points: r.points as i32,
            });
            let time_on_ice = form_by_player
                .get(&player.nhl_id)
                .and_then(|r| r.latest_toi_seconds)
                .map(format_toi);

            let (playoff_goals, playoff_assists, playoff_points, playoff_games) = playoff_by_player
                .get(&player.nhl_id)
                .map(|r| (r.goals as i32, r.assists as i32, r.points as i32, r.games as i32))
                .unwrap_or((0, 0, 0, 0));

            by_fantasy_team
                .entry(fantasy_team_id)
                .or_default()
                .push(FantasyPlayerExtendedResponse {
                    fantasy_team: fantasy_team_name.clone(),
                    fantasy_team_id,
                    player_name: player.player_name.clone(),
                    position: player.position.clone(),
                    nhl_id: player.nhl_id,
                    nhl_team: nhl_team.to_string(),
                    image_url: nhl_client.get_player_image_url(player.nhl_id),
                    team_logo: nhl_client.get_team_logo_url(nhl_team),
                    goals,
                    assists,
                    points,
                    playoff_goals,
                    playoff_assists,
                    playoff_points,
                    playoff_games,
                    form,
                    time_on_ice,
                });
        }
    }
}

fn game_response_from_row(
    nhl_client: &NhlClient,
    row: &NhlGameRow,
    home_team_players: Vec<FantasyPlayerResponse>,
    away_team_players: Vec<FantasyPlayerResponse>,
) -> GameResponse {
    GameResponse {
        id: row.game_id as u32,
        home_team: row.home_team.clone(),
        away_team: row.away_team.clone(),
        start_time: row.start_time_utc.to_rfc3339(),
        venue: row.venue.clone().unwrap_or_default(),
        home_team_players,
        away_team_players,
        home_team_logo: nhl_client.get_team_logo_url(&row.home_team),
        away_team_logo: nhl_client.get_team_logo_url(&row.away_team),
        home_score: row.home_score,
        away_score: row.away_score,
        game_state: game_state_from_str(&row.game_state),
        period: format_period(row.period_number, row.period_type.as_deref()),
        series_status: deser_series_status(&row.series_status),
    }
}

fn match_day_game_response(nhl_client: &NhlClient, row: &NhlGameRow) -> MatchDayGameResponse {
    MatchDayGameResponse {
        id: row.game_id as u32,
        home_team: row.home_team.clone(),
        away_team: row.away_team.clone(),
        start_time: row.start_time_utc.to_rfc3339(),
        venue: row.venue.clone().unwrap_or_default(),
        home_team_logo: nhl_client.get_team_logo_url(&row.home_team),
        away_team_logo: nhl_client.get_team_logo_url(&row.away_team),
        home_score: row.home_score,
        away_score: row.away_score,
        game_state: game_state_from_str(&row.game_state),
        period: format_period(row.period_number, row.period_type.as_deref()),
        series_status: deser_series_status(&row.series_status),
    }
}

fn summary_from_games(
    games: &[NhlGameRow],
    nhl_team_players: &HashMap<String, HashMap<String, Vec<PlayerInGame>>>,
) -> GamesSummaryResponse {
    let mut teams: Vec<String> = games
        .iter()
        .flat_map(|g| [g.home_team.clone(), g.away_team.clone()])
        .collect();
    teams.sort();
    teams.dedup();

    let mut counts: Vec<TeamPlayerCountResponse> = teams
        .iter()
        .filter_map(|team| {
            let count = nhl_team_players
                .get(team)
                .map(|m| m.values().map(|v| v.len()).sum::<usize>())
                .unwrap_or(0);
            (count > 0).then(|| TeamPlayerCountResponse {
                nhl_team: team.clone(),
                player_count: count,
            })
        })
        .collect();
    counts.sort_by(|a, b| b.player_count.cmp(&a.player_count));

    GamesSummaryResponse {
        total_games: games.len(),
        total_teams_playing: teams.len(),
        team_players_count: counts,
    }
}

// ---------------------------------------------------------------------
// Formatters
// ---------------------------------------------------------------------

fn format_period(number: Option<i16>, period_type: Option<&str>) -> Option<String> {
    let num = number?;
    let label = match period_type {
        Some("REG") => "Period",
        Some("OT") => "OT",
        Some("SO") => "Shootout",
        Some(other) => other,
        None => "",
    };
    Some(format!("{} {}", num, label))
}

fn format_toi(seconds: i32) -> String {
    let m = seconds / 60;
    let s = seconds % 60;
    format!("{:02}:{:02}", m, s)
}

fn game_state_from_str(s: &str) -> GameState {
    GameState::from_str(s).unwrap_or_default()
}

fn state_str_is_live(s: &str) -> bool {
    matches!(s, "LIVE" | "CRIT")
}

fn state_str_is_live_or_done(s: &str) -> bool {
    matches!(s, "LIVE" | "CRIT" | "OFF" | "FINAL")
}

fn deser_series_status(
    raw: &Option<serde_json::Value>,
) -> Option<crate::api::dtos::SeriesStatusResponse> {
    let v = raw.as_ref()?;
    let series: SeriesStatus = serde_json::from_value(v.clone()).ok()?;
    Some(series.into())
}
