use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{Query, State},
    Json,
};
use serde::Serialize;

use crate::api::dtos::LeagueParams;
use crate::api::response::{json_success, ApiResponse};
use crate::api::routes::AppState;
use crate::api::season;
use crate::error::Result;
use crate::infra::db::league_stats;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NhlTeamRosterRow {
    pub nhl_team: String,
    pub team_name: String,
    pub team_logo: String,
    pub rostered_count: i32,
    pub playoff_points: i32,
    pub top_skater_name: Option<String>,
    pub top_skater_photo: Option<String>,
    pub top_skater_points: Option<i32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RosteredSkaterRow {
    pub nhl_id: i64,
    pub name: String,
    pub photo: String,
    pub nhl_team: String,
    pub team_logo: String,
    pub playoff_points: i32,
    pub fantasy_team_id: i64,
    pub fantasy_team_name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeagueStatsResponse {
    pub nhl_teams_rostered: Vec<NhlTeamRosterRow>,
    pub top_rostered_skaters: Vec<RosteredSkaterRow>,
}

/// League-wide stats for the /stats page: how our rosters concentrate
/// across NHL teams (with each NHL team's playoff output + top scorer)
/// and the top-10 rostered skaters by playoff fantasy points.
///
/// One response so the frontend doesn't fan out two queries for what
/// is really one page section.
pub async fn get_league_stats(
    State(state): State<Arc<AppState>>,
    Query(params): Query<LeagueParams>,
) -> Result<Json<ApiResponse<LeagueStatsResponse>>> {
    let pool = state.db.pool();
    let season = season() as i32;
    let league_id = &params.league_id;

    // Four independent reads. Running them concurrently keeps the
    // handler's total latency ~= max(query) instead of sum(query),
    // which matters on cold pages where no cache entry exists.
    let (counts, points, top_skaters, skaters) = tokio::try_join!(
        league_stats::list_nhl_team_roster_counts(pool, league_id),
        league_stats::list_nhl_team_playoff_points(pool, season),
        league_stats::list_nhl_team_top_skaters(pool, season),
        league_stats::list_top_rostered_skaters(pool, league_id, season, 10),
    )?;

    let points_by_team: HashMap<String, i64> = points
        .into_iter()
        .map(|r| (r.team_abbrev, r.playoff_points))
        .collect();
    let top_by_team: HashMap<String, league_stats::NhlTeamTopSkaterRow> = top_skaters
        .into_iter()
        .map(|r| (r.team_abbrev.clone(), r))
        .collect();

    // Drop empty-string nhl_team rows (placeholder rosters from drafts
    // that never finished) rather than rendering a blank logo row.
    let mut nhl_teams_rostered: Vec<NhlTeamRosterRow> = counts
        .into_iter()
        .filter(|r| !r.nhl_team.is_empty())
        .map(|r| {
            let playoff_points = points_by_team.get(&r.nhl_team).copied().unwrap_or(0) as i32;
            let top = top_by_team.get(&r.nhl_team);
            NhlTeamRosterRow {
                nhl_team: r.nhl_team.clone(),
                team_name: state.nhl_client.get_team_name(&r.nhl_team),
                team_logo: state.nhl_client.get_team_logo_url(&r.nhl_team),
                rostered_count: r.rostered_count as i32,
                playoff_points,
                top_skater_name: top.map(|t| t.name.clone()),
                top_skater_photo: top.map(|t| state.nhl_client.get_player_image_url(t.player_id)),
                top_skater_points: top.map(|t| t.points as i32),
            }
        })
        .collect();

    nhl_teams_rostered.sort_by(|a, b| {
        b.rostered_count
            .cmp(&a.rostered_count)
            .then_with(|| b.playoff_points.cmp(&a.playoff_points))
            .then_with(|| a.nhl_team.cmp(&b.nhl_team))
    });

    let top_rostered_skaters: Vec<RosteredSkaterRow> = skaters
        .into_iter()
        .map(|r| RosteredSkaterRow {
            nhl_id: r.nhl_id,
            name: r.name,
            photo: state.nhl_client.get_player_image_url(r.nhl_id),
            nhl_team: r.nhl_team.clone(),
            team_logo: state.nhl_client.get_team_logo_url(&r.nhl_team),
            playoff_points: r.playoff_points as i32,
            fantasy_team_id: r.fantasy_team_id,
            fantasy_team_name: r.fantasy_team_name,
        })
        .collect();

    Ok(json_success(LeagueStatsResponse {
        nhl_teams_rostered,
        top_rostered_skaters,
    }))
}
