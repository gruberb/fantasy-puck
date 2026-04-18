use serde::{Deserialize, Serialize};

use crate::domain::prediction::series_projection::SeriesStateCode;

/// Raw signal data computed from existing stats. Pulse-facing personal
/// surfaces (Race Odds, Rivalry, Fantasy Race sparklines) are NOT in here —
/// they live on `/api/pulse` and `/api/race-odds`. Insights is NHL-centric:
/// today's games, hot/cold skaters, the bracket, and news.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsightsSignals {
    pub hot_players: Vec<HotPlayerSignal>,
    #[serde(default)]
    pub cold_hands: Vec<HotPlayerSignal>,
    pub todays_games: Vec<TodaysGameSignal>,
    pub news_headlines: Vec<String>,
    /// Per-team series state + odds across every active playoff series.
    #[serde(default)]
    pub series_projections: Vec<TeamSeriesProjection>,
    /// True when Hot/Cold data was sourced from regular-season leaders
    /// (pre-playoff fallback). Drives the UI's "season pts" vs "playoff
    /// pts" label and keeps Claude from claiming playoff stats that don't
    /// exist yet.
    #[serde(default)]
    pub hot_cold_is_regular_season: bool,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TeamSeriesProjection {
    pub team_abbrev: String,
    pub team_name: String,
    pub opponent_abbrev: String,
    pub opponent_name: String,
    pub round: u32,
    pub wins: u32,
    pub opponent_wins: u32,
    pub series_state: SeriesStateCode,
    pub series_label: String,
    pub odds_to_advance: f32,
    pub games_remaining: u32,
    /// Regular-season standings points for this team, used as a strength
    /// proxy so the bracket can show who's the favorite beyond the raw W-L.
    #[serde(default)]
    pub team_rating: Option<f32>,
    /// Regular-season standings points for the opponent — same source as
    /// `team_rating`, exposed for convenient frontend diffing.
    #[serde(default)]
    pub opponent_rating: Option<f32>,
    /// Fantasy teams that own players on this NHL team in the active
    /// league (empty in the global/no-league view).
    #[serde(default)]
    pub rostered_tags: Vec<RosteredPlayerTag>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RosteredPlayerTag {
    pub fantasy_team_name: String,
    pub count: usize,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HotPlayerSignal {
    /// NHL player id — enables the frontend to link the card to the public
    /// player profile at `nhl.com/player/{id}`.
    pub nhl_id: i64,
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
    /// Fantasy-team ownership tags — "your team has 3 players in this game".
    #[serde(default)]
    pub rostered_player_tags: Vec<RosteredPlayerTag>,
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
    /// Narrative for the Bracket / Stanley Cup Odds section.
    #[serde(default)]
    pub bracket: String,
}
