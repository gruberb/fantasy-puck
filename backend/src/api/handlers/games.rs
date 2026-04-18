use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use axum::{
    extract::{Query, State},
    Json,
};
use chrono::{DateTime, Timelike, Utc};
use futures::future::join_all;

use crate::api::dtos::*;
use crate::api::response::{json_success, ApiResponse};
use crate::api::routes::AppState;
use crate::error::Result;
use crate::models::fantasy::PlayerInGame;
use crate::models::nhl::{BoxscorePlayer, GameBoxscore};
use crate::utils::nhl::find_player_stats_by_name;
use crate::utils::api::{
    create_games_summary, get_fantasy_players_for_nhl_team, process_players_for_team,
};

pub async fn list_games(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<TodaysGamesResponse>>> {
    // league_id is optional for Game Center - if absent, just show NHL games without fantasy overlay
    let league_id = params.get("league_id").cloned().unwrap_or_default();
    let detail = params.get("detail").cloned().unwrap_or_default();

    let date = match params.get("date") {
        Some(date) => {
            // Validate date format YYYY-MM-DD
            if date.len() != 10 || !date.chars().all(|c| c == '-' || c.is_ascii_digit()) {
                return Err(crate::error::Error::Validation(
                    "Invalid date format. Use YYYY-MM-DD".into(),
                ));
            }
            date.to_string()
        }
        None => {
            return Err(crate::error::Error::Validation(
                "Date parameter is required (format: YYYY-MM-DD)".into(),
            ));
        }
    };

    // Fetch schedule for the specified date
    let schedule = state.nhl_client.get_schedule_by_date(&date).await?;

    if detail == "extended" && !league_id.is_empty() {
        process_games_extended(state, schedule, date, &league_id).await
    } else {
        process_games(state, schedule, date, &league_id).await
    }
}

async fn process_games(
    state: Arc<AppState>,
    schedule: crate::models::nhl::TodaySchedule,
    date: String,
    league_id: &str,
) -> Result<Json<ApiResponse<TodaysGamesResponse>>> {
    // Check if there are games for the requested date
    let mut games_for_date = Vec::new();

    for game_day in &schedule.game_week {
        if game_day.date == date {
            games_for_date = game_day.games.clone();
            break;
        }
    }

    if games_for_date.is_empty() {
        return Ok(json_success(TodaysGamesResponse {
            date,
            games: Vec::new(),
            summary: GamesSummaryResponse {
                total_games: 0,
                total_teams_playing: 0,
                team_players_count: Vec::new(),
            },
            fantasy_teams: None,
        }));
    }

    // Collect all NHL teams playing on this date
    let mut nhl_teams = Vec::new();
    for game in &games_for_date {
        nhl_teams.push(game.home_team.abbrev.as_str());
        nhl_teams.push(game.away_team.abbrev.as_str());
    }

    // Get fantasy teams - only if a league is specified
    let fantasy_teams = if league_id.is_empty() {
        Vec::new()
    } else {
        state.db
            .get_fantasy_teams_for_nhl_teams(&nhl_teams, league_id)
            .await?
    };

    let mut nhl_team_players: HashMap<String, HashMap<String, Vec<PlayerInGame>>> = HashMap::new();

    // Populate the mapping
    for team in &fantasy_teams {
        for player in &team.players {
            nhl_team_players
                .entry(player.nhl_team.clone())
                .or_default()
                .entry(team.team_name.clone())
                .or_default()
                .push(player.clone());
        }
    }

    // Create game responses
    let game_responses_futures = games_for_date.iter().map(|game| {
        let state_clone = Arc::clone(&state);
        let nhl_team_players_clone = nhl_team_players.clone();
        let fantasty_teams_clone = fantasy_teams.clone();

        async move {
            let home_team = &game.home_team.abbrev;
            let away_team = &game.away_team.abbrev;

            // Game start time in UTC: "2024-04-23T23:00:00Z"
            let game_time = game.start_time_utc.clone();

            // Get home team players with points data
            let mut home_team_players = get_fantasy_players_for_nhl_team(
                &state_clone.nhl_client,
                &nhl_team_players_clone,
                home_team,
                &fantasty_teams_clone,
            );
            // Get away team players with points data
            let mut away_team_players = get_fantasy_players_for_nhl_team(
                &state_clone.nhl_client,
                &nhl_team_players_clone,
                away_team,
                &fantasty_teams_clone,
            );

            // For completed or in-progress games, fetch player stats
            if game.game_state.is_live() || game.game_state.is_completed() {
                // Try to fetch boxscore for player stats
                if let Ok(boxscore) = state_clone.nhl_client.get_game_boxscore(game.id).await {
                    // Update home team players with points data
                    for player in &mut home_team_players {
                        let (goals, assists) =
                            find_player_stats_by_name(&boxscore, home_team, &player.player_name, Some(player.nhl_id));
                        player.goals = goals;
                        player.assists = assists;
                        player.points = goals + assists;
                    }

                    // Update away team players with points data
                    for player in &mut away_team_players {
                        let (goals, assists) =
                            find_player_stats_by_name(&boxscore, away_team, &player.player_name, Some(player.nhl_id));
                        player.goals = goals;
                        player.assists = assists;
                        player.points = goals + assists;
                    }
                }
            }

            // Get team logos
            let home_team_logo = state_clone.nhl_client.get_team_logo_url(home_team);
            let away_team_logo = state_clone.nhl_client.get_team_logo_url(away_team);

            // Get game scores if available
            let (home_score, away_score) = if let Some(game_score) = &game.game_score {
                (Some(game_score.home), Some(game_score.away))
            } else if game.game_state.is_live() || game.game_state.is_completed() {
                // For games in progress or completed but missing scores in the schedule data,
                // try to fetch scores from the gamecenter landing endpoint
                match state_clone.nhl_client.get_game_scores(game.id).await {
                    Ok((h_score, a_score)) => (h_score, a_score),
                    Err(_) => (None, None),
                }
            } else {
                (None, None)
            };

            // Get period information
            let period = game.period_descriptor.as_ref().map(|p| {
                let number = p.number.unwrap_or(0);
                let period_type = match p.period_type.as_deref() {
                    Some("REG") => "Period",
                    Some("OT") => "OT",
                    Some("SO") => "Shootout",
                    Some(other) => other,
                    None => "",
                };
                format!("{} {}", number, period_type)
            });

            GameResponse {
                id: game.id,
                home_team: home_team.to_string(),
                away_team: away_team.to_string(),
                start_time: game_time,
                venue: game.venue.default.clone(),
                home_team_players,
                away_team_players,
                home_team_logo,
                away_team_logo,
                home_score,
                away_score,
                game_state: game.game_state,
                period,
                series_status: game.series_status.clone().map(|s| s.into()),
            }
        }
    });

    let game_responses = join_all(game_responses_futures).await;

    // Create summary of players by NHL team
    let mut nhl_counts: Vec<TeamPlayerCountResponse> = nhl_teams
        .iter()
        .map(|&team| {
            let count = nhl_team_players
                .get(team)
                .map(|fantasy_map| fantasy_map.values().map(|players| players.len()).sum())
                .unwrap_or(0);
            TeamPlayerCountResponse {
                nhl_team: team.to_string(),
                player_count: count,
            }
        })
        .collect();

    // Remove duplicates and sort by count (descending)
    nhl_counts.sort_by(|a, b| b.player_count.cmp(&a.player_count));
    nhl_counts.dedup_by(|a, b| a.nhl_team == b.nhl_team);

    // Filter out teams with zero players
    let nhl_counts = nhl_counts
        .into_iter()
        .filter(|count| count.player_count > 0)
        .collect();

    Ok(json_success(TodaysGamesResponse {
        date,
        games: game_responses,
        summary: GamesSummaryResponse {
            total_games: games_for_date.len(),
            total_teams_playing: nhl_teams.len(),
            team_players_count: nhl_counts,
        },
        fantasy_teams: None,
    }))
}

async fn process_games_extended(
    state: Arc<AppState>,
    schedule: crate::models::nhl::TodaySchedule,
    date: String,
    league_id: &str,
) -> Result<Json<ApiResponse<TodaysGamesResponse>>> {
    let now_utc = chrono::Utc::now();
    let now_et = now_utc.with_timezone(&chrono_tz::America::New_York);
    let hockey_today = now_et.format("%Y-%m-%d").to_string();
    let is_today = date == hockey_today;

    // Cache key for extended games (separate from match_day cache).
    // Includes game_type() so regular-season and playoff payloads don't collide.
    let cache_key = format!(
        "games_extended:{}:{}:{}",
        league_id,
        crate::api::game_type(),
        date
    );

    // Check cache
    if let Some(cached) = state.db.cache().get_cached_response::<TodaysGamesResponse>(&cache_key).await? {
        // Check for potentially live games and refresh if needed
        let has_potential_live = cached.games.iter().any(|g| {
            if let Ok(time) = g.start_time.parse::<DateTime<Utc>>() {
                let diff = now_utc.signed_duration_since(time);
                diff.num_minutes() > -30 && diff.num_hours() < 4
            } else {
                false
            }
        });
        let has_live = cached.games.iter().any(|g| g.game_state.is_live());

        if !has_potential_live && !has_live {
            return Ok(json_success(cached));
        }
        // For live games, fall through and regenerate (the expensive part is
        // already cached in NHL client layer for game logs)
    }

    // Find games for the requested date
    let mut games_for_date = Vec::new();
    for game_day in &schedule.game_week {
        if game_day.date == date {
            games_for_date = game_day.games.clone();
            break;
        }
    }

    // Early morning: include yesterday's live games (only when viewing today)
    if is_today && now_et.time().hour() < 12 {
        let hockey_yesterday = (now_et - chrono::Duration::days(1))
            .format("%Y-%m-%d")
            .to_string();
        if let Ok(yesterday_schedule) = state.nhl_client.get_schedule_by_date(&hockey_yesterday).await {
            for game_day in &yesterday_schedule.game_week {
                if game_day.date == hockey_yesterday {
                    games_for_date.extend(
                        game_day.games.iter()
                            .filter(|g| g.game_state.is_live())
                            .cloned(),
                    );
                    break;
                }
            }
        }
    }

    if games_for_date.is_empty() {
        let empty = TodaysGamesResponse {
            date: date.clone(),
            games: Vec::new(),
            summary: GamesSummaryResponse {
                total_games: 0,
                total_teams_playing: 0,
                team_players_count: Vec::new(),
            },
            fantasy_teams: Some(Vec::new()),
        };
        let _ = state.db.cache().store_response(&cache_key, &date, &empty).await;
        return Ok(json_success(empty));
    }

    // Collect NHL teams playing
    let mut nhl_teams = Vec::new();
    for game in &games_for_date {
        nhl_teams.push(game.home_team.abbrev.as_str());
        nhl_teams.push(game.away_team.abbrev.as_str());
    }

    // Get fantasy teams
    let fantasy_teams = state.db
        .get_fantasy_teams_for_nhl_teams(&nhl_teams, league_id)
        .await?;

    let mut nhl_team_players: HashMap<String, HashMap<String, Vec<PlayerInGame>>> = HashMap::new();
    for team in &fantasy_teams {
        for player in &team.players {
            nhl_team_players
                .entry(player.nhl_team.clone())
                .or_default()
                .entry(team.team_name.clone())
                .or_default()
                .push(player.clone());
        }
    }

    // Pre-load boxscores for live/completed games in parallel — the
    // NhlClient semaphore (10 concurrent since v1.17) throttles
    // naturally, so this doesn't burst NHL. Was a sequential for-loop
    // + .await, which serialised the whole slate end-to-end on cold
    // loads.
    //
    // Also pre-load player game logs for every unique rostered skater
    // in tonight's slate so the downstream serial calls in
    // `process_players_for_team` become cache hits. On a 3-game slate
    // with ~60-100 unique players this collapses the serial NHL
    // round-trips into parallel ones, capped by the semaphore.
    //
    // Both prefetches run inside a single `tokio::join!` so the
    // shorter one (boxscores, usually) doesn't gate the longer one.
    let boxscore_futures = games_for_date
        .iter()
        .filter(|g| g.game_state.is_live() || g.game_state.is_completed())
        .map(|game| {
            let client = state.nhl_client.clone();
            let id = game.id;
            async move { (id, client.get_game_boxscore(id).await.ok()) }
        });
    let unique_player_ids: HashSet<i64> = games_for_date
        .iter()
        .flat_map(|g| [g.home_team.abbrev.as_str(), g.away_team.abbrev.as_str()])
        .filter_map(|abbrev| nhl_team_players.get(abbrev))
        .flat_map(|fantasy_map| fantasy_map.values())
        .flatten()
        .map(|p| p.nhl_id)
        .collect();
    let season_prefetch = crate::api::season();
    let game_type_prefetch = crate::api::game_type();
    let player_log_futures = unique_player_ids.into_iter().map(|nhl_id| {
        let client = state.nhl_client.clone();
        async move {
            let _ = client
                .get_player_game_log(nhl_id, &season_prefetch, game_type_prefetch)
                .await;
        }
    });
    let (boxscore_results, _) = tokio::join!(
        join_all(boxscore_futures),
        join_all(player_log_futures),
    );
    let mut boxscore_cache: HashMap<u32, Option<GameBoxscore>> =
        boxscore_results.into_iter().collect();

    // Retry failed boxscores for live games once, sequentially. The
    // parallel prefetch above can lose individual calls to a transient
    // NHL rate-limit; without a retry, live games silently render as
    // "0 pts" across every rostered skater. The retry runs serially so
    // it doesn't re-burst the endpoint.
    let live_game_ids_missing_boxscore: Vec<u32> = games_for_date
        .iter()
        .filter(|g| g.game_state.is_live())
        .filter_map(|g| match boxscore_cache.get(&g.id) {
            Some(Some(_)) => None,
            _ => Some(g.id),
        })
        .collect();
    for gid in live_game_ids_missing_boxscore {
        if let Ok(bx) = state.nhl_client.get_game_boxscore(gid).await {
            boxscore_cache.insert(gid, Some(bx));
        }
    }

    // Build game responses (with basic player data for per-game display)
    let mut game_responses = Vec::new();
    for game in &games_for_date {
        let home_team = &game.home_team.abbrev;
        let away_team = &game.away_team.abbrev;

        let mut home_team_players = get_fantasy_players_for_nhl_team(
            &state.nhl_client,
            &nhl_team_players,
            home_team,
            &fantasy_teams,
        );
        let mut away_team_players = get_fantasy_players_for_nhl_team(
            &state.nhl_client,
            &nhl_team_players,
            away_team,
            &fantasy_teams,
        );

        // Update basic player stats from boxscores
        if game.game_state.is_live() || game.game_state.is_completed() {
            if let Some(Some(boxscore)) = boxscore_cache.get(&game.id) {
                for player in &mut home_team_players {
                    let (goals, assists) = find_player_stats_by_name(boxscore, home_team, &player.player_name, Some(player.nhl_id));
                    player.goals = goals;
                    player.assists = assists;
                    player.points = goals + assists;
                }
                for player in &mut away_team_players {
                    let (goals, assists) = find_player_stats_by_name(boxscore, away_team, &player.player_name, Some(player.nhl_id));
                    player.goals = goals;
                    player.assists = assists;
                    player.points = goals + assists;
                }
            }
        }

        let home_team_logo = state.nhl_client.get_team_logo_url(home_team);
        let away_team_logo = state.nhl_client.get_team_logo_url(away_team);

        let (home_score, away_score) = if let Some(game_score) = &game.game_score {
            (Some(game_score.home), Some(game_score.away))
        } else if game.game_state.is_live() || game.game_state.is_completed() {
            match state.nhl_client.get_game_scores(game.id).await {
                Ok((h, a)) => (h, a),
                Err(_) => (None, None),
            }
        } else {
            (None, None)
        };

        // Live-game fallback: if neither the schedule nor the scores
        // endpoint yielded a score (typically because both hit NHL rate
        // limits), derive the score from the boxscore we already have
        // cached. Sum of skater goals per side = team goals. Avoids the
        // UI rendering "just the time" for a live game we know is live.
        let (home_score, away_score) = if home_score.is_none() && away_score.is_none() {
            match boxscore_cache.get(&game.id) {
                Some(Some(bx)) => {
                    let sum = |players: &[BoxscorePlayer]| -> i32 {
                        players.iter().map(|p| p.goals.unwrap_or(0)).sum()
                    };
                    let h = sum(&bx.player_by_game_stats.home_team.forwards)
                        + sum(&bx.player_by_game_stats.home_team.defense);
                    let a = sum(&bx.player_by_game_stats.away_team.forwards)
                        + sum(&bx.player_by_game_stats.away_team.defense);
                    (Some(h), Some(a))
                }
                _ => (home_score, away_score),
            }
        } else {
            (home_score, away_score)
        };

        let period = game.period_descriptor.as_ref().map(|p| {
            let number = p.number.unwrap_or(0);
            let period_type = match p.period_type.as_deref() {
                Some("REG") => "Period",
                Some("OT") => "OT",
                Some("SO") => "Shootout",
                Some(other) => other,
                None => "",
            };
            format!("{} {}", number, period_type)
        });

        game_responses.push(GameResponse {
            id: game.id,
            home_team: home_team.to_string(),
            away_team: away_team.to_string(),
            start_time: game.start_time_utc.clone(),
            venue: game.venue.default.clone(),
            home_team_players,
            away_team_players,
            home_team_logo,
            away_team_logo,
            home_score,
            away_score,
            game_state: game.game_state,
            period,
            series_status: game.series_status.clone().map(|s| s.into()),
        });
    }

    // Build extended fantasy team data (playoff stats, form, TOI).
    // Game-log cache is already warm from the tokio::join! above —
    // every `get_player_game_log` call inside `process_players_for_team`
    // will hit the NhlClient cache instead of burning a round-trip.
    let season = crate::api::season();
    let game_type = crate::api::game_type();
    let form_games = 5;

    let mut all_players_by_fantasy_team: HashMap<i64, Vec<FantasyPlayerExtendedResponse>> = HashMap::new();
    let mut processed_players = HashSet::new();

    for game in &games_for_date {
        let home_team = &game.home_team.abbrev;
        let away_team = &game.away_team.abbrev;

        process_players_for_team(
            home_team, game.id, game.game_state, &state, &nhl_team_players,
            &fantasy_teams, &season, game_type, form_games,
            &mut processed_players, &mut all_players_by_fantasy_team, &boxscore_cache,
        ).await?;

        process_players_for_team(
            away_team, game.id, game.game_state, &state, &nhl_team_players,
            &fantasy_teams, &season, game_type, form_games,
            &mut processed_players, &mut all_players_by_fantasy_team, &boxscore_cache,
        ).await?;
    }

    // Build fantasy team responses
    let mut fantasy_team_responses = Vec::new();
    for (team_id, players) in all_players_by_fantasy_team {
        let team_name = fantasy_teams
            .iter()
            .find(|t| t.team_id == team_id)
            .map(|t| t.team_name.clone())
            .unwrap_or_else(|| format!("Team {}", team_id));

        let mut sorted_players = players;
        sorted_players.sort_by(|a, b| b.points.cmp(&a.points));

        fantasy_team_responses.push(MatchDayFantasyTeamResponse {
            team_id,
            team_name,
            players_in_action: sorted_players.clone(),
            total_players_today: sorted_players.len(),
        });
    }
    fantasy_team_responses.sort_by(|a, b| b.total_players_today.cmp(&a.total_players_today));

    let summary = create_games_summary(&games_for_date, &nhl_team_players);

    let response = TodaysGamesResponse {
        date: date.clone(),
        games: game_responses,
        summary,
        fantasy_teams: Some(fantasy_team_responses),
    };

    // Cache the response
    let _ = state.db.cache().store_response(&cache_key, &date, &response).await;

    Ok(json_success(response))
}

pub async fn get_match_day(
    State(state): State<Arc<AppState>>,
    Query(league_params): Query<LeagueParams>,
) -> Result<Json<ApiResponse<MatchDayResponse>>> {
    let league_id = &league_params.league_id;

    let now_utc = chrono::Utc::now();
    let now = now_utc.with_timezone(&chrono_tz::America::New_York);
    let hockey_today = now.format("%Y-%m-%d").to_string();

    // Create cache key for today's match day (scoped by league and game_type)
    let cache_key = format!(
        "match_day:{}:{}:{}",
        league_id,
        crate::api::game_type(),
        hockey_today
    );

    // Check if we have a valid cached response
    if let Some(cached_response) = state.db.cache().get_cached_response::<MatchDayResponse>(&cache_key).await? {
        // First check for games that are scheduled to start around now
        // This helps catch games transitioning from FUT to LIVE
        let now_utc = chrono::Utc::now();
        let has_potential_live_games = cached_response.games.iter().any(|g| {
            let start_time = g.start_time.parse::<DateTime<Utc>>().ok();
            if let Some(time) = start_time {
                // Consider games that started up to 4 hours ago or will start in the next 30 minutes
                let time_diff = now_utc.signed_duration_since(time);
                time_diff.num_minutes() > -30 && time_diff.num_hours() < 4
            } else {
                false
            }
        });

        // Always check for real-time updates if any games could be live
        if has_potential_live_games {
            // Force a check for live updates
            let updated_response = update_live_game_data(cached_response, &state).await?;
            return Ok(json_success(updated_response));
        }

        // Check if any games are marked as in progress in the cache
        let has_live_games = cached_response.games.iter().any(|g| g.game_state.is_live());
        if has_live_games {
            // Only fetch and update the live parts of the response
            let updated_response = update_live_game_data(cached_response, &state).await?;
            return Ok(json_success(updated_response));
        } else {
            // No live games, cached response is fully valid
            return Ok(json_success(cached_response));
        }
    }
    // No cached response, continue with original implementation to generate a full response

    // If it's early morning in hockey time (like 1am-noon), we're probably
    // still interested in "yesterday's" games that might be finishing up
    let early_morning_hours = now.time().hour() < 12;

    // Get yesterday's date in hockey timezone
    let hockey_yesterday = (now - chrono::Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();

    // Season and game type for playoff stats
    let season = crate::api::season();
    let game_type = crate::api::game_type();
    let form_games = 5;

    // Always fetch today's schedule
    let today_schedule = state.nhl_client.get_schedule_by_date(&hockey_today).await?;

    // Fetch yesterday's schedule if we're in early morning hours
    let yesterday_schedule = if early_morning_hours {
        state
            .nhl_client
            .get_schedule_by_date(&hockey_yesterday)
            .await?
    } else {
        today_schedule.clone() // Just use today's schedule as a placeholder
    };

    // Find games for hockey today
    let mut games_for_date = Vec::new();
    for game_day in &today_schedule.game_week {
        if game_day.date == hockey_today {
            games_for_date = game_day.games.clone();
            break;
        }
    }

    // If it's early morning, include yesterday's games that are in progress
    if early_morning_hours {
        for game_day in &yesterday_schedule.game_week {
            if game_day.date == hockey_yesterday {
                // Get games that are still in progress
                let live_yesterday_games = game_day
                    .games
                    .iter()
                    .filter(|game| game.game_state.is_live())
                    .cloned()
                    .collect::<Vec<_>>();

                games_for_date.extend(live_yesterday_games);
                break;
            }
        }
    }

    // Continue with the rest of your function's code as is...
    if games_for_date.is_empty() {
        let empty_response = MatchDayResponse {
            date: hockey_today.clone(),
            games: Vec::new(),
            fantasy_teams: Vec::new(),
            summary: GamesSummaryResponse {
                total_games: 0,
                total_teams_playing: 0,
                team_players_count: Vec::new(),
            },
        };

        // Cache the empty response too
        state
            .db
            .cache()
            .store_response(&cache_key, &hockey_today, &empty_response)
            .await?;

        return Ok(json_success(empty_response));
    }

    // 1. First collect all NHL teams playing today
    let mut nhl_teams = Vec::new();
    for game in &games_for_date {
        nhl_teams.push(game.home_team.abbrev.as_str());
        nhl_teams.push(game.away_team.abbrev.as_str());
    }

    // 2. Get fantasy teams with players from these NHL teams
    let fantasy_teams = state
        .db
        .get_fantasy_teams_for_nhl_teams(&nhl_teams, league_id)
        .await?;

    // 3. Organize players by NHL team and fantasy team
    let mut nhl_team_players: HashMap<String, HashMap<String, Vec<PlayerInGame>>> = HashMap::new();
    for team in &fantasy_teams {
        for player in &team.players {
            nhl_team_players
                .entry(player.nhl_team.clone())
                .or_default()
                .entry(team.team_name.clone())
                .or_default()
                .push(player.clone());
        }
    }

    // 4. Create game responses without players first
    let mut game_responses = Vec::new();
    for game in &games_for_date {
        let home_team = &game.home_team.abbrev;
        let away_team = &game.away_team.abbrev;
        let game_time = game.start_time_utc.clone();

        // Get team logos
        let home_team_logo = state.nhl_client.get_team_logo_url(home_team);
        let away_team_logo = state.nhl_client.get_team_logo_url(away_team);

        // Get game scores if available
        let (home_score, away_score) = if let Some(game_score) = &game.game_score {
            (Some(game_score.home), Some(game_score.away))
        } else if game.game_state.is_live() || game.game_state.is_completed() {
            // For games in progress or completed but missing scores in the schedule data,
            // try to fetch scores from the gamecenter landing endpoint
            match state.nhl_client.get_game_scores(game.id).await {
                Ok((h_score, a_score)) => (h_score, a_score),
                Err(_) => (None, None),
            }
        } else {
            (None, None)
        };

        // Get period information
        let period = game.period_descriptor.as_ref().map(|p| {
            let number = p.number.unwrap_or(0);
            let period_type = match p.period_type.as_deref() {
                Some("REG") => "Period",
                Some("OT") => "OT",
                Some("SO") => "Shootout",
                Some(other) => other,
                None => "",
            };
            format!("{} {}", number, period_type)
        });

        game_responses.push(MatchDayGameResponse {
            id: game.id,
            home_team: home_team.to_string(),
            away_team: away_team.to_string(),
            start_time: game_time,
            venue: game.venue.default.clone(),
            home_team_logo,
            away_team_logo,
            home_score,
            away_score,
            game_state: game.game_state,
            period,
            series_status: game.series_status.clone().map(|s| s.into()),
        });
    }

    // 5. Process fantasy players playing today with stats and form
    // Track all players regardless of NHL team
    let mut all_players_by_fantasy_team: HashMap<i64, Vec<FantasyPlayerExtendedResponse>> =
        HashMap::new();

    // Set of (fantasy_team_id, nhl_id) to prevent duplicate players
    let mut processed_players = HashSet::new();

    // Cache for boxscores to avoid duplicate requests
    let mut boxscore_cache: HashMap<u32, Option<GameBoxscore>> = HashMap::new();

    // Fix the boxscore loading issue: pre-load boxscores for ALL live or completed games
    for game in &games_for_date {
        if game.game_state.is_live() || game.game_state.is_completed() {
            // Fetch boxscore and cache it (even if it fails - store None)
            let boxscore = state.nhl_client.get_game_boxscore(game.id).await.ok();
            boxscore_cache.insert(game.id, boxscore);
        }
    }

    // Process all players from all teams playing today
    for game in &games_for_date {
        let home_team = &game.home_team.abbrev;
        let away_team = &game.away_team.abbrev;
        let game_id = game.id;
        let game_state = game.game_state;

        // Process home team players
        process_players_for_team(
            home_team,
            game_id,
            game_state,
            &state,
            &nhl_team_players,
            &fantasy_teams,
            &season,
            game_type,
            form_games,
            &mut processed_players,
            &mut all_players_by_fantasy_team,
            &boxscore_cache,
        )
        .await?;

        // Process away team players
        process_players_for_team(
            away_team,
            game_id,
            game_state,
            &state,
            &nhl_team_players,
            &fantasy_teams,
            &season,
            game_type,
            form_games,
            &mut processed_players,
            &mut all_players_by_fantasy_team,
            &boxscore_cache,
        )
        .await?;
    }

    // 6. Create fantasy team responses
    let mut fantasy_team_responses = Vec::new();
    for (team_id, players) in all_players_by_fantasy_team {
        let team_name = fantasy_teams
            .iter()
            .find(|t| t.team_id == team_id)
            .map(|t| t.team_name.clone())
            .unwrap_or_else(|| format!("Team {}", team_id));

        // Sort players by points (descending)
        let mut sorted_players = players;
        sorted_players.sort_by(|a, b| b.points.cmp(&a.points));

        fantasy_team_responses.push(MatchDayFantasyTeamResponse {
            team_id,
            team_name,
            players_in_action: sorted_players.clone(),
            total_players_today: sorted_players.len(),
        });
    }

    // Sort fantasy teams by number of players (descending)
    fantasy_team_responses.sort_by(|a, b| b.total_players_today.cmp(&a.total_players_today));

    // 7. Create summary information
    let summary = create_games_summary(&games_for_date, &nhl_team_players);

    // Create the final response
    let response = MatchDayResponse {
        date: hockey_today.clone(),
        games: game_responses,
        fantasy_teams: fantasy_team_responses,
        summary,
    };

    // Cache the response before returning
    state
        .db
        .cache()
        .store_response(&cache_key, &hockey_today, &response)
        .await?;

    // Return final response
    Ok(json_success(response))
}

// New helper function to update only the live parts of a cached response
async fn update_live_game_data(
    mut cached_response: MatchDayResponse,
    state: &Arc<AppState>,
) -> Result<MatchDayResponse> {
    // Keep track of game IDs that are being updated
    let mut live_game_ids = Vec::new();
    let mut cache_updated = false;

    // Update game states, scores, and periods for all games that could be live
    for game in &mut cached_response.games {
        // Always check current game state from NHL API
        if let Ok(Some(game_data)) = state.nhl_client.get_game_data(game.id).await {
            // Update game state if it has changed
            if game.game_state != game_data.game_state {
                game.game_state = game_data.game_state;
                cache_updated = true;
            }

            // If game is now live, update other data
            if game.game_state.is_live() {
                live_game_ids.push(game.id);

                // Update scores
                if let Some(home_score) = game_data.home_score {
                    if game.home_score != Some(home_score) {
                        game.home_score = Some(home_score);
                        cache_updated = true;
                    }
                }

                if let Some(away_score) = game_data.away_score {
                    if game.away_score != Some(away_score) {
                        game.away_score = Some(away_score);
                        cache_updated = true;
                    }
                }

                // Update period information
                if let Some(period_info) = game_data.period {
                    if game.period != Some(period_info.clone()) {
                        game.period = Some(period_info);
                        cache_updated = true;
                    }
                }
            }
        } else {
            // Fallback to old logic if get_game_data fails
            if game.game_state.is_live() {
                live_game_ids.push(game.id);

                // Update scores
                if let Ok((home_score, away_score)) =
                    state.nhl_client.get_game_scores(game.id).await
                {
                    game.home_score = home_score;
                    game.away_score = away_score;
                    cache_updated = true;
                }

                // Update period information
                if let Ok(Some(period_info)) = state.nhl_client.get_period_info(game.id).await {
                    game.period = Some(period_info);
                    cache_updated = true;
                }
            }
        }
    }

    // No live games to update
    if live_game_ids.is_empty() {
        // Still save the cache if states were updated (e.g., games went from FUT to LIVE)
        if cache_updated {
            // Create cache key for today's match day
            let now = chrono::Utc::now() + chrono::Duration::hours(-4); // NHL timezone offset
            let hockey_today = now.format("%Y-%m-%d").to_string();
            // Note: We don't know the league_id here, so we use the date from the cached response
            let cache_key = format!("match_day:{}", hockey_today);

            // Update the cache with the new data
            state
                .db
                .cache()
                .store_response(&cache_key, &hockey_today, &cached_response)
                .await?;
        }
        return Ok(cached_response);
    }

    // Create a cache for boxscores to avoid duplicate requests
    let mut boxscore_cache = HashMap::new();

    // Fetch boxscores for all live games
    for game_id in &live_game_ids {
        let boxscore = state.nhl_client.get_game_boxscore(*game_id).await.ok();
        boxscore_cache.insert(*game_id, boxscore);
    }

    // Update player stats for each fantasy team
    for team in &mut cached_response.fantasy_teams {
        for player in &mut team.players_in_action {
            // Find which game this player is playing in
            let player_game = cached_response.games.iter().find(|g| {
                g.game_state.is_live()
                    && (g.home_team == player.nhl_team || g.away_team == player.nhl_team)
            });

            if let Some(game) = player_game {
                if let Some(Some(boxscore)) = boxscore_cache.get(&game.id) {
                    // Update player stats from boxscore
                    let before_goals = player.goals;
                    let before_assists = player.assists;
                    update_player_stats_from_boxscore(player, boxscore);

                    // Check if stats were updated
                    if player.goals != before_goals || player.assists != before_assists {
                        cache_updated = true;
                    }
                }
            }
        }
    }

    // Store the updated response in the cache if changes were made
    if cache_updated {
        // Create cache key for today's match day
        let now = chrono::Utc::now() + chrono::Duration::hours(-4); // NHL timezone offset
        let hockey_today = now.format("%Y-%m-%d").to_string();
        let cache_key = format!("match_day:{}", hockey_today);

        // Update the cache with the new data
        state
            .db
            .cache()
            .store_response(&cache_key, &hockey_today, &cached_response)
            .await?;
    }

    Ok(cached_response)
}

// Helper function to update a player's stats from a boxscore
fn update_player_stats_from_boxscore(
    player: &mut FantasyPlayerExtendedResponse,
    boxscore: &GameBoxscore,
) {
    // Find the player in the boxscore stats by player ID
    let player_stats = find_player_by_id(boxscore, player.nhl_id as u32);

    if let Some(stats) = player_stats {
        // Update player stats
        player.goals = stats.goals.unwrap_or(0);
        player.assists = stats.assists.unwrap_or(0);
        player.points = stats.points.unwrap_or(0);
    }
}

// Helper function to find a player by ID in the boxscore
fn find_player_by_id(boxscore: &GameBoxscore, player_id: u32) -> Option<&BoxscorePlayer> {
    // Check home team players
    boxscore
        .player_by_game_stats
        .home_team
        .forwards
        .iter()
        .chain(boxscore.player_by_game_stats.home_team.defense.iter())
        .chain(boxscore.player_by_game_stats.home_team.goalies.iter())
        // Check away team players if not found in home team
        .chain(boxscore.player_by_game_stats.away_team.forwards.iter())
        .chain(boxscore.player_by_game_stats.away_team.defense.iter())
        .chain(boxscore.player_by_game_stats.away_team.goalies.iter())
        .find(|p| p.player_id == player_id as i32)
}
