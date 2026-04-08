use std::sync::Arc;

use axum::{
    extract::{Query, State},
    Json,
};

use crate::api::dtos::*;
use crate::api::response::{json_success, ApiResponse};
use crate::api::routes::AppState;
use crate::api::{GAME_TYPE, SEASON};
use crate::error::Result;

pub async fn get_sleepers(
    State(state): State<Arc<AppState>>,
    Query(league_params): Query<LeagueParams>,
) -> Result<Json<ApiResponse<Vec<SleeperStatsResponse>>>> {
    let league_id = &league_params.league_id;
    let sleepers = state.db.get_all_sleepers(league_id).await?;
    let stats = state
        .nhl_client
        .get_skater_stats(&SEASON, GAME_TYPE)
        .await?;

    // Get fantasy team names for this league
    let fantasy_teams = state.db.get_all_teams(league_id).await?;
    let team_name_map: std::collections::HashMap<i64, String> = fantasy_teams
        .into_iter()
        .map(|team| (team.id, team.name))
        .collect();

    // Process sleepers with stats
    let mut sleeper_stats = Vec::new();
    for sleeper in sleepers {
        // Find player in NHL stats
        let mut goals = 0;
        let mut assists = 0;
        let mut plus_minus = None;

        // Look for goals
        if let Some(player) = stats.goals.iter().find(|p| p.id as i64 == sleeper.nhl_id) {
            goals = player.value as i32;
        }

        // Look for assists
        if let Some(player) = stats.assists.iter().find(|p| p.id as i64 == sleeper.nhl_id) {
            assists = player.value as i32;
        }

        // Look for plus/minus
        if let Some(player) = stats
            .plus_minus
            .iter()
            .find(|p| p.id as i64 == sleeper.nhl_id)
        {
            plus_minus = Some(player.value as i32);
        }

        // Get TOI if available
        let time_on_ice = stats
            .toi
            .iter()
            .find(|p| p.id as i64 == sleeper.nhl_id)
            .map(|p| p.value.to_string());

        // Get fantasy team name
        let fantasy_team = match sleeper.team_id {
            Some(team_id) => team_name_map.get(&team_id).cloned(),
            None => None,
        };

        sleeper_stats.push(SleeperStatsResponse {
            id: sleeper.id,
            nhl_id: sleeper.nhl_id,
            name: sleeper.name,
            nhl_team: sleeper.nhl_team.clone(),
            position: sleeper.position,
            fantasy_team,
            fantasy_team_id: sleeper.team_id,
            goals,
            assists,
            total_points: goals + assists,
            plus_minus,
            time_on_ice,
            image_url: state.nhl_client.get_player_image_url(sleeper.nhl_id),
            team_logo: state.nhl_client.get_team_logo_url(&sleeper.nhl_team),
        });
    }

    // Sort by total points (descending)
    sleeper_stats.sort_by(|a, b| b.total_points.cmp(&a.total_points));

    Ok(json_success(sleeper_stats))
}
