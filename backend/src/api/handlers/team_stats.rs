use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use axum::{
    extract::{Query, State},
    Json,
};

use crate::api::dtos::*;
use crate::api::response::{json_success, ApiResponse};
use crate::api::routes::AppState;
use crate::api::{current_date_window, game_type, playoff_start, season};
use crate::error::Result;
use crate::domain::models::db::FantasyTeamWithPlayers;
use crate::infra::db::{nhl_mirror, DateWindow};

/// Per-team season-overview cards on the rankings page. Reads the same
/// per-game mirror aggregate that the dashboard rankings, daily scores,
/// top-rostered, and team detail handlers use, so a fantasy team's
/// `Total Points` here cannot drift away from its row on the dashboard.
pub async fn get_team_stats(
    State(state): State<Arc<AppState>>,
    Query(league_params): Query<LeagueParams>,
) -> Result<Json<ApiResponse<Vec<TeamStatsResponse>>>> {
    let league_id = &league_params.league_id;

    let teams = state.db.get_all_teams(league_id).await?;
    let mut teams_with_players: Vec<FantasyTeamWithPlayers> = Vec::new();
    for team in teams {
        teams_with_players.push(FantasyTeamWithPlayers {
            id: team.id,
            name: team.name,
            players: state.db.get_team_players(team.id).await?,
        });
    }

    let season_num = season() as i32;
    let game_type_num = game_type() as i16;
    let window = current_date_window();

    // Per-team totals (Total Points). One round-trip, used both for
    // the league_id-scoped aggregation and as the source of truth for
    // each fantasy team's ranking points figure.
    let league_totals = nhl_mirror::list_league_team_season_totals(
        state.db.pool(),
        league_id,
        season_num,
        game_type_num,
        window,
    )
    .await?;
    let total_points_by_team: HashMap<i64, i32> = league_totals
        .iter()
        .map(|r| (r.team_id, r.points as i32))
        .collect();

    // Per-rostered-player totals across the whole league. We collect
    // the union of every team's player set and pull all of them in
    // one query rather than N queries; the query already returns one
    // row per (player_id) so a HashMap lookup gives constant-time
    // resolution per player below.
    let all_nhl_ids: Vec<i64> = teams_with_players
        .iter()
        .flat_map(|t| t.players.iter().map(|p| p.nhl_id))
        .collect();
    let player_totals = nhl_mirror::list_player_season_totals(
        state.db.pool(),
        &all_nhl_ids,
        season_num,
        game_type_num,
        window,
    )
    .await?;
    let points_by_player: HashMap<i64, i32> = player_totals
        .iter()
        .map(|r| (r.nhl_id, r.points as i32))
        .collect();

    // `daily_rankings` is append-only across seasons and game types, so
    // playoff Season Overview must clamp to `playoff_start()` or it
    // counts regular-season daily wins as playoff wins.
    let daily_window = if game_type() == 3 {
        DateWindow::since(playoff_start())
    } else {
        DateWindow::unbounded()
    };
    let daily_rankings = state
        .db
        .get_daily_ranking_stats(league_id, daily_window)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(
                league_id = %league_id,
                error = %e,
                "team_stats: daily ranking stats query failed; rendering zeros"
            );
            Vec::new()
        });
    let daily_rankings_map: HashMap<i64, crate::domain::models::db::TeamDailyRankingStats> =
        daily_rankings
            .into_iter()
            .map(|stats| (stats.team_id, stats))
            .collect();

    let mut response = Vec::new();
    for team in &teams_with_players {
        let total_points = total_points_by_team.get(&team.id).copied().unwrap_or(0);

        let mut player_stats: Vec<TopPlayerForTeam> = Vec::new();
        let mut nhl_team_points: HashMap<String, i32> = HashMap::new();
        let mut seen_players: HashSet<i64> = HashSet::new();

        for player in &team.players {
            if !seen_players.insert(player.nhl_id) {
                continue;
            }
            let points = points_by_player.get(&player.nhl_id).copied().unwrap_or(0);

            player_stats.push(TopPlayerForTeam {
                nhl_id: player.nhl_id,
                name: player.name.clone(),
                points,
                nhl_team: player.nhl_team.clone(),
                position: player.position.clone(),
                image_url: state.nhl_client.get_player_image_url(player.nhl_id),
                team_logo: state.nhl_client.get_team_logo_url(&player.nhl_team),
            });

            *nhl_team_points.entry(player.nhl_team.clone()).or_insert(0) += points;
        }

        player_stats.sort_by(|a, b| b.points.cmp(&a.points));
        let top_players = player_stats.into_iter().take(3).collect();

        let mut top_nhl_teams = nhl_team_points
            .into_iter()
            .map(|(nhl_team, points)| TopNhlTeamForFantasy {
                nhl_team: nhl_team.clone(),
                points,
                team_logo: state.nhl_client.get_team_logo_url(&nhl_team),
                team_name: state.nhl_client.get_team_name(&nhl_team),
            })
            .collect::<Vec<_>>();
        top_nhl_teams.sort_by(|a, b| b.points.cmp(&a.points));
        let top_nhl_teams = top_nhl_teams.into_iter().take(3).collect();

        let (daily_wins, daily_top_three, win_dates, top_three_dates) = daily_rankings_map
            .get(&team.id)
            .map(|stats| {
                (
                    stats.wins,
                    stats.top_three,
                    stats.win_dates.clone(),
                    stats.top_three_dates.clone(),
                )
            })
            .unwrap_or((0, 0, Vec::new(), Vec::new()));

        response.push(TeamStatsResponse {
            team_id: team.id,
            team_name: team.name.clone(),
            total_points,
            daily_wins,
            daily_top_three,
            win_dates,
            top_three_dates,
            top_players,
            top_nhl_teams,
        });
    }

    Ok(json_success(response))
}
