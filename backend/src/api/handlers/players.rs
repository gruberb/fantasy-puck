use std::sync::Arc;

use axum::{
    extract::{Query, State},
    Json,
};

use crate::api::dtos::*;
use crate::api::response::{json_success, ApiResponse};
use crate::api::routes::AppState;
use crate::error::Result;

// Get players per NHL team, scoped to a league
pub async fn get_players_per_team(
    State(state): State<Arc<AppState>>,
    Query(league_params): Query<LeagueParams>,
) -> Result<Json<ApiResponse<Vec<NhlTeamPlayersResponse>>>> {
    let result = state
        .db
        .get_nhl_teams_and_players(&league_params.league_id)
        .await?;

    let response = result
        .into_iter()
        .map(|team_players| {
            let players = team_players
                .players
                .into_iter()
                .map(|player| PlayerWithTeamResponse {
                    nhl_id: player.nhl_id,
                    name: player.name,
                    fantasy_team_id: player.fantasy_team_id,
                    fantasy_team_name: player.fantasy_team_name,
                    position: player.position,
                    nhl_team: player.nhl_team.clone(),
                    image_url: state.nhl_client.get_player_image_url(player.nhl_id),
                })
                .collect();

            NhlTeamPlayersResponse {
                nhl_team: team_players.nhl_team.clone(),
                team_logo: state.nhl_client.get_team_logo_url(&team_players.nhl_team),
                players,
            }
        })
        .collect();

    Ok(json_success(response))
}
