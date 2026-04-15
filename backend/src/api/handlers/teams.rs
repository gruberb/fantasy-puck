use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use tracing::error;

use crate::api::dtos::*;
use crate::api::response::{json_success, ApiResponse};
use crate::api::routes::AppState;
use crate::api::{game_type, season};
use crate::auth::middleware::AuthUser;
use crate::error::Result;
use crate::models::db::{FantasyPlayer, FantasyTeam};
use crate::models::nhl::StatsLeaders;
use crate::PlayerStats;

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTeamRequest {
    pub name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddPlayerRequest {
    pub nhl_id: i64,
    pub name: String,
    pub position: String,
    pub nhl_team: String,
}

// ---------------------------------------------------------------------------
// Existing handlers
// ---------------------------------------------------------------------------

/// List all fantasy teams in a league
pub async fn list_teams(
    State(state): State<Arc<AppState>>,
    Query(league_params): Query<LeagueParams>,
) -> Result<Json<ApiResponse<Vec<FantasyTeam>>>> {
    let teams = state.db.get_all_teams(&league_params.league_id).await?;
    Ok(json_success(teams))
}

/// Calculate points for a specific team
pub async fn get_team(
    State(state): State<Arc<AppState>>,
    Path(team_id): Path<i64>,
    Query(league_params): Query<LeagueParams>,
) -> Result<Json<ApiResponse<TeamPointsResponse>>> {
    let league_id = &league_params.league_id;

    // Get team and players (verifies team is in league)
    let team = state.db.get_team(team_id, league_id).await?;
    let players = state.db.get_team_players(team_id).await?;

    // Fetch stats from NHL API
    let stats = state
        .nhl_client
        .get_skater_stats(&season(), game_type())
        .await
        .unwrap_or_else(|e| {
            error!("Warning: Couldn't fetch detailed stats: {}", e);
            StatsLeaders::default()
        });

    // Calculate points for each player
    let mut team_totals = PlayerStats::default();

    // Track unique players
    let mut seen_players = std::collections::HashSet::new();
    let mut player_stats_list = Vec::new();

    for player in &players {
        // Skip if we've already processed this player
        if !seen_players.insert(player.nhl_id) {
            continue;
        }

        let player_stats = PlayerStats::default().calculate_player_points(player.nhl_id, &stats);

        // Add to player stats list
        player_stats_list.push(PlayerStatsResponse {
            name: player.name.clone(),
            nhl_team: player.nhl_team.clone(),
            nhl_id: player.nhl_id,
            position: player.position.clone(),
            goals: player_stats.goals,
            assists: player_stats.assists,
            total_points: player_stats.total_points,
            image_url: state.nhl_client.get_player_image_url(player.nhl_id),
            team_logo: state.nhl_client.get_team_logo_url(&player.nhl_team),
        });

        // Add to team totals
        team_totals.goals += player_stats.goals;
        team_totals.assists += player_stats.assists;
        team_totals.total_points += player_stats.total_points;
    }

    Ok(json_success(TeamPointsResponse {
        team_id: team.id,
        team_name: team.name,
        players: player_stats_list,
        team_totals: TeamTotalsResponse {
            goals: team_totals.goals,
            assists: team_totals.assists,
            total_points: team_totals.total_points,
        },
    }))
}

/// Get fantasy team bets by NHL team
pub async fn get_team_bets(
    State(state): State<Arc<AppState>>,
    Query(league_params): Query<LeagueParams>,
) -> Result<Json<ApiResponse<Vec<FantasyTeamBetsResponse>>>> {
    let bets = state
        .db
        .get_fantasy_bets_by_nhl_team(&league_params.league_id)
        .await?;

    let response = bets
        .into_iter()
        .map(|team| FantasyTeamBetsResponse {
            team_id: team.team_id,
            team_name: team.team_name,
            bets: team
                .bets
                .into_iter()
                .map(|bet| NhlBetCountResponse {
                    nhl_team: bet.nhl_team.clone(),
                    nhl_team_name: state.nhl_client.get_team_name(&bet.nhl_team),
                    num_players: bet.num_players,
                    team_logo: state.nhl_client.get_team_logo_url(&bet.nhl_team),
                })
                .collect(),
        })
        .collect();

    Ok(json_success(response))
}

// ---------------------------------------------------------------------------
// New handlers
// ---------------------------------------------------------------------------

/// PUT /api/fantasy/teams/:team_id
pub async fn update_team_name(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(team_id): Path<i64>,
    Json(body): Json<UpdateTeamRequest>,
) -> Result<Json<ApiResponse<()>>> {
    let league_id = state.db.get_league_id_for_team(team_id).await?;
    state.db.verify_league_owner(&league_id, &auth_user.id).await?;
    state.db.update_team_name(team_id, &body.name).await?;
    Ok(json_success(()))
}

/// POST /api/fantasy/teams/:team_id/players
pub async fn add_player_to_team(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(team_id): Path<i64>,
    Json(body): Json<AddPlayerRequest>,
) -> Result<Json<ApiResponse<FantasyPlayer>>> {
    let league_id = state.db.get_league_id_for_team(team_id).await?;
    state.db.verify_league_owner(&league_id, &auth_user.id).await?;
    let player = state
        .db
        .add_player_to_team(team_id, body.nhl_id, &body.name, &body.position, &body.nhl_team)
        .await?;
    Ok(json_success(player))
}

/// DELETE /api/fantasy/players/:player_id
pub async fn remove_player(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(player_id): Path<i64>,
) -> Result<Json<ApiResponse<()>>> {
    let league_id = state.db.get_league_id_for_player(player_id).await?;
    state.db.verify_league_owner(&league_id, &auth_user.id).await?;
    state.db.remove_player(player_id).await?;
    Ok(json_success(()))
}
