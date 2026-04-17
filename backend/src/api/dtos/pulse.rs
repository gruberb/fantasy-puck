use serde::{Deserialize, Serialize};

use crate::utils::series_projection::SeriesStateCode;

// ---------------------------------------------------------------------------
// Top-level response
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PulseResponse {
    pub generated_at: String,
    /// Summary of the requesting user's team — null when no `my_team_id` could be resolved.
    pub my_team: Option<MyTeamStatus>,
    /// Per-fantasy-team roster × series grid. One entry per fantasy team in the league.
    pub series_forecast: Vec<FantasyTeamForecast>,
    pub my_goalies_tonight: Vec<MyGoalieSignal>,
    pub my_games_tonight: Vec<MyGameTonight>,
    pub league_board: Vec<LeagueBoardEntry>,
    pub has_games_today: bool,
    pub has_live_games: bool,
}

// ---------------------------------------------------------------------------
// "My team" status block
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MyTeamStatus {
    pub team_id: i64,
    pub team_name: String,
    pub rank: usize,
    pub total_points: i32,
    pub points_today: i32,
    pub players_active_today: usize,
    pub total_roster_size: usize,
}

// ---------------------------------------------------------------------------
// Series Forecast (flagship)
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FantasyTeamForecast {
    pub team_id: i64,
    pub team_name: String,
    pub total_players: usize,
    /// Sum: players on teams already eliminated.
    pub players_eliminated: usize,
    /// Sum: players on teams trailing 0-3 or 1-3 (one loss from elimination).
    pub players_facing_elimination: usize,
    /// Sum: players on teams trailing but not yet facing elimination.
    pub players_trailing: usize,
    /// Sum: players on teams currently ahead or about to advance.
    pub players_leading: usize,
    /// Sum: players on teams that have advanced to the next round.
    pub players_advanced: usize,
    /// Per-player cell data for the grid render.
    pub cells: Vec<PlayerForecastCell>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerForecastCell {
    pub player_name: String,
    pub position: String,
    pub nhl_team: String,
    pub nhl_team_name: String,
    pub opponent_abbrev: Option<String>,
    pub opponent_name: Option<String>,
    pub series_state: SeriesStateCode,
    pub series_label: String,
    pub wins: u32,
    pub opponent_wins: u32,
    /// Historical probability this team advances from the current state (0-1).
    pub odds_to_advance: f32,
    /// Max games remaining in the current series.
    pub games_remaining: u32,
    pub headshot_url: String,
}

// ---------------------------------------------------------------------------
// My Goalies Tonight
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MyGoalieSignal {
    pub player_name: String,
    pub nhl_team: String,
    pub nhl_team_logo: String,
    pub opponent_abbrev: String,
    pub opponent_logo: String,
    pub game_start_utc: Option<String>,
    pub venue: Option<String>,
    pub nhl_id: i64,
    pub headshot_url: String,
    /// "Probable", "Confirmed", "Backup", "Unknown" — derived from NHL
    /// `game-landing.probableGoalies` when available.
    pub start_status: GoalieStartStatus,
    pub playoff_record: Option<String>,
    pub playoff_gaa: Option<f64>,
    pub playoff_save_pctg: Option<f64>,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum GoalieStartStatus {
    Confirmed,
    Probable,
    Backup,
    Unknown,
}

// ---------------------------------------------------------------------------
// My Games Tonight
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MyGameTonight {
    pub game_id: u32,
    pub home_team: String,
    pub home_team_name: String,
    pub home_team_logo: String,
    pub away_team: String,
    pub away_team_name: String,
    pub away_team_logo: String,
    pub start_time_utc: String,
    pub venue: String,
    pub game_state: String,
    /// Current score once live/final.
    pub home_score: Option<i32>,
    pub away_score: Option<i32>,
    pub period: Option<String>,
    pub series_context: Option<String>,
    pub is_elimination: bool,
    /// Your rostered players in this game with live stats when available.
    pub my_players: Vec<MyPlayerInGame>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MyPlayerInGame {
    pub nhl_id: i64,
    pub name: String,
    pub position: String,
    pub nhl_team: String,
    pub headshot_url: String,
    pub goals: i32,
    pub assists: i32,
    pub points: i32,
}

// ---------------------------------------------------------------------------
// League Live Board
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeagueBoardEntry {
    pub rank: usize,
    pub team_id: i64,
    pub team_name: String,
    pub total_points: i32,
    pub points_today: i32,
    pub players_active_today: usize,
    /// Trailing-5-days daily points (may be shorter early in the playoffs).
    pub sparkline: Vec<i32>,
    pub is_my_team: bool,
}
