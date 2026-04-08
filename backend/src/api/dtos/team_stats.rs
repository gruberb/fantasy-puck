use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamStatsResponse {
    pub team_id: i64,
    pub team_name: String,
    pub total_points: i32,
    pub daily_wins: i32,
    pub daily_top_three: i32,
    pub win_dates: Vec<String>,
    pub top_three_dates: Vec<String>,
    pub top_players: Vec<TopPlayerForTeam>,
    pub top_nhl_teams: Vec<TopNhlTeamForFantasy>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopPlayerForTeam {
    pub nhl_id: i64,
    pub name: String,
    pub points: i32,
    pub nhl_team: String,
    pub position: String,
    pub image_url: String,
    pub team_logo: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopNhlTeamForFantasy {
    pub nhl_team: String,
    pub points: i32,
    pub team_logo: String,
    pub team_name: String,
}
