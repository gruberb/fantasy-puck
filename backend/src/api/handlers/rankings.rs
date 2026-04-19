//! Rankings handlers.
//!
//! Post-Phase-3 all three handlers are pure database reads. The NHL
//! mirror pollers (`infra/jobs/{meta_poller, live_poller}`) keep the
//! source tables fresh; this module joins them with the per-league
//! fantasy_* tables and applies the domain ranking rules.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use axum::{
    extract::{Query, State},
    Json,
};

use crate::api::dtos::*;
use crate::api::dtos::conversion::IntoResponse;
use crate::api::response::{json_success, ApiResponse};
use crate::api::routes::AppState;
use crate::api::{game_type, season};
use crate::domain::models::db::FantasyTeamWithPlayers;
use crate::domain::models::fantasy::TeamRanking;
use crate::domain::models::nhl::PlayoffCarousel;
use crate::domain::services::rankings::{
    build_daily_rankings, calculate_team_rankings, DailyPlayerStat, SeasonSkaterStat,
};
use crate::error::Result;
use crate::infra::db::nhl_mirror;
use crate::infra::nhl::urls::parse_date_param;

// ---------------------------------------------------------------------
// Overall season rankings: GET /api/fantasy/rankings
// ---------------------------------------------------------------------

pub async fn get_rankings(
    State(state): State<Arc<AppState>>,
    Query(league_params): Query<LeagueParams>,
) -> Result<Json<ApiResponse<Vec<TeamRanking>>>> {
    let league_id = &league_params.league_id;
    let teams = load_league_teams(&state, league_id).await?;
    let stats = load_skater_leaderboard(&state).await?;
    Ok(json_success(calculate_team_rankings(teams, &stats)))
}

// ---------------------------------------------------------------------
// Daily rankings: GET /api/fantasy/rankings/daily
// ---------------------------------------------------------------------

pub async fn get_daily_rankings(
    State(state): State<Arc<AppState>>,
    Query(params): Query<DailyRankingsParams>,
) -> Result<Json<ApiResponse<DailyRankingsResponse>>> {
    let league_id = &params.league_id;
    let date = parse_date_param(params.date)?;

    let rows = nhl_mirror::list_league_player_stats_for_date(state.db.pool(), league_id, &date)
        .await?
        .into_iter()
        .map(|r| DailyPlayerStat {
            team_id: r.team_id,
            team_name: r.team_name,
            nhl_id: r.nhl_id,
            player_name: r.player_name,
            nhl_team: r.nhl_team,
            goals: r.goals,
            assists: r.assists,
            points: r.points,
        })
        .collect::<Vec<_>>();

    let rankings = build_daily_rankings(rows);
    let response = DailyRankingsResponse {
        date: date.clone(),
        rankings: rankings.into_iter().map(|r| r.into_response()).collect(),
    };
    Ok(json_success(response))
}

// ---------------------------------------------------------------------
// Playoff rankings: GET /api/fantasy/rankings/playoffs
// ---------------------------------------------------------------------

pub async fn get_playoff_rankings(
    State(state): State<Arc<AppState>>,
    Query(league_params): Query<LeagueParams>,
) -> Result<Json<ApiResponse<Vec<PlayoffRankingResponse>>>> {
    let league_id = &league_params.league_id;

    let teams = load_league_teams(&state, league_id).await?;
    let stats = load_skater_leaderboard(&state).await?;
    let base_rankings = calculate_team_rankings(teams.clone(), &stats);

    let bets = state.db.get_fantasy_bets_by_nhl_team(league_id).await?;
    let bets_by_team: HashMap<i64, Vec<String>> = bets
        .into_iter()
        .map(|b| (b.team_id, b.bets.into_iter().map(|bet| bet.nhl_team).collect()))
        .collect();

    let teams_in_playoffs = load_teams_in_playoffs(&state).await?;

    // Top 10 scorers: stats are already sorted by points desc.
    let mut top_ten: Vec<(i64, i32)> = stats
        .iter()
        .take(10)
        .map(|s| (s.nhl_id, s.goals + s.assists))
        .collect();
    top_ten.sort_by(|a, b| b.1.cmp(&a.1));
    let top_ids: HashSet<i64> = top_ten.iter().map(|(id, _)| *id).collect();

    let top_count_by_team: HashMap<i64, i32> = teams
        .iter()
        .map(|twp| {
            let n = twp
                .players
                .iter()
                .filter(|p| top_ids.contains(&p.nhl_id))
                .count() as i32;
            (twp.id, n)
        })
        .collect();

    let mut response: Vec<PlayoffRankingResponse> = teams
        .iter()
        .map(|twp| {
            let total_points = base_rankings
                .iter()
                .find(|r| r.team_id == twp.id)
                .map(|r| r.total_points)
                .unwrap_or(0);
            let nhl_teams = bets_by_team.get(&twp.id).cloned().unwrap_or_default();
            let player_nhl_teams: Vec<String> =
                twp.players.iter().map(|p| p.nhl_team.clone()).collect();
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

    response.sort_by(|a, b| b.playoff_score.cmp(&a.playoff_score));
    for (i, entry) in response.iter_mut().enumerate() {
        entry.rank = i + 1;
    }
    Ok(json_success(response))
}

// ---------------------------------------------------------------------
// Private helpers — DB loaders shared across the three handlers.
// ---------------------------------------------------------------------

async fn load_league_teams(
    state: &Arc<AppState>,
    league_id: &str,
) -> Result<Vec<FantasyTeamWithPlayers>> {
    let teams = state.db.get_all_teams(league_id).await?;
    let mut out = Vec::with_capacity(teams.len());
    for team in teams {
        out.push(FantasyTeamWithPlayers {
            id: team.id,
            name: team.name,
            players: state.db.get_team_players(team.id).await?,
        });
    }
    Ok(out)
}

/// Load the skater leaderboard from the mirror and adapt to the
/// shape the domain service consumes.
async fn load_skater_leaderboard(state: &Arc<AppState>) -> Result<Vec<SeasonSkaterStat>> {
    let rows = nhl_mirror::list_skater_season_stats(
        state.db.pool(),
        season() as i32,
        game_type() as i16,
    )
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| SeasonSkaterStat {
            nhl_id: r.player_id,
            goals: r.goals,
            assists: r.assists,
        })
        .collect())
}

/// Parse the playoff carousel JSONB out of `nhl_playoff_bracket` and
/// extract the set of NHL team abbrevs that are still alive.
async fn load_teams_in_playoffs(state: &Arc<AppState>) -> Result<HashSet<String>> {
    let carousel_json: Option<serde_json::Value> = sqlx::query_scalar(
        "SELECT carousel FROM nhl_playoff_bracket WHERE season = $1",
    )
    .bind(season() as i32)
    .fetch_optional(state.db.pool())
    .await
    .map_err(crate::error::Error::Database)?;

    let Some(json) = carousel_json else {
        return Ok(HashSet::new());
    };
    let carousel: PlayoffCarousel = match serde_json::from_value(json) {
        Ok(c) => c,
        Err(_) => return Ok(HashSet::new()),
    };
    let val = serde_json::to_value(carousel)
        .map_err(|e| crate::error::Error::Internal(format!("serialization error: {e}")))?;
    let parsed: PlayoffCarouselResponse = serde_json::from_value(val)
        .map_err(|e| crate::error::Error::Internal(format!("conversion error: {e}")))?;
    let computed = parsed.with_computed_state();
    Ok(computed.teams_in_playoffs.into_iter().collect())
}
