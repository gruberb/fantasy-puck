use serde::{Deserialize, Serialize};

/// Raw signal data computed from existing stats
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsightsSignals {
    pub hot_players: Vec<HotPlayerSignal>,
    pub cup_contenders: Vec<ContenderSignal>,
    pub todays_games: Vec<TodaysGameSignal>,
    pub fantasy_race: Vec<FantasyRaceSignal>,
    pub sleeper_alerts: Vec<SleeperAlertSignal>,
    pub news_headlines: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HotPlayerSignal {
    pub name: String,
    pub nhl_team: String,
    pub position: String,
    pub form_goals: i32,
    pub form_assists: i32,
    pub form_points: i32,
    pub form_games: usize,
    pub playoff_points: i32,
    pub fantasy_team: Option<String>,
    pub image_url: String,
    // NHL Edge data
    #[serde(default)]
    pub top_speed: Option<f64>,
    #[serde(default)]
    pub top_shot_speed: Option<f64>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContenderSignal {
    pub team_abbrev: String,
    pub series_title: String,
    pub wins: u32,
    pub opponent_abbrev: String,
    pub opponent_wins: u32,
    pub round: u32,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlayerLeader {
    pub name: String,
    pub position: String,
    pub value: i32,
    pub headshot: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GoalieStats {
    pub name: String,
    pub record: String,
    pub gaa: f64,
    pub save_pctg: f64,
    pub shutouts: i32,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodaysGameSignal {
    pub home_team: String,
    pub away_team: String,
    pub home_record: String,
    pub away_record: String,
    pub venue: String,
    pub start_time: String,
    pub series_context: Option<String>,
    pub is_elimination: bool,
    // Players to Watch (last 5 games)
    pub points_leaders: Option<(PlayerLeader, PlayerLeader)>,
    pub goals_leaders: Option<(PlayerLeader, PlayerLeader)>,
    pub assists_leaders: Option<(PlayerLeader, PlayerLeader)>,
    // Goaltending
    pub home_goalie: Option<GoalieStats>,
    pub away_goalie: Option<GoalieStats>,
    // Standings context
    #[serde(default)]
    pub home_streak: Option<String>,
    #[serde(default)]
    pub away_streak: Option<String>,
    #[serde(default)]
    pub home_l10: Option<String>,
    #[serde(default)]
    pub away_l10: Option<String>,
    // Last game result
    #[serde(default)]
    pub home_last_result: Option<String>,
    #[serde(default)]
    pub away_last_result: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FantasyRaceSignal {
    pub team_name: String,
    pub total_points: i32,
    pub rank: usize,
    pub players_active_today: usize,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SleeperAlertSignal {
    pub name: String,
    pub nhl_team: String,
    pub fantasy_team: Option<String>,
    pub points: i32,
    pub goals: i32,
    pub assists: i32,
}

/// Final response with LLM narratives + raw signal data
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsightsResponse {
    pub generated_at: String,
    pub narratives: InsightsNarratives,
    pub signals: InsightsSignals,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsightsNarratives {
    pub todays_watch: String,
    #[serde(default)]
    pub game_narratives: Vec<String>,
    pub hot_players: String,
    pub cup_contenders: String,
    pub fantasy_race: String,
    pub sleeper_watch: String,
}
