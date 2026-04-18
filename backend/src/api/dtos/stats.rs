use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::api::dtos::common::{FantasyTeamInfo, PlayerForm};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsolidatedPlayerStats {
    pub id: i64,
    pub first_name: String,
    pub last_name: String,
    pub sweater_number: Option<u32>,
    pub headshot: String,
    pub team_abbrev: String,
    pub team_name: String,
    pub team_logo: String,
    pub position: String,
    pub stats: HashMap<String, i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fantasy_team: Option<FantasyTeamInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form: Option<PlayerForm>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerHighlightResponse {
    pub player_name: String,
    pub points: i32,
    pub nhl_team: String,
    pub nhl_id: i64,
    pub image_url: String,
}
