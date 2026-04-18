use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::api::dtos::stats::PlayerHighlightResponse;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RankingResponse {
    pub rank: usize,
    pub team_id: i64,
    pub team_name: String,
    pub goals: i32,
    pub assists: i32,
    pub total_points: i32,
}

#[derive(Serialize, Deserialize)]
pub struct DailyRankingsResponse {
    pub date: String,
    pub rankings: Vec<DailyFantasyRankingResponse>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyFantasyRankingResponse {
    pub rank: usize,
    pub team_id: i64,
    pub team_name: String,
    pub daily_assists: i32,
    pub daily_goals: i32,
    pub daily_points: i32,
    pub player_highlights: Vec<PlayerHighlightResponse>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayoffRankingResponse {
    pub rank: usize,
    pub team_id: i64,
    pub team_name: String,
    pub teams_in_playoffs: i32,
    pub total_teams: i32,
    pub players_in_playoffs: i32,
    pub total_players: i32,
    pub top_ten_players_count: i32,
    pub playoff_score: i32,
    pub total_points: i32,
}

impl PlayoffRankingResponse {
    pub fn compute(
        team_id: i64,
        team_name: String,
        total_points: i32,
        nhl_teams: &[String],
        player_nhl_teams: &[String],
        top_skater_count: i32,
        teams_in_playoffs: &HashSet<String>,
    ) -> Self {
        let total_teams = nhl_teams.len() as i32;
        let in_playoffs = nhl_teams
            .iter()
            .filter(|t| teams_in_playoffs.contains(t.as_str()))
            .count() as i32;

        let total_players = player_nhl_teams.len() as i32;
        let players_in = player_nhl_teams
            .iter()
            .filter(|t| teams_in_playoffs.contains(t.as_str()))
            .count() as i32;

        let playoff_score = in_playoffs * 10 + players_in * 5 + top_skater_count * 20;

        PlayoffRankingResponse {
            rank: 0, // Set after sorting
            team_id,
            team_name,
            teams_in_playoffs: in_playoffs,
            total_teams,
            players_in_playoffs: players_in,
            total_players,
            top_ten_players_count: top_skater_count,
            playoff_score,
            total_points,
        }
    }
}
