use serde::{Deserialize, Serialize};

use crate::api::dtos::common::PlayerForm;
use crate::api::dtos::common::{GamesSummaryResponse, SeriesStatusResponse};
use crate::domain::models::nhl::GameState;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchDayResponse {
    pub date: String,
    pub games: Vec<MatchDayGameResponse>,
    pub fantasy_teams: Vec<MatchDayFantasyTeamResponse>,
    pub summary: GamesSummaryResponse,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchDayGameResponse {
    pub id: u32,
    pub home_team: String,
    pub away_team: String,
    pub start_time: String,
    pub venue: String,
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
pub struct MatchDayFantasyTeamResponse {
    pub team_id: i64,
    pub team_name: String,
    pub players_in_action: Vec<FantasyPlayerExtendedResponse>,
    pub total_players_today: usize,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FantasyPlayerExtendedResponse {
    pub fantasy_team: String,
    pub fantasy_team_id: i64,
    pub player_name: String,
    pub position: String,
    pub nhl_id: i64,
    pub image_url: String,
    pub team_logo: String,
    pub nhl_team: String,

    // Game stats (if game is in progress or completed)
    pub goals: i32,
    pub assists: i32,
    pub points: i32,

    // Playoff totals
    pub playoff_goals: i32,
    pub playoff_assists: i32,
    pub playoff_points: i32,
    pub playoff_games: i32,

    // Form data (last n games)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form: Option<PlayerForm>,

    // Time on ice data for current/last game
    pub time_on_ice: Option<String>,
}
