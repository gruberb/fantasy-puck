use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamPointsResponse {
    pub team_id: i64,
    pub team_name: String,
    pub players: Vec<PlayerStatsResponse>,
    pub team_totals: TeamTotalsResponse,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamTotalsResponse {
    pub goals: i32,
    pub assists: i32,
    pub total_points: i32,
}

#[derive(Serialize)]
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
