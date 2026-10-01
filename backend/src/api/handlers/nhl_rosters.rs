use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};

use crate::api::dtos::NhlRosterPlayer;
use crate::api::response::{json_success, ApiResponse};
use crate::api::routes::AppState;
use crate::domain::models::nhl::default_name;
use crate::error::Result;
use crate::infra::db::nhl_mirror;

/// Returns the roster for a specific NHL team.
/// GET /api/nhl/roster/:team
pub async fn get_team_roster(
    State(state): State<Arc<AppState>>,
    Path(team): Path<String>,
) -> Result<Json<ApiResponse<Vec<NhlRosterPlayer>>>> {
    let team_abbrev = team.to_uppercase();

    // The meta poller mirrors every roster daily; the live call only
    // covers a team that hasn't been captured yet (fresh deploy).
    let players = match nhl_mirror::get_team_roster(
        state.db.pool(),
        &team_abbrev,
        crate::api::season() as i32,
    )
    .await?
    {
        Some(players) => players,
        None => state.nhl_client.get_team_roster(&team_abbrev).await?,
    };

    let roster: Vec<NhlRosterPlayer> = players
        .into_iter()
        .map(|p| NhlRosterPlayer {
            nhl_id: p.id,
            name: format!(
                "{} {}",
                default_name(&p.first_name),
                default_name(&p.last_name)
            ),
            position: p.position,
            team: team_abbrev.clone(),
            headshot_url: state.nhl_client.get_player_image_url(p.id as i64),
        })
        .collect();

    Ok(json_success(roster))
}
