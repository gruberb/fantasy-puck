use serde::Deserialize;

// League-scoped query parameter required by all fantasy endpoints
#[derive(Deserialize)]
pub struct LeagueParams {
    pub league_id: String,
}

// Add query parameters for the games_by_date endpoint
#[derive(Deserialize)]
pub struct GamesByDateParams {
    pub date: String, // Required date parameter in YYYY-MM-DD format
}

// Add query parameters for the daily_rankings endpoint
#[derive(Deserialize)]
pub struct DailyRankingsParams {
    pub date: String, // Required date parameter in YYYY-MM-DD format
    pub league_id: String,
}

// Add query parameters for including form data
#[derive(Deserialize)]
pub struct TopSkatersParams {
    pub limit: u32,
    pub season: u32,
    pub game_type: u8,
    #[serde(default)]
    pub include_form: bool,
    pub form_games: usize,
    /// Optional league_id to include fantasy team ownership info
    pub league_id: Option<String>,
}
