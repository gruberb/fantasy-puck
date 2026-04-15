use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::api::response::{json_success, ApiResponse};
use crate::api::routes::AppState;
use crate::api::{game_type, season};
use crate::auth::middleware::AuthUser;
use crate::db::draft::{DraftPickRow, DraftSessionRow, PlayerPoolRow};
use crate::error::{Error, Result};
use crate::ws::draft_hub::DraftEvent;

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
    _auth_user: AuthUser,
    Path(league_id): Path<String>,
) -> Result<Json<ApiResponse<Option<DraftStateResponse>>>> {
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
    state.db.verify_user_in_league(&league_id, &auth_user.id).await?;
    let session = state
        .db
        .create_draft_session(&league_id, body.total_rounds, body.snake_draft)
        .await?;

    // Auto-populate the player pool with regular season stats (game_type=2)
    let stats = state
        .nhl_client
        .get_skater_stats(&season(), 2) // regular season for broader player pool
        .await
        .map_err(|e| {
            error!("Failed to fetch skater stats for player pool: {}", e);
            Error::NhlApi(format!("Failed to fetch skater stats: {}", e))
        })?;

    let mut players_map: HashMap<i64, (String, String, String, String)> = HashMap::new();
    let all_players = [
        &stats.goals, &stats.assists, &stats.points, &stats.goals_pp,
        &stats.goals_sh, &stats.plus_minus, &stats.faceoff_leaders,
        &stats.penalty_mins, &stats.toi,
    ];
    for category in all_players {
        for player in category {
            let player_id = player.id as i64;
            players_map.entry(player_id).or_insert_with(|| {
                let first = player.first_name.get("default").cloned().unwrap_or_default();
                let last = player.last_name.get("default").cloned().unwrap_or_default();
                let name = format!("{} {}", first, last);
                let headshot = format!("https://assets.nhle.com/mugs/nhl/latest/{}.png", player_id);
                (name, player.position.clone(), player.team_abbrev.clone(), headshot)
            });
        }
    }

    let pool_inserts: Vec<crate::db::draft::PlayerPoolInsert> = players_map
        .into_iter()
        .map(|(nhl_id, (name, position, nhl_team, headshot_url))| {
            crate::db::draft::PlayerPoolInsert {
                draft_session_id: session.id.clone(),
                nhl_id,
                name,
                position,
                nhl_team,
                headshot_url,
            }
        })
        .collect();

    state.db.insert_player_pool(&session.id, pool_inserts).await?;

    Ok(json_success(session))
}

/// GET /api/draft/:draft_id
pub async fn get_draft_state(
    State(state): State<Arc<AppState>>,
    _auth_user: AuthUser,
    Path(draft_id): Path<String>,
) -> Result<Json<ApiResponse<DraftStateResponse>>> {
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
    _auth_user: AuthUser,
    Path(draft_id): Path<String>,
) -> Result<Json<ApiResponse<Vec<PlayerPoolRow>>>> {
    // Verify session exists
    let _session = state.db.get_draft_session_by_id(&draft_id).await?;

    // Fetch skater stats from the NHL API
    let stats = state
        .nhl_client
        .get_skater_stats(&season(), game_type())
        .await
        .map_err(|e| {
            error!("Failed to fetch skater stats for player pool: {}", e);
            Error::NhlApi(format!("Failed to fetch skater stats: {}", e))
        })?;

    // Collect unique players from all stat categories
    let mut players_map: HashMap<i64, (String, String, String, String)> = HashMap::new();

    let all_players = [
        &stats.goals,
        &stats.assists,
        &stats.points,
        &stats.goals_pp,
        &stats.goals_sh,
        &stats.plus_minus,
        &stats.faceoff_leaders,
        &stats.penalty_mins,
        &stats.toi,
    ];

    for category in all_players {
        for player in category {
            let player_id = player.id as i64;
            players_map.entry(player_id).or_insert_with(|| {
                let first = player
                    .first_name
                    .get("default")
                    .cloned()
                    .unwrap_or_default();
                let last = player
                    .last_name
                    .get("default")
                    .cloned()
                    .unwrap_or_default();
                let name = format!("{} {}", first, last);
                let headshot =
                    format!("https://assets.nhle.com/mugs/nhl/latest/{}.png", player_id);
                (name, player.position.clone(), player.team_abbrev.clone(), headshot)
            });
        }
    }

    // Clear existing pool for this session and insert fresh
    state.db.delete_player_pool(&draft_id).await?;

    for (nhl_id, (name, position, nhl_team, headshot_url)) in &players_map {
        state
            .db
            .insert_single_pool_player(&draft_id, *nhl_id, name, position, nhl_team, headshot_url)
            .await?;
    }

    // Return the populated pool
    let pool = state.db.get_player_pool(&draft_id).await?;

    Ok(json_success(pool))
}

/// POST /api/leagues/:league_id/draft/randomize-order
pub async fn randomize_order(
    State(state): State<Arc<AppState>>,
    _auth_user: AuthUser,
    Path(league_id): Path<String>,
) -> Result<Json<ApiResponse<()>>> {
    state.db.randomize_draft_order(&league_id).await?;
    Ok(json_success(()))
}

