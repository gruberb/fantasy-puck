use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use axum::{
    extract::{Query, State},
    Json,
};
use futures::{stream, StreamExt, TryStreamExt};
use tracing::error;

use crate::api::dtos::*;
use crate::api::dtos::conversion::IntoResponse;
use crate::api::response::{json_success, ApiResponse};
use crate::api::routes::AppState;
use crate::api::{GAME_TYPE, SEASON};
use crate::Error;
use crate::error::Result;
use crate::models::db::FantasyTeamWithPlayers;
use crate::models::fantasy::{DailyRanking, TeamDailyPerformance, TeamRanking};
use crate::utils::api::parse_date_param;
use crate::utils::fantasy::process_game_performances;

/// Get current total rankings of all Fantasy Teams in a league
pub async fn get_rankings(
    State(state): State<Arc<AppState>>,
    Query(league_params): Query<LeagueParams>,
) -> Result<Json<ApiResponse<Vec<TeamRanking>>>> {
    let league_id = &league_params.league_id;
    let teams = state.db.get_all_teams(league_id).await?;

    let mut list_of_teams_with_players: Vec<FantasyTeamWithPlayers> = Vec::default();

    for team in teams {
        list_of_teams_with_players.push(FantasyTeamWithPlayers {
            id: team.id,
            name: team.name,
            players: state.db.get_team_players(team.id).await?,
        })
    }

    let stats = state
        .nhl_client
        .get_skater_stats(&SEASON, GAME_TYPE)
        .await?;

    Ok(json_success(TeamRanking::calculate_rankings(
        list_of_teams_with_players,
        stats,
    )))
}

pub async fn get_daily_rankings(
    State(state): State<Arc<AppState>>,
    Query(params): Query<DailyRankingsParams>,
) -> Result<Json<ApiResponse<DailyRankingsResponse>>> {
    let league_id = &params.league_id;

    // Validate date format (has to be YYYY-MM-DD)
    let date = parse_date_param(params.date)?;

    // Fetch the games for the specified date
    let games_response = state.nhl_client.get_schedule_by_date(&date).await?;

    // Extract LIVE or FINISHED games for this date
    let games_for_date = games_response
        .games_for_date(&date)
        .into_iter()
        .filter(|game| game.game_state.is_completed() || game.game_state.is_live())
        .collect::<Vec<_>>();

    // If no Game is found, we return an Error
    if games_for_date.is_empty() {
        return Err(Error::NotFound(format!("No games found for date {}", date)));
    }

    // Process all completed games and aggregate team performances
    let all_team_performances = stream::iter(games_for_date)
        .map(|game| {
            let state_cloned = state.clone();
            let league_id_owned = league_id.to_string();
            async move {
                // Try to get boxscore for this game
                let boxscore = state_cloned
                    .nhl_client
                    .get_game_boxscore(game.id)
                    .await
                    .map_err(|e| {
                        error!(
                            "Warning: Could not fetch boxscore for game {}: {}",
                            game.id, e
                        );
                        Error::Internal(
                            "Internal Server Error trying to get NHL Game information".to_string(),
                        )
                    })?;

                // Get fantasy players for both teams
                let home_team = game.home_team.abbrev.as_str();
                let away_team = game.away_team.abbrev.as_str();
                let fantasy_players = state_cloned
                    .db
                    .get_fantasy_players_for_nhl_teams(&[home_team, away_team], &league_id_owned)
                    .await?;

                // Process performances for this game
                Ok::<Vec<TeamDailyPerformance>, Error>(process_game_performances(
                    &fantasy_players,
                    &boxscore,
                ))
            }
        })
        .buffer_unordered(4) // Process up to 4 games concurrently
        .try_fold(
            HashMap::<i64, TeamDailyPerformance>::new(),
            |mut acc, performances| async move {
                // Merge these performances into the accumulator
                for perf in performances {
                    acc.entry(perf.team_id)
                        .and_modify(|existing| {
                            existing
                                .player_performances
                                .extend(perf.player_performances.clone());
                            existing.total_points += perf.total_points;
                            existing.total_assists += perf.total_assists;
                            existing.total_goals += perf.total_goals;
                        })
                        .or_insert(perf);
                }
                Ok(acc)
            },
        )
        .await?;

    // Convert to rankings domain model
    let daily_rankings = DailyRanking::build_rankings(all_team_performances);

    // Convert to API response DTOs
    let response_rankings = daily_rankings
        .into_iter()
        .map(|r| r.into_response())
        .collect();

    Ok(json_success(DailyRankingsResponse {
        date,
        rankings: response_rankings,
    }))
}

