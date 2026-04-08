use serde::{Deserialize, Serialize};

use crate::api::dtos::match_day::MatchDayFantasyTeamResponse;
use crate::api::dtos::SeriesStatusResponse;
use crate::models::nhl::GameState;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodaysGamesResponse {
    pub date: String,
    pub games: Vec<GameResponse>,
    pub summary: crate::api::dtos::common::GamesSummaryResponse,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fantasy_teams: Option<Vec<MatchDayFantasyTeamResponse>>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameResponse {
    pub id: u32,
    pub home_team: String,
    pub away_team: String,
    pub start_time: String,
    pub venue: String,
    pub home_team_players: Vec<FantasyPlayerResponse>,
    pub away_team_players: Vec<FantasyPlayerResponse>,
    pub home_team_logo: String,
    pub away_team_logo: String,
    pub home_score: Option<i32>,
    pub away_score: Option<i32>,
    pub game_state: GameState,
    pub period: Option<String>,
    pub series_status: Option<SeriesStatusResponse>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FantasyPlayerResponse {
    pub fantasy_team: String,
    pub fantasy_team_id: i64,
    pub player_name: String,
    pub position: String,
    pub nhl_id: i64,
    pub image_url: String,
    pub goals: i32,
    pub assists: i32,
    pub points: i32,
}