/// POST /api/draft/:draft_id/start
pub async fn start_draft(
    State(state): State<Arc<AppState>>,
    _auth_user: AuthUser,
    Path(draft_id): Path<String>,
) -> Result<Json<ApiResponse<DraftSessionRow>>> {
    let session = state.db.start_draft_session(&draft_id).await?;

    state
        .draft_hub
        .broadcast(
            &draft_id,
            session_updated_event(&session),
        )
        .await;

    Ok(json_success(session))
}

/// POST /api/draft/:draft_id/pause
pub async fn pause_draft(
    State(state): State<Arc<AppState>>,
    _auth_user: AuthUser,
    Path(draft_id): Path<String>,
) -> Result<Json<ApiResponse<DraftSessionRow>>> {
    let session = state.db.pause_draft_session(&draft_id).await?;

    state
        .draft_hub
        .broadcast(
            &draft_id,
            session_updated_event(&session),
        )
        .await;

    Ok(json_success(session))
}

/// POST /api/draft/:draft_id/resume
pub async fn resume_draft(
    State(state): State<Arc<AppState>>,
    _auth_user: AuthUser,
    Path(draft_id): Path<String>,
) -> Result<Json<ApiResponse<DraftSessionRow>>> {
    let session = state.db.resume_draft_session(&draft_id).await?;

    state
        .draft_hub
        .broadcast(
            &draft_id,
            session_updated_event(&session),
        )
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
    state.db.verify_league_owner(&league_id, &auth_user.id).await?;
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
    // Get the current session
    let session = state.db.get_draft_session_by_id(&draft_id).await?;
    state.db.verify_user_in_league(&session.league_id, &auth_user.id).await?;

    if session.status != "active" {
        return Err(Error::Validation("Draft is not active".into()));
    }

    // Get members to check total picks
    let member_ids = state
        .db
        .get_league_member_ids_ordered(&session.league_id)
        .await?;
    let num_members = member_ids.len() as i32;
    let total_picks = session.total_rounds * num_members;

    if session.current_pick_index >= total_picks {
        return Err(Error::Validation("All rounds are complete".into()));
    }

    // Look up the player in the pool
    let pool_player = state
        .db
        .get_draft_pool_player(&body.player_pool_id, &draft_id)
        .await?;

    // Check that this player hasn't already been picked
    let already_picked = state
        .db
        .check_player_already_picked(&draft_id, pool_player.nhl_id)
        .await?;

    if already_picked {
        return Err(Error::Validation("Player already drafted".into()));
    }

    if num_members == 0 {
        return Err(Error::Validation("No members in league".into()));
    }

    // current_pick_index is a GLOBAL counter: 0, 1, 2, ..., (total_rounds * num_members - 1)
    let pick_index = session.current_pick_index;
    let round = pick_index / num_members; // 0-based round
    let index_in_round = pick_index % num_members;

    // Snake draft: even rounds go forward, odd rounds go reverse
    let member_index = if session.snake_draft && round % 2 == 1 {
        (num_members - 1) - index_in_round
    } else {
        index_in_round
    };

    let picking_member_id = &member_ids[member_index as usize];

    // Insert the pick (pick_number is the global 0-based index, matching frontend)
    let pick = state
        .db
        .insert_draft_pick(crate::db::draft::DraftPickInsert {
            draft_session_id: draft_id.clone(),
            league_member_id: picking_member_id.clone(),
            player_pool_id: body.player_pool_id.clone(),
            nhl_id: pool_player.nhl_id,
            player_name: pool_player.name.clone(),
            nhl_team: pool_player.nhl_team.clone(),
            position: pool_player.position.clone(),
            round: round + 1, // 1-based for display
            pick_number: pick_index, // 0-based global index
        })
        .await?;

    // Advance: increment global pick index and compute new round (1-based)
    let new_pick_index = pick_index + 1;
    let new_round = (new_pick_index / num_members) + 1; // 1-based

    // Check if all rounds are now complete
    let updated_session = if new_pick_index >= total_picks {
        // Mark as "picks_done" — admin needs to finalize before sleeper round
        state
            .db
            .update_draft_status(&draft_id, "picks_done", None, None)
            .await?;
        state.db.get_draft_session_by_id(&draft_id).await?
    } else {
        state
            .db
            .advance_draft_session(&draft_id, new_pick_index, new_round)
            .await?
    };

    // Broadcast events
    let pick_json = serde_json::to_value(&pick).unwrap_or_default();
    state
        .draft_hub
        .broadcast(&draft_id, DraftEvent::PickMade { pick: pick_json })
        .await;

    state
        .draft_hub
        .broadcast(
            &draft_id,
            session_updated_event(&updated_session),
        )
        .await;

    Ok(json_success(pick))
}