/// Compute playoff rankings — combines rankings, team bets, player rosters,
/// top skaters, and playoff state into a single response.
pub async fn get_playoff_rankings(
    State(state): State<Arc<AppState>>,
    Query(league_params): Query<LeagueParams>,
) -> Result<Json<ApiResponse<Vec<PlayoffRankingResponse>>>> {
    let league_id = &league_params.league_id;

    // 1. Get all fantasy teams with players
    let teams = state.db.get_all_teams(league_id).await?;
    let mut teams_with_players: Vec<FantasyTeamWithPlayers> = Vec::new();
    for team in &teams {
        teams_with_players.push(FantasyTeamWithPlayers {
            id: team.id,
            name: team.name.clone(),
            players: state.db.get_team_players(team.id).await?,
        });
    }

    // 2. Get NHL skater stats and calculate base rankings
    let stats = state.nhl_client.get_skater_stats(&SEASON, GAME_TYPE).await?;
    let base_rankings = TeamRanking::calculate_rankings(teams_with_players.clone(), stats.clone());

    // 3. Get team bets (which NHL teams each fantasy team has players on)
    let bets = state.db.get_fantasy_bets_by_nhl_team(league_id).await?;
    let bets_by_team: HashMap<i64, Vec<String>> = bets
        .into_iter()
        .map(|b| (b.team_id, b.bets.into_iter().map(|bet| bet.nhl_team).collect()))
        .collect();

    // 4. Get playoff data to determine which teams are still in
    let playoff_raw = state
        .nhl_client
        .get_playoff_carousel(SEASON.to_string())
        .await
        .map_err(|_| crate::error::Error::NotFound("Playoff data not available".into()))?;

    let teams_in_playoffs: HashSet<String> = if let Some(carousel) = playoff_raw {
        let val = serde_json::to_value(carousel)
            .map_err(|e| crate::error::Error::Internal(format!("serialization error: {e}")))?;
        let parsed: PlayoffCarouselResponse = serde_json::from_value(val)
            .map_err(|e| crate::error::Error::Internal(format!("conversion error: {e}")))?;
        let computed = parsed.with_computed_state();
        computed.teams_in_playoffs.into_iter().collect()
    } else {
        HashSet::new()
    };

    // 5. Get top 10 skaters and count per fantasy team
    let top_skaters = state.nhl_client.get_skater_stats(&SEASON, GAME_TYPE).await?;
    let mut top_player_ids: Vec<(i64, i32)> = top_skaters
        .points
        .iter()
        .take(10)
        .map(|p| (p.id as i64, p.value as i32))
        .collect();
    top_player_ids.sort_by(|a, b| b.1.cmp(&a.1));

    let top_ids: HashSet<i64> = top_player_ids.iter().map(|(id, _)| *id).collect();

    let mut top_count_by_team: HashMap<i64, i32> = HashMap::new();
    for twp in &teams_with_players {
        let count = twp.players.iter().filter(|p| top_ids.contains(&p.nhl_id)).count() as i32;
        top_count_by_team.insert(twp.id, count);
    }

    // 6. Build response
    let mut response: Vec<PlayoffRankingResponse> = teams_with_players
        .iter()
        .map(|twp| {
            let base = base_rankings.iter().find(|r| r.team_id == twp.id);
            let total_points = base.map(|r| r.total_points).unwrap_or(0);

            let nhl_teams = bets_by_team.get(&twp.id).cloned().unwrap_or_default();
            let player_nhl_teams: Vec<String> = twp.players.iter().map(|p| p.nhl_team.clone()).collect();
            let top_count = top_count_by_team.get(&twp.id).copied().unwrap_or(0);

            PlayoffRankingResponse::compute(
                twp.id,
                twp.name.clone(),
                total_points,
                &nhl_teams,
                &player_nhl_teams,
                top_count,
                &teams_in_playoffs,
            )
        })
        .collect();

    // Sort by playoff score descending
    response.sort_by(|a, b| b.playoff_score.cmp(&a.playoff_score));

    // Assign ranks
    for (i, entry) in response.iter_mut().enumerate() {
        entry.rank = i + 1;
    }

    Ok(json_success(response))
}
