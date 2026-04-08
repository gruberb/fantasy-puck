use serde::{Deserialize, Serialize};

use crate::models::nhl::SeriesStatus;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FantasyTeamInfo {
    pub team_id: i64,
    pub team_name: String,
}

/// Form indicator for a player's recent performance
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerForm {
    pub games: usize,
    pub goals: i32,
    pub assists: i32,
    pub points: i32,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesStatusResponse {
    pub round: u32,
    pub series_title: String,
    pub top_seed_team_abbrev: String,
    pub top_seed_wins: u32,
    pub bottom_seed_team_abbrev: String,
    pub bottom_seed_wins: u32,
    pub game_number_of_series: u32,
}

impl From<SeriesStatus> for SeriesStatusResponse {
    fn from(status: SeriesStatus) -> Self {
        Self {
            round: status.round,
            series_title: status.series_title,
            top_seed_team_abbrev: status.top_seed_team_abbrev,
            top_seed_wins: status.top_seed_wins,
            bottom_seed_team_abbrev: status.bottom_seed_team_abbrev,
            bottom_seed_wins: status.bottom_seed_wins,
            game_number_of_series: status.game_number_of_series,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GamesSummaryResponse {
    pub total_games: usize,
    pub total_teams_playing: usize,
    pub team_players_count: Vec<TeamPlayerCountResponse>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamPlayerCountResponse {
    pub nhl_team: String,
    pub player_count: usize,
}
