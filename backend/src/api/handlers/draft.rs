use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::api::response::{json_success, ApiResponse};
use crate::api::routes::AppState;
use crate::api::{game_type, season};
use crate::auth::middleware::AuthUser;
use crate::error::Result;
use crate::infra::db::draft::{DraftPickRow, DraftSessionRow, PlayerPoolRow, SleeperRow};
use crate::infra::jobs::player_pool::{
    fetch_playoff_roster_pool_cached, fetch_stats_leader_pool, PoolMap,
};
use crate::ws::draft_hub::DraftEvent;

/// Branch on configured game_type to build the pool.
/// Playoffs (3) use the 16-team rosters; everything else uses the stats-leader endpoint.
async fn build_player_pool(state: &AppState) -> Result<PoolMap> {
    if crate::api::is_playoffs() {
        fetch_playoff_roster_pool_cached(&state.db, &state.nhl_client, season(), game_type()).await
    } else {
        fetch_stats_leader_pool(&state.nhl_client, season(), game_type()).await
    }
}

fn pool_to_inserts(
    session_id: &str,
    map: PoolMap,
) -> Vec<crate::infra::db::draft::PlayerPoolInsert> {
    map.into_iter()
        .map(|(nhl_id, (name, position, nhl_team, headshot_url))| {
            crate::infra::db::draft::PlayerPoolInsert {
                draft_session_id: session_id.to_string(),
                nhl_id,
                name,
                position,
                nhl_team,
                headshot_url,
            }
        })
        .collect()
}

/// Global admins pass every league check so they can repair drafts in
/// leagues they don't belong to.
async fn require_member(state: &AppState, league_id: &str, user: &AuthUser) -> Result<()> {
    if !user.is_admin {
        state.db.verify_user_in_league(league_id, &user.id).await?;
    }
    Ok(())
}

/// Draft controls (pool, order, start/pause/resume, sleeper round) are
/// owner-only, matching what the draft page exposes.
async fn require_owner(state: &AppState, league_id: &str, user: &AuthUser) -> Result<()> {
    if !user.is_admin {
        state.db.verify_league_owner(league_id, &user.id).await?;
    }
    Ok(())
}

