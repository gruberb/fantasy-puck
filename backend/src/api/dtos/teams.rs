use serde::{Deserialize, Serialize};

use crate::domain::prediction::grade::{GradeReport, PlayerBucket, RemainingImpact};
use crate::domain::prediction::series_projection::SeriesStateCode;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TeamPointsResponse {
    pub team_id: i64,
    pub team_name: String,
    pub players: Vec<PlayerStatsResponse>,
    pub team_totals: TeamTotalsResponse,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnosis: Option<TeamDiagnosis>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TeamTotalsResponse {
    pub goals: i32,
    pub assists: i32,
    pub total_points: i32,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlayerStatsResponse {
    pub name: String,
    pub nhl_team: String,
    pub nhl_id: i64,
    pub position: String,
    pub goals: i32,
    pub assists: i32,
    pub total_points: i32,
    pub image_url: String,
    pub team_logo: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub breakdown: Option<PlayerBreakdown>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlayerBreakdown {
    pub games_played: u32,
    pub sog: i32,
    pub pim: i32,
    pub plus_minus: i32,
    pub hits: i32,
    pub toi_seconds_per_game: i32,
    pub projected_ppg: f32,
    pub active_prob: f32,
    pub toi_multiplier: f32,
    pub grade: GradeReport,
    pub remaining_impact: RemainingImpact,
    pub series_state: SeriesStateCode,
    pub bucket: PlayerBucket,
    pub recent_games: Vec<PlayerRecentGameCell>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlayerRecentGameCell {
    pub game_date: String,
    pub opponent: String,
    pub toi_seconds: Option<i32>,
    pub goals: i32,
    pub assists: i32,
    pub points: i32,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TeamDiagnosis {
    pub headline: String,
    pub narrative_markdown: String,
    pub league_rank: i32,
    pub league_size: i32,
    pub gap_to_first: i32,
    pub gap_to_third: i32,
    pub concentration_by_team: Vec<TeamConcentrationCell>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TeamConcentrationCell {
    pub nhl_team: String,
    pub rostered: i32,
    pub team_playoff_points: i32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FantasyTeamBetsResponse {
    pub team_id: i64,
    pub team_name: String,
    pub bets: Vec<NhlBetCountResponse>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NhlBetCountResponse {
    pub nhl_team: String,
    pub nhl_team_name: String,
    pub num_players: i64,
    pub team_logo: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FantasyTeamResponse {
    pub team_id: i64,
    pub team_name: String,
    pub players: Vec<String>,
}
