use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;

use crate::api::dtos::*;
use crate::api::response::{json_success, ApiResponse};
use crate::api::routes::AppState;
use crate::api::{current_date_window, game_type, season};
use crate::auth::middleware::AuthUser;
use crate::error::Result;
use crate::domain::models::db::{FantasyPlayer, FantasyTeam};
use crate::infra::db::nhl_mirror;

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

/// Per-team detail view rendered on `/league/:id/team/:id`.
///
/// Reads the same `nhl_player_game_stats` aggregate the dashboard's
/// overall rankings use, so `Total Points` here is a strict refinement
/// of that team's row on the dashboard rather than an independent
/// computation that drifts. Before the seal-on-final pipeline existed
/// this handler called the NHL skater-stats-leaders endpoint directly,
/// which gave a "live-correct" cumulative number that disagreed with
/// the dashboard's per-game-mirror sum every time a game's post-buzzer
/// adjustment landed (see commit history for the playoff drift bug).
pub async fn get_team(
    State(state): State<Arc<AppState>>,
    Path(team_id): Path<i64>,
    Query(league_params): Query<LeagueParams>,
) -> Result<Json<ApiResponse<TeamPointsResponse>>> {
    let league_id = &league_params.league_id;

    let team = state.db.get_team(team_id, league_id).await?;
    let players = state.db.get_team_players(team_id).await?;

    let totals = nhl_mirror::list_team_player_season_totals(
        state.db.pool(),
        team_id,
        season() as i32,
        game_type() as i16,
        current_date_window(),
    )
    .await?;
    let totals_by_id: HashMap<i64, &nhl_mirror::TeamPlayerSeasonTotalsRow> =
        totals.iter().map(|r| (r.nhl_id, r)).collect();

    let mut seen_players = HashSet::new();
    let mut player_stats_list = Vec::new();
    let mut team_goals = 0i32;
    let mut team_assists = 0i32;
    let mut team_points = 0i32;

    for player in &players {
        if !seen_players.insert(player.nhl_id) {
            continue;
        }
        let row = totals_by_id.get(&player.nhl_id);
        let goals = row.map(|r| r.goals as i32).unwrap_or(0);
        let assists = row.map(|r| r.assists as i32).unwrap_or(0);
        let points = row.map(|r| r.points as i32).unwrap_or(0);

        team_goals += goals;
        team_assists += assists;
        team_points += points;

        player_stats_list.push(PlayerStatsResponse {
            name: player.name.clone(),
            nhl_team: player.nhl_team.clone(),
            nhl_id: player.nhl_id,
            position: player.position.clone(),
            goals,
            assists,
            total_points: points,
            image_url: state.nhl_client.get_player_image_url(player.nhl_id),
            team_logo: state.nhl_client.get_team_logo_url(&player.nhl_team),
            breakdown: None,
        });
    }

    Ok(json_success(TeamPointsResponse {
        team_id: team.id,
        team_name: team.name,
        players: player_stats_list,
        team_totals: TeamTotalsResponse {
            goals: team_goals,
            assists: team_assists,
            total_points: team_points,
        },
        diagnosis: None,
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
