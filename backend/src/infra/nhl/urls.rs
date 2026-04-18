use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::api::dtos::{
    FantasyPlayerExtendedResponse, FantasyPlayerResponse, GamesSummaryResponse,
    PlayerForm, TeamPlayerCountResponse,
};
use crate::api::routes::AppState;
use crate::Error;
use crate::error::Result;
use crate::domain::models::fantasy::{FantasyTeamInGame, PlayerInGame};
use crate::domain::models::nhl::{GameBoxscore, GameState};
use crate::domain::services::nhl_stats::{calculate_form_from_game_log, find_player_stats_by_name};

pub fn parse_date_param(date: String) -> Result<String> {
    match date.len() {
        10 if date.chars().all(|c| c == '-' || c.is_ascii_digit()) => Ok(date),
        _ => Err(Error::Validation(
            "Invalid date format. Use YYYY-MM-DD".into(),
        )),
    }
}

/// Helper function to get fantasy players for an NHL team from the pre-aggregated map.
pub fn get_fantasy_players_for_nhl_team(
    nhl_client: &crate::infra::nhl::client::NhlClient,
    nhl_team_players: &HashMap<String, HashMap<String, Vec<PlayerInGame>>>,
    nhl_team: &str,
    fantasy_teams: &[crate::domain::models::fantasy::FantasyTeamInGame],
) -> Vec<FantasyPlayerResponse> {
    let mut players = Vec::new();
    if let Some(fantasy_map) = nhl_team_players.get(nhl_team) {
        for (fantasy_team, fantasy_players) in fantasy_map {
            let fantasy_team_id = fantasy_teams
                .iter()
                .find(|t| t.team_name == fantasy_team.as_str())
                .map(|t| t.team_id)
                .unwrap_or(0);

            for player in fantasy_players {
                players.push(FantasyPlayerResponse {
                    fantasy_team: fantasy_team.clone(),
                    fantasy_team_id,
                    player_name: player.player_name.clone(),
                    position: player.position.clone(),
                    nhl_id: player.nhl_id,
                    image_url: nhl_client.get_player_image_url(player.nhl_id),
                    goals: 0,   // Initialize with zero
                    assists: 0, // Initialize with zero
                    points: 0,  // Initialize with zero
                                // team_logo field needs to be added here if we want it,
                                // but it requires NhlClient access or passing it in.
                                // Let's defer adding the logo until we refactor how NhlClient is accessed or passed.
                });
            }
        }
    }

    // Sort by fantasy team name
    players.sort_by(|a, b| a.fantasy_team.cmp(&b.fantasy_team));
    players
}

