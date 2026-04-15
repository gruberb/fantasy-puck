use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;

use crate::api::response::{json_success, ApiResponse};
use crate::api::routes::AppState;
use crate::auth::middleware::AuthUser;
use crate::db::leagues::LeagueMemberRow;
use crate::db::leagues::LeagueRow;
use crate::error::Result;
use crate::models::db::League;

// ---------------------------------------------------------------------------
// Query / request types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct LeagueQueryParams {
    pub visibility: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateLeagueRequest {
    pub name: String,
    pub season: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinLeagueRequest {
    pub team_name: String,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET /api/leagues
pub async fn list_leagues(
    State(state): State<Arc<AppState>>,
    Query(params): Query<LeagueQueryParams>,
) -> Result<Json<ApiResponse<Vec<League>>>> {
    let leagues = match params.visibility.as_deref() {
        Some("public") => state.db.get_public_leagues().await?,
        _ => state.db.get_all_leagues().await?,
    };
    Ok(json_success(leagues))
}

/// POST /api/leagues
pub async fn create_league(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(body): Json<CreateLeagueRequest>,
) -> Result<Json<ApiResponse<LeagueRow>>> {
    let season = body.season.unwrap_or_else(|| "20252026".to_string());
    let league = state
        .db
        .create_league(&body.name, &season, &auth_user.id)
        .await?;
    Ok(json_success(league))
}

/// DELETE /api/leagues/:league_id
pub async fn delete_league(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(league_id): Path<String>,
) -> Result<Json<ApiResponse<()>>> {
    state.db.verify_league_owner(&league_id, &auth_user.id).await?;
    state.db.delete_league(&league_id).await?;
    Ok(json_success(()))
}

/// GET /api/leagues/:league_id/members
pub async fn get_league_members(
    State(state): State<Arc<AppState>>,
    _auth_user: AuthUser,
    Path(league_id): Path<String>,
) -> Result<Json<ApiResponse<Vec<LeagueMemberRow>>>> {
    let members = state.db.get_league_members(&league_id).await?;
    Ok(json_success(members))
}

/// POST /api/leagues/:league_id/join
pub async fn join_league(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(league_id): Path<String>,
    Json(body): Json<JoinLeagueRequest>,
) -> Result<Json<ApiResponse<()>>> {
    state
        .db
        .join_league(&league_id, &auth_user.id, &body.team_name)
        .await?;
    Ok(json_success(()))
}

/// DELETE /api/leagues/:league_id/members/:member_id
pub async fn remove_member(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path((league_id, member_id)): Path<(String, String)>,
) -> Result<Json<ApiResponse<()>>> {
    // Only the league owner can remove members
    state.db.verify_league_owner(&league_id, &auth_user.id).await?;

    // Validate the member belongs to the league
    state
        .db
        .validate_league_member(&member_id, &league_id)
        .await?;

    state.db.remove_league_member(&member_id).await?;
    Ok(json_success(()))
}