fn session_updated_event(s: &DraftSessionRow) -> DraftEvent {
    DraftEvent::SessionUpdated {
        session_id: s.id.clone(),
        status: s.status.clone(),
        current_round: s.current_round,
        current_pick_index: s.current_pick_index,
        sleeper_status: s.sleeper_status.clone(),
        sleeper_pick_index: s.sleeper_pick_index,
    }
}

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDraftRequest {
    pub total_rounds: i32,
    pub snake_draft: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MakePickRequest {
    pub player_pool_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MakeSleeperPickRequest {
    pub player_pool_id: String,
    pub team_id: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftStateResponse {
    pub session: DraftSessionRow,
    pub picks: Vec<DraftPickRow>,
    pub player_pool: Vec<PlayerPoolRow>,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET /api/leagues/:league_id/draft
pub async fn get_draft_by_league(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(league_id): Path<String>,
) -> Result<Json<ApiResponse<Option<DraftStateResponse>>>> {
    require_member(&state, &league_id, &auth_user).await?;
    let session = state.db.get_draft_session(&league_id).await?;

    match session {
        None => Ok(json_success(None)),
        Some(session) => {
            let picks = state.db.get_draft_picks(&session.id).await?;
            let player_pool = state.db.get_player_pool(&session.id).await?;

            Ok(json_success(Some(DraftStateResponse {
                session,
                picks,
                player_pool,
            })))
        }
    }
}

/// POST /api/leagues/:league_id/draft
pub async fn create_draft_session(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(league_id): Path<String>,
    Json(body): Json<CreateDraftRequest>,
) -> Result<Json<ApiResponse<DraftSessionRow>>> {
    state
        .db
        .verify_user_in_league(&league_id, &auth_user.id)
        .await?;
    let session = state
        .db
        .create_draft_session(&league_id, body.total_rounds, body.snake_draft)
        .await?;

    let pool = build_player_pool(&state).await?;
    state
        .db
        .insert_player_pool(&session.id, pool_to_inserts(&session.id, pool))
        .await?;

    Ok(json_success(session))
}

/// GET /api/draft/:draft_id
pub async fn get_draft_state(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(draft_id): Path<String>,
) -> Result<Json<ApiResponse<DraftStateResponse>>> {
    require_member(
        &state,
        &state.db.get_league_id_for_draft(&draft_id).await?,
        &auth_user,
    )
    .await?;
    let session = state.db.get_draft_session_by_id(&draft_id).await?;
    let picks = state.db.get_draft_picks(&draft_id).await?;
    let player_pool = state.db.get_player_pool(&draft_id).await?;

    Ok(json_success(DraftStateResponse {
        session,
        picks,
        player_pool,
    }))
}

/// POST /api/draft/:draft_id/populate
pub async fn populate_player_pool(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(draft_id): Path<String>,
) -> Result<Json<ApiResponse<Vec<PlayerPoolRow>>>> {
    require_owner(
        &state,
        &state.db.get_league_id_for_draft(&draft_id).await?,
        &auth_user,
    )
    .await?;
    let pool = build_player_pool(&state).await?;

    state.db.delete_player_pool(&draft_id).await?;
    state
        .db
        .insert_player_pool(&draft_id, pool_to_inserts(&draft_id, pool))
        .await?;

    let rows = state.db.get_player_pool(&draft_id).await?;

    state
        .draft_hub
        .broadcast(&draft_id, DraftEvent::PlayerPoolUpdated)
        .await;

    Ok(json_success(rows))
}

/// POST /api/leagues/:league_id/draft/randomize-order
pub async fn randomize_order(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(league_id): Path<String>,
) -> Result<Json<ApiResponse<()>>> {
    require_owner(&state, &league_id, &auth_user).await?;
    state.db.randomize_draft_order(&league_id).await?;
    Ok(json_success(()))
}

/// POST /api/draft/:draft_id/start
pub async fn start_draft(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(draft_id): Path<String>,
) -> Result<Json<ApiResponse<DraftSessionRow>>> {
    require_owner(
        &state,
        &state.db.get_league_id_for_draft(&draft_id).await?,
        &auth_user,
    )
    .await?;
    let session = state.db.start_draft_session(&draft_id).await?;

    state
        .draft_hub
        .broadcast(&draft_id, session_updated_event(&session))
        .await;

    Ok(json_success(session))
}

/// POST /api/draft/:draft_id/pause
pub async fn pause_draft(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(draft_id): Path<String>,
) -> Result<Json<ApiResponse<DraftSessionRow>>> {
    require_owner(
        &state,
        &state.db.get_league_id_for_draft(&draft_id).await?,
        &auth_user,
    )
    .await?;
    let session = state.db.pause_draft_session(&draft_id).await?;

    state
        .draft_hub
        .broadcast(&draft_id, session_updated_event(&session))
        .await;

    Ok(json_success(session))
}

/// POST /api/draft/:draft_id/resume
pub async fn resume_draft(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(draft_id): Path<String>,
) -> Result<Json<ApiResponse<DraftSessionRow>>> {
    require_owner(
        &state,
        &state.db.get_league_id_for_draft(&draft_id).await?,
        &auth_user,
    )
    .await?;
    let session = state.db.resume_draft_session(&draft_id).await?;

    state
        .draft_hub
        .broadcast(&draft_id, session_updated_event(&session))
        .await;

    Ok(json_success(session))
}

/// DELETE /api/draft/:draft_id
pub async fn delete_draft(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(draft_id): Path<String>,
) -> Result<Json<ApiResponse<()>>> {
    let league_id = state.db.get_league_id_for_draft(&draft_id).await?;
    state
        .db
        .verify_league_owner(&league_id, &auth_user.id)
        .await?;
    state.db.delete_draft_session(&draft_id).await?;
    Ok(json_success(()))
}

/// POST /api/draft/:draft_id/pick
pub async fn make_pick(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(draft_id): Path<String>,
    Json(body): Json<MakePickRequest>,
) -> Result<Json<ApiResponse<DraftPickRow>>> {
    let league_id = state.db.get_league_id_for_draft(&draft_id).await?;
    state
        .db
        .verify_user_in_league(&league_id, &auth_user.id)
        .await?;

    let (pick, session) = state
        .db
        .make_draft_pick(&draft_id, &body.player_pool_id)
        .await?;

    match serde_json::to_value(&pick) {
        Ok(pick_json) => {
            state
                .draft_hub
                .broadcast(&draft_id, DraftEvent::PickMade { pick: pick_json })
                .await
        }
        Err(e) => {
            tracing::warn!(draft_id = %draft_id, "failed to serialize pick for broadcast: {e}")
        }
    }
    state
        .draft_hub
        .broadcast(&draft_id, session_updated_event(&session))
        .await;

    Ok(json_success(pick))
}

/// POST /api/draft/:draft_id/finalize
pub async fn finalize_draft(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(draft_id): Path<String>,
) -> Result<Json<ApiResponse<()>>> {
    require_member(
        &state,
        &state.db.get_league_id_for_draft(&draft_id).await?,
        &auth_user,
    )
    .await?;

    // Sync draft picks to fantasy_players table
    state.db.finalize_draft_to_players(&draft_id).await?;

    // Start the sleeper round
    state
        .db
        .update_sleeper_status(&draft_id, "active", 0)
        .await?;

    let session = state.db.get_draft_session_by_id(&draft_id).await?;

    // Broadcast so all clients transition to sleeper round
    state
        .draft_hub
        .broadcast(&draft_id, session_updated_event(&session))
        .await;

    Ok(json_success(()))
}

/// POST /api/draft/:draft_id/complete — marks draft as fully completed
pub async fn complete_draft(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(draft_id): Path<String>,
) -> Result<Json<ApiResponse<()>>> {
    require_member(
        &state,
        &state.db.get_league_id_for_draft(&draft_id).await?,
        &auth_user,
    )
    .await?;

    state
        .db
        .update_draft_status(
            &draft_id,
            "completed",
            None,
            Some(&chrono::Utc::now().to_rfc3339()),
        )
        .await?;

    let session = state.db.get_draft_session_by_id(&draft_id).await?;
    state
        .draft_hub
        .broadcast(&draft_id, session_updated_event(&session))
        .await;

    Ok(json_success(()))
}

/// GET /api/draft/:draft_id/sleepers
pub async fn get_eligible_sleepers(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(draft_id): Path<String>,
) -> Result<Json<ApiResponse<Vec<PlayerPoolRow>>>> {
    require_member(
        &state,
        &state.db.get_league_id_for_draft(&draft_id).await?,
        &auth_user,
    )
    .await?;
    let sleepers = state.db.get_undrafted_pool_players(&draft_id).await?;
    Ok(json_success(sleepers))
}

/// POST /api/draft/:draft_id/sleeper/start
pub async fn start_sleeper_round(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(draft_id): Path<String>,
) -> Result<Json<ApiResponse<DraftSessionRow>>> {
    require_owner(
        &state,
        &state.db.get_league_id_for_draft(&draft_id).await?,
        &auth_user,
    )
    .await?;
    let session = state.db.start_sleeper_round(&draft_id).await?;

    state
        .draft_hub
        .broadcast(&draft_id, DraftEvent::SleeperUpdated)
        .await;

    Ok(json_success(session))
}

/// GET /api/draft/:draft_id/sleeper-picks
pub async fn get_sleeper_picks(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(draft_id): Path<String>,
) -> Result<Json<ApiResponse<Vec<SleeperRow>>>> {
    let session = state.db.get_draft_session_by_id(&draft_id).await?;
    require_member(&state, &session.league_id, &auth_user).await?;
    let picks = state.db.list_league_sleepers(&session.league_id).await?;

    Ok(json_success(picks))
}

/// POST /api/draft/:draft_id/sleeper/pick
pub async fn make_sleeper_pick(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(draft_id): Path<String>,
    Json(body): Json<MakeSleeperPickRequest>,
) -> Result<Json<ApiResponse<()>>> {
    let league_id = state.db.get_league_id_for_draft(&draft_id).await?;
    state
        .db
        .verify_user_in_league(&league_id, &auth_user.id)
        .await?;

    let session = state
        .db
        .make_sleeper_pick(&draft_id, body.team_id, &body.player_pool_id)
        .await?;

    state
        .draft_hub
        .broadcast(&draft_id, session_updated_event(&session))
        .await;
    state
        .draft_hub
        .broadcast(&draft_id, DraftEvent::SleeperUpdated)
        .await;

    Ok(json_success(()))
}