/// Helper function to process players for a specific team in the context of match day.
pub async fn process_players_for_team(
    nhl_team: &str,
    game_id: u32,
    game_state: GameState,
    state: &Arc<AppState>,
    nhl_team_players: &HashMap<String, HashMap<String, Vec<PlayerInGame>>>,
    fantasy_teams: &[FantasyTeamInGame],
    season: &u32,
    game_type: u8,
    form_games: usize,
    processed_players: &mut HashSet<(i64, i64)>,
    all_players_by_fantasy_team: &mut HashMap<i64, Vec<FantasyPlayerExtendedResponse>>,
    boxscore_cache: &HashMap<u32, Option<GameBoxscore>>,
) -> Result<()> {
    // Get fantasy players for this NHL team
    if let Some(fantasy_map) = nhl_team_players.get(nhl_team) {
        for (fantasy_team_name, fantasy_players) in fantasy_map {
            let fantasy_team_id = fantasy_teams
                .iter()
                .find(|t| t.team_name == fantasy_team_name.as_str())
                .map(|t| t.team_id)
                .unwrap_or(0);
            for player in fantasy_players {
                // Skip if we've already processed this player
                if !processed_players.insert((fantasy_team_id, player.nhl_id)) {
                    continue;
                }

                tracing::info!(
                    "Processing player: {} (NHL ID: {})",
                    player.player_name,
                    player.nhl_id
                );
                // Get player image URL
                let image_url = state.nhl_client.get_player_image_url(player.nhl_id);
                let team_logo = state.nhl_client.get_team_logo_url(nhl_team);

                // Initialize extended player response
                let mut extended_player = FantasyPlayerExtendedResponse {
                    fantasy_team: fantasy_team_name.clone(),
                    fantasy_team_id,
                    player_name: player.player_name.clone(),
                    position: player.position.clone(),
                    nhl_id: player.nhl_id,
                    nhl_team: nhl_team.to_string(),
                    image_url,
                    team_logo,
                    goals: 0,
                    assists: 0,
                    points: 0,
                    playoff_goals: 0,
                    playoff_assists: 0,
                    playoff_points: 0,
                    playoff_games: 0,
                    form: None,
                    time_on_ice: None,
                };
                // Get playoff stats if available
                match state
                    .nhl_client
                    .get_player_game_log(player.nhl_id, season, game_type)
                    .await
                {
                    Ok(game_log) => {
                        // Check if player has playoff games
                        if !game_log.game_log.is_empty() {
                            // Calculate totals
                            let (goals, assists, points, games) =
                                crate::domain::services::nhl_stats::calculate_totals_from_game_log(
                                    &game_log.game_log,
                                );

                            extended_player.playoff_goals = goals;
                            extended_player.playoff_assists = assists;
                            extended_player.playoff_points = points;
                            extended_player.playoff_games = games;
                            // Calculate form (last N games)
                            let (form_goals, form_assists, form_points, recent_games) =
                                calculate_form_from_game_log(&game_log.game_log, form_games);
                            if !recent_games.is_empty() {
                                extended_player.form = Some(PlayerForm {
                                    games: recent_games.len(),
                                    goals: form_goals,
                                    assists: form_assists,
                                    points: form_points,
                                });
                                // Get TOI from most recent game
                                if let Some(latest_game) = recent_games.first() {
                                    extended_player.time_on_ice = Some(latest_game.toi.clone());
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Error fetching game log for player {}: {}",
                            player.player_name,
                            e
                        );
                    }
                }

                // For games in progress or completed, get current game stats
                if game_state.is_live() || game_state.is_completed() {
                    // Get boxscore data from cache
                    if let Some(Some(boxscore)) = boxscore_cache.get(&game_id) {
                        let (goals, assists) =
                            find_player_stats_by_name(boxscore, nhl_team, &player.player_name, Some(player.nhl_id));
                        extended_player.goals = goals;
                        extended_player.assists = assists;
                        extended_player.points = goals + assists;
                    }
                }

                // Add player to their fantasy team's list
                all_players_by_fantasy_team
                    .entry(fantasy_team_id)
                    .or_default()
                    .push(extended_player);
            }
        }
    }

    Ok(())
}

/// Helper function to create a games summary.
pub fn create_games_summary(
    games: &[crate::domain::models::nhl::TodayGame],
    nhl_team_players: &HashMap<String, HashMap<String, Vec<PlayerInGame>>>,
) -> GamesSummaryResponse {
    // Collect all NHL teams playing today
    let mut nhl_teams = Vec::new();
    for game in games {
        nhl_teams.push(game.home_team.abbrev.clone());
        nhl_teams.push(game.away_team.abbrev.clone());
    }

    // Remove duplicates
    nhl_teams.sort();
    nhl_teams.dedup();
    // Count players per team
    let mut team_player_counts = Vec::new();
    for team in &nhl_teams {
        let count = nhl_team_players
            .get(team)
            .map(|fantasy_map| fantasy_map.values().map(|players| players.len()).sum())
            .unwrap_or(0);
        if count > 0 {
            team_player_counts.push(TeamPlayerCountResponse {
                nhl_team: team.clone(),
                player_count: count,
            });
        }
    }

    // Sort by count (descending)
    team_player_counts.sort_by(|a, b| b.player_count.cmp(&a.player_count));
    GamesSummaryResponse {
        total_games: games.len(),
        total_teams_playing: nhl_teams.len(),
        team_players_count: team_player_counts,
    }
}
