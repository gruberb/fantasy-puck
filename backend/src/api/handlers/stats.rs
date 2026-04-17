use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{Query, State},
    Json,
};
use futures::future::join_all;

use crate::api::dtos::*;
use crate::api::response::{json_success, ApiResponse};
use crate::api::routes::AppState;
use crate::error::Result;
use crate::models::nhl::Player;

/// Endpoint to fetch top skaters.
pub async fn get_top_skaters(
    State(state): State<Arc<AppState>>,
    Query(params): Query<TopSkatersParams>,
) -> Result<Json<ApiResponse<Vec<ConsolidatedPlayerStats>>>> {
    // Use parameters from the query, with defaults if not provided
    let season = &params.season;
    let game_type = params.game_type;
    let limit = params.limit as usize;
    let include_form = params.include_form;
    let form_games = params.form_games;

    // Build fantasy team mapping if league_id is provided
    let mut fantasy_mapping: HashMap<i64, FantasyTeamInfo> = HashMap::new();
    if let Some(ref league_id) = params.league_id {
        let fantasy_players_groups = state.db.get_nhl_teams_and_players(league_id).await?;
        for group in fantasy_players_groups {
            for player in group.players {
                fantasy_mapping.insert(
                    player.nhl_id,
                    FantasyTeamInfo {
                        team_id: player.fantasy_team_id,
                        team_name: player.fantasy_team_name,
                    },
                );
            }
        }
    }

    // Playoffs: skater-stats-leaders is empty until games are played, so source
    // the pool from the 16 playoff team rosters instead. Stats fields are zero
    // (callers search by name/team/position, not by stat values).
    if game_type == 3 {
        let pool = crate::utils::player_pool::fetch_playoff_roster_pool(
            &state.nhl_client,
            *season,
        )
        .await?;
        let players = pool
            .into_iter()
            .map(|(id, (name, position, team_abbrev, headshot))| {
                let (first_name, last_name) = split_name(&name);
                let mut stats = HashMap::new();
                stats.insert("points".to_string(), 0);
                ConsolidatedPlayerStats {
                    id,
                    first_name,
                    last_name,
                    sweater_number: None,
                    headshot,
                    team_abbrev: team_abbrev.clone(),
                    team_name: team_abbrev.clone(),
                    team_logo: format!(
                        "https://assets.nhle.com/logos/nhl/svg/{}_light.svg",
                        team_abbrev
                    ),
                    position,
                    stats,
                    fantasy_team: fantasy_mapping.get(&id).cloned(),
                    form: None,
                }
            })
            .take(limit)
            .collect::<Vec<_>>();
        return Ok(json_success(players));
    }

    match state.nhl_client.get_skater_stats(season, game_type).await {
        Ok(stats) => {
            // Create a HashMap to store unique players with their stats
            let mut players_map: HashMap<i64, ConsolidatedPlayerStats> = HashMap::new();

            // Process all stat categories and merge them into the players_map
            process_stat_category(&mut players_map, stats.goals_sh, "goalsSh");
            process_stat_category(&mut players_map, stats.plus_minus, "plusMinus");
            process_stat_category(&mut players_map, stats.assists, "assists");
            process_stat_category(&mut players_map, stats.goals_pp, "goalsPp");
            process_stat_category(&mut players_map, stats.faceoff_leaders, "faceoffPct");
            process_stat_category(&mut players_map, stats.penalty_mins, "penaltyMins");
            process_stat_category(&mut players_map, stats.goals, "goals");
            process_stat_category(&mut players_map, stats.points, "points");
            process_stat_category(&mut players_map, stats.toi, "toi");

            // Add fantasy team information
            for (player_id, stats) in &mut players_map {
                if let Some(fantasy_info) = fantasy_mapping.get(player_id) {
                    stats.fantasy_team = Some(fantasy_info.clone());
                }
            }

            // If form data is requested, fetch it for each player
            if include_form {
                let form_futures = players_map
                    .keys()
                    .map(|player_id| {
                        let state_clone = Arc::clone(&state);
                        let season_clone = *season;
                        let player_id = *player_id;

                        async move {
                            match state_clone
                                .nhl_client
                                .get_player_form(player_id, &season_clone, game_type, form_games)
                                .await
                            {
                                Ok((goals, assists, points)) => (
                                    player_id,
                                    Some(PlayerForm {
                                        games: form_games,
                                        goals,
                                        assists,
                                        points,
                                    }),
                                ),
                                Err(_) => (player_id, None),
                            }
                        }
                    })
                    .collect::<Vec<_>>();

                // Execute all form fetches concurrently
                let form_results = join_all(form_futures).await;

                // Update player stats with form data
                for (player_id, form) in form_results {
                    if let Some(player_stats) = players_map.get_mut(&player_id) {
                        player_stats.form = form;
                    }
                }
            }

            // Convert HashMap to Vec and take only the requested number of players
            let mut players: Vec<ConsolidatedPlayerStats> = players_map.into_values().collect();

            // Sort by points (highest first) and limit the results
            players.sort_by(|a, b| {
                b.stats
                    .get("points")
                    .unwrap_or(&0)
                    .cmp(a.stats.get("points").unwrap_or(&0))
            });
            let limited_players = players.into_iter().take(limit).collect();

            Ok(json_success(limited_players))
        }
        Err(e) => Err(crate::error::Error::NhlApi(format!(
            "Failed to fetch skater stats: {}",
            e
        ))),
    }
}

/// Split a "First Last" (or "First Middle Last") name into (first, last).
fn split_name(full: &str) -> (String, String) {
    let trimmed = full.trim();
    match trimmed.rsplit_once(' ') {
        Some((first, last)) => (first.to_string(), last.to_string()),
        None => (trimmed.to_string(), String::new()),
    }
}

// Helper function to process a stat category and merge it into the players map
fn process_stat_category(
    players_map: &mut HashMap<i64, ConsolidatedPlayerStats>,
    players: Vec<Player>,
    category: &str,
) {
    for player in players {
        let player_id = player.id as i64;

        // Get or create player stats
        let player_stats = players_map.entry(player_id).or_insert_with(|| {
            // Initialize a new player with basic info
            ConsolidatedPlayerStats {
                id: player_id,
                first_name: player
                    .first_name
                    .get("default")
                    .unwrap_or(&String::new())
                    .clone(),
                last_name: player
                    .last_name
                    .get("default")
                    .unwrap_or(&String::new())
                    .clone(),
                sweater_number: player.sweater_number,
                headshot: format!("https://assets.nhle.com/mugs/nhl/latest/{}.png", player_id),
                team_abbrev: player.team_abbrev.clone(),
                team_name: player.team_abbrev.clone(), // This might need to be improved
                team_logo: format!(
                    "https://assets.nhle.com/logos/nhl/svg/{}_light.svg",
                    player.team_abbrev
                ),
                position: player.position.clone(),
                stats: HashMap::new(),
                fantasy_team: None,
                form: None,
            }
        });

        // Add or update the specific stat value
        player_stats
            .stats
            .insert(category.to_string(), player.value as i32);
    }
}
