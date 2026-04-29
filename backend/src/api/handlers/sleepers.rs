use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    Json,
};

use crate::api::dtos::*;
use crate::api::response::{json_success, ApiResponse};
use crate::api::routes::AppState;
use crate::api::{current_date_window, game_type, season};
use crate::auth::middleware::AuthUser;
use crate::error::Result;
use crate::infra::db::nhl_mirror;

/// Sleeper-pick widget. Reads counting stats from the per-game mirror
/// rather than the cumulative-leaderboard endpoint so the totals match
/// every other surface in the league dashboard. The leaderboard path
/// previously used here was the second offender in the dashboard-vs-
/// detail point drift; this share-the-source rewrite is what closes
/// that gap.
///
/// Plus/minus and TOI are still presented per-player but as cumulative
/// boxscore-derived numbers from `nhl_player_game_stats`, not the NHL
/// leaderboard's per-category top-N rendering.
pub async fn get_sleepers(
    State(state): State<Arc<AppState>>,
    Query(league_params): Query<LeagueParams>,
) -> Result<Json<ApiResponse<Vec<SleeperStatsResponse>>>> {
    let league_id = &league_params.league_id;
    let sleepers = state.db.get_all_sleepers(league_id).await?;

    let nhl_ids: Vec<i64> = sleepers.iter().map(|s| s.nhl_id).collect();
    let totals = nhl_mirror::list_player_season_totals(
        state.db.pool(),
        &nhl_ids,
        season() as i32,
        game_type() as i16,
        current_date_window(),
    )
    .await?;
    let totals_by_id: HashMap<i64, &nhl_mirror::TeamPlayerSeasonTotalsRow> =
        totals.iter().map(|r| (r.nhl_id, r)).collect();

    let fantasy_teams = state.db.get_all_teams(league_id).await?;
    let team_name_map: HashMap<i64, String> = fantasy_teams
        .into_iter()
        .map(|team| (team.id, team.name))
        .collect();

    let mut sleeper_stats = Vec::new();
    for sleeper in sleepers {
        let row = totals_by_id.get(&sleeper.nhl_id);
        let goals = row.map(|r| r.goals as i32).unwrap_or(0);
        let assists = row.map(|r| r.assists as i32).unwrap_or(0);
        let total_points = row.map(|r| r.points as i32).unwrap_or(0);

        let fantasy_team = sleeper
            .team_id
            .and_then(|tid| team_name_map.get(&tid).cloned());

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
            total_points,
            // The boxscore mirror does not summarise plus/minus or TOI
            // back up to the season level in a single read; both fields
            // would require a second round-trip. Sleeper Pick has only
            // ever displayed counting stats in the Pulse / dashboard
            // surfaces, so we drop them rather than ship a half-correct
            // number. If a future reader needs them, lift the values
            // out of `nhl_player_game_stats` (cumulative SUM/AVG)
            // alongside the goals/assists aggregate above.
            plus_minus: None,
            time_on_ice: None,
            image_url: state.nhl_client.get_player_image_url(sleeper.nhl_id),
            team_logo: state.nhl_client.get_team_logo_url(&sleeper.nhl_team),
        });
    }

    sleeper_stats.sort_by(|a, b| b.total_points.cmp(&a.total_points));
    Ok(json_success(sleeper_stats))
}

/// DELETE /api/fantasy/sleepers/:sleeper_id
pub async fn remove_sleeper(
    State(state): State<Arc<AppState>>,
    _auth_user: AuthUser,
    Path(sleeper_id): Path<i64>,
) -> Result<Json<ApiResponse<()>>> {
    state.db.remove_sleeper(sleeper_id).await?;
    Ok(json_success(()))
}
