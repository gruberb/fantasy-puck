use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};

use crate::api::dtos::NhlRosterPlayer;
use crate::api::response::{json_success, ApiResponse};
use crate::api::routes::AppState;
use crate::error::Result;

/// Returns the roster for a specific NHL team.
/// GET /api/nhl/roster/:team
pub async fn get_team_roster(
    State(state): State<Arc<AppState>>,
    Path(team): Path<String>,
) -> Result<Json<ApiResponse<Vec<NhlRosterPlayer>>>> {
    let team_abbrev = team.to_uppercase();

    let players = state.nhl_client.get_team_roster(&team_abbrev).await?;

    let roster: Vec<NhlRosterPlayer> = players
        .into_iter()
        .map(|p| {
            let first = p.first_name.get("default").cloned().unwrap_or_default();
            let last = p.last_name.get("default").cloned().unwrap_or_default();
            NhlRosterPlayer {
                nhl_id: p.id,
                name: format!("{} {}", first, last),
                position: p.position,
                team: team_abbrev.clone(),
                headshot_url: format!(
                    "https://assets.nhle.com/mugs/nhl/latest/{}.png",
                    p.id
                ),
            }
        })
        .collect();

    Ok(json_success(roster))
}
