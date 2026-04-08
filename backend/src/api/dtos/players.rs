use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NhlTeamPlayersResponse {
    pub nhl_team: String,
    pub team_logo: String,
    pub players: Vec<PlayerWithTeamResponse>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerWithTeamResponse {
    pub nhl_id: i64,
    pub name: String,
    pub fantasy_team_id: i64,
    pub fantasy_team_name: String,
    pub position: String,
    pub nhl_team: String,
    pub image_url: String,
}

/// A player from an NHL team roster (used for the roster lookup endpoint)
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NhlRosterPlayer {
    pub nhl_id: u32,
    pub name: String,
    pub position: String,
    pub team: String,
    pub headshot_url: String,
}
