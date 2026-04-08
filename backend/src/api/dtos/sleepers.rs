use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SleeperStatsResponse {
    pub id: i64,
    pub nhl_id: i64,
    pub name: String,
    pub nhl_team: String,
    pub position: String,
    pub fantasy_team: Option<String>,
    pub fantasy_team_id: Option<i64>,
    pub goals: i32,
    pub assists: i32,
    pub total_points: i32,
    pub plus_minus: Option<i32>,
    pub time_on_ice: Option<String>,
    pub image_url: String,
    pub team_logo: String,
}
