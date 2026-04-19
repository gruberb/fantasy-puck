use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use axum::{
    extract::{Query, State},
    Json,
};

use crate::api::dtos::*;
use crate::api::response::{json_success, ApiResponse};
use crate::api::routes::AppState;
use crate::api::{game_type, playoff_start, season};
use crate::error::Result;
use crate::domain::models::db::FantasyTeamWithPlayers;
use crate::infra::db::DateWindow;

pub async fn get_team_stats(
    State(state): State<Arc<AppState>>,
    Query(league_params): Query<LeagueParams>,
) -> Result<Json<ApiResponse<Vec<TeamStatsResponse>>>> {
    let league_id = &league_params.league_id;

    // 1. Get all fantasy teams with their players
    let teams = state.db.get_all_teams(league_id).await?;

    let mut teams_with_players: Vec<FantasyTeamWithPlayers> = Vec::new();
    for team in teams {
        teams_with_players.push(FantasyTeamWithPlayers {
            id: team.id,
            name: team.name,
            players: state.db.get_team_players(team.id).await?,
        });
    }

    // 2. Get NHL skater stats
    let stats = state
        .nhl_client
        .get_skater_stats(&season(), game_type())
        .await?;

    // 3. Calculate rankings
    let rankings = crate::domain::models::fantasy::TeamRanking::calculate_rankings(
        teams_with_players.clone(),
        stats.clone(),
    );

    // `daily_rankings` is append-only across seasons and game types, so
    // playoff Season Overview must clamp to `playoff_start()` or it
    // counts regular-season daily wins as playoff wins.
    let window = if game_type() == 3 {
        DateWindow::since(playoff_start())
    } else {
        DateWindow::unbounded()
    };

    let daily_rankings = state
        .db
        .get_daily_ranking_stats(league_id, window)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(
                league_id = %league_id,
                error = %e,
                "team_stats: daily ranking stats query failed; rendering zeros"
            );
            Vec::new()
        });

    let daily_rankings_map: HashMap<i64, crate::domain::models::db::TeamDailyRankingStats> = daily_rankings
        .into_iter()
        .map(|stats| (stats.team_id, stats))
        .collect();

    // 5. Process each team's stats
    let mut response = Vec::new();

    for team in &teams_with_players {
        let default_ranking = crate::domain::models::fantasy::TeamRanking::default();
        let team_ranking = rankings
            .iter()
            .find(|r| r.team_id == team.id)
            .unwrap_or(&default_ranking);

        // Calculate player stats for this team
        let mut player_stats = Vec::new();
        let mut nhl_team_points: HashMap<String, i32> = HashMap::new();

        // Track unique players to avoid duplicates
        let mut seen_players = HashSet::new();

        for player in &team.players {
            // Skip if already processed
            if !seen_players.insert(player.nhl_id) {
                continue;
            }

            // Calculate points for this player
            let mut goals = 0;
            let mut assists = 0;

            // Look for goals
            if let Some(p) = stats.goals.iter().find(|p| p.id as i64 == player.nhl_id) {
                goals = p.value as i32;
            }

            // Look for assists
            if let Some(p) = stats.assists.iter().find(|p| p.id as i64 == player.nhl_id) {
                assists = p.value as i32;
            }

            let points = goals + assists;

            // Add to player stats
            player_stats.push(TopPlayerForTeam {
                nhl_id: player.nhl_id,
                name: player.name.clone(),
                points,
                nhl_team: player.nhl_team.clone(),
                position: player.position.clone(),
                image_url: state.nhl_client.get_player_image_url(player.nhl_id),
                team_logo: state.nhl_client.get_team_logo_url(&player.nhl_team),
            });

            // Increment this NHL team's points
            *nhl_team_points.entry(player.nhl_team.clone()).or_insert(0) += points;
        }

        // Sort players by points and take top 3
        player_stats.sort_by(|a, b| b.points.cmp(&a.points));
        let top_players = player_stats.into_iter().take(3).collect();

        // Sort NHL teams by points and take top 3
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

        // Get daily ranking stats
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
            total_points: team_ranking.total_points,
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