/// POST /api/draft/:draft_id/finalize
pub async fn finalize_draft(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(draft_id): Path<String>,
) -> Result<Json<ApiResponse<()>>> {
    let league_id = state.db.get_league_id_for_draft(&draft_id).await?;
    state.db.verify_user_in_league(&league_id, &auth_user.id).await?;

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
        .broadcast(
            &draft_id,
            session_updated_event(&session),
        )
        .await;

    Ok(json_success(()))
}

/// POST /api/draft/:draft_id/complete — marks draft as fully completed
pub async fn complete_draft(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(draft_id): Path<String>,
) -> Result<Json<ApiResponse<()>>> {
    let league_id = state.db.get_league_id_for_draft(&draft_id).await?;
    state.db.verify_user_in_league(&league_id, &auth_user.id).await?;

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
    _auth_user: AuthUser,
    Path(draft_id): Path<String>,
) -> Result<Json<ApiResponse<Vec<PlayerPoolRow>>>> {
    let sleepers = state.db.get_undrafted_pool_players(&draft_id).await?;
    Ok(json_success(sleepers))
}

/// POST /api/draft/:draft_id/sleeper/start
pub async fn start_sleeper_round(
    State(state): State<Arc<AppState>>,
    _auth_user: AuthUser,
    Path(draft_id): Path<String>,
) -> Result<Json<ApiResponse<DraftSessionRow>>> {
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
    _auth_user: AuthUser,
    Path(draft_id): Path<String>,
) -> Result<Json<ApiResponse<Vec<serde_json::Value>>>> {
    let session = state.db.get_draft_session_by_id(&draft_id).await?;
    let picks: Vec<serde_json::Value> = sqlx::query_as::<_, (i64, i64, i64, String, String, String)>(
        r#"
        SELECT fs.id, fs.team_id, fs.nhl_id, fs.name, fs.position, fs.nhl_team
        FROM fantasy_sleepers fs
        JOIN league_members lm ON lm.fantasy_team_id = fs.team_id
        WHERE lm.league_id = $1::uuid
        ORDER BY fs.id
        "#,
    )
    .bind(&session.league_id)
    .fetch_all(state.db.pool())
    .await?
    .into_iter()
    .map(|(id, team_id, nhl_id, name, position, nhl_team)| {
        serde_json::json!({
            "id": id,
            "teamId": team_id,
            "nhlId": nhl_id,
            "name": name,
            "position": position,
            "nhlTeam": nhl_team
        })
    })
    .collect();

    Ok(json_success(picks))
}

/// POST /api/draft/:draft_id/sleeper/pick
pub async fn make_sleeper_pick(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(draft_id): Path<String>,
    Json(body): Json<MakeSleeperPickRequest>,
) -> Result<Json<ApiResponse<()>>> {
    let session = state.db.get_draft_session_by_id(&draft_id).await?;
    state.db.verify_user_in_league(&session.league_id, &auth_user.id).await?;

    if session.sleeper_status.as_deref() != Some("active") {
        return Err(Error::Validation("Sleeper round is not active".into()));
    }

    // Check how many members — each gets exactly 1 pick
    let member_ids = state.db.get_league_member_ids_ordered(&session.league_id).await?;
    let num_members = member_ids.len() as i32;

    if session.sleeper_pick_index >= num_members {
        return Err(Error::Validation("All sleeper picks are done".into()));
    }

    // Check if this team already picked a sleeper
    let already_has_sleeper: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM fantasy_sleepers WHERE team_id = $1 LIMIT 1",
    )
    .bind(body.team_id)
    .fetch_optional(state.db.pool())
    .await?;

    if already_has_sleeper.is_some() {
        return Err(Error::Validation("This team already has a sleeper pick".into()));
    }

    // Look up the pool player
    let pool_player = state
        .db
        .get_draft_pool_player(&body.player_pool_id, &draft_id)
        .await?;

    // Check if this NHL player is already picked as a sleeper by another team
    let player_already_sleeper: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM fantasy_sleepers WHERE nhl_id = $1 LIMIT 1",
    )
    .bind(pool_player.nhl_id)
    .fetch_optional(state.db.pool())
    .await?;

    if player_already_sleeper.is_some() {
        return Err(Error::Validation("This player is already picked as a sleeper by another team".into()));
    }

    // Insert sleeper pick and advance the index
    state
        .db
        .insert_sleeper_and_advance(
            &draft_id,
            body.team_id,
            pool_player.nhl_id,
            &pool_player.name,
            &pool_player.position,
            &pool_player.nhl_team,
        )
        .await?;

    // Check if all sleeper picks are now done
    let new_index = session.sleeper_pick_index + 1;
    if new_index >= num_members {
        state.db.update_sleeper_status(&draft_id, "completed", new_index).await?;
    }

    // Broadcast updated session so all clients refresh
    let updated = state.db.get_draft_session_by_id(&draft_id).await?;
    state
        .draft_hub
        .broadcast(
            &draft_id,
            session_updated_event(&updated),
        )
        .await;
    state
        .draft_hub
        .broadcast(&draft_id, DraftEvent::SleeperUpdated)
        .await;

    Ok(json_success(()))
}
