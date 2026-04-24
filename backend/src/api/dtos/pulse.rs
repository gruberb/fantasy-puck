use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::api::dtos::teams::{PlayerStatsResponse, TeamDiagnosis};
use crate::domain::prediction::series_projection::SeriesStateCode;

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
    pub my_games_tonight: Vec<MyGameTonight>,
    pub league_board: Vec<LeagueBoardEntry>,
    pub has_games_today: bool,
    pub has_live_games: bool,
    /// Every NHL matchup on today's slate (home / away abbrev pairs).
    /// Used by the Live Rankings section on the dashboard to render a
    /// per-fantasy-team "games you have a stake in" cell. Empty on
    /// off-days.
    #[serde(default)]
    pub games_today: Vec<GameMatchup>,
    /// Cup-win probability per NHL team from the last Monte Carlo run
    /// (values in 0.0 – 1.0). Keyed by 3-letter abbreviation. Populated
    /// opportunistically from the cached `/api/race-odds` payload; empty
    /// when that cache hasn't warmed yet. The narrator uses this to
    /// contrast "high-diversity, low-ceiling" rosters against
    /// concentrated stacks that depend on one run going deep.
    #[serde(default)]
    pub nhl_team_cup_odds: HashMap<String, f32>,
    /// Full per-player breakdown + descriptive diagnosis for the
    /// caller's team — concentration chips, grades, remaining-points
    /// projections, yesterday's mirror-backed recap, and the Claude
    /// `### Yesterday / ### Where You Stand / ### Player-by-Player /
    /// ### What to Expect` narrative. Populated
    /// only when `my_team` is resolved and we're in playoff mode.
    /// The Pulse page's "Your Read" block renders from here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub my_team_diagnosis: Option<MyTeamDiagnosis>,
    /// League-level overview: leader, points distribution, the three
    /// teams with the highest projected-final points. Drives the
    /// Pulse page's "Your League" block. Empty when the race-odds
    /// cache is cold.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub league_outlook: Option<LeagueOutlook>,
}

/// Full team breakdown payload shared between the Pulse page's Your
/// Read block and (previously) the Fantasy Team Detail page. Shape
/// mirrors the "diagnosis + roster" pair returned by the shared
/// composition helper so the frontend can render a single component.
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MyTeamDiagnosis {
    pub team_id: i64,
    pub team_name: String,
    pub total_points: i32,
    pub diagnosis: TeamDiagnosis,
    pub players: Vec<PlayerStatsResponse>,
}

/// League-level outlook for the Pulse "Your League" block.
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LeagueOutlook {
    /// Total team count in the league. Used by the UI to label
    /// the points-distribution strip ("15 teams, median X pts").
    pub total_teams: i32,
    pub leader_team_id: i64,
    pub leader_name: String,
    pub leader_points: i32,
    /// Current points across every team, sorted desc. Enables the
    /// horizontal bar strip that visualises the spread.
    pub points_distribution: Vec<i32>,
    pub median_points: f32,
    /// Top-3 projected finishers from the cached Monte Carlo, sorted
    /// by projected mean final points desc.
    pub top_three: Vec<LeagueOutlookEntry>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LeagueOutlookEntry {
    pub team_id: i64,
    pub team_name: String,
    pub current_points: i32,
    pub projected_final_mean: f32,
    pub win_prob: f32,
    pub top3_prob: f32,
    /// The team's largest NHL-team stack and what it's produced so
    /// far. Lets the UI write a one-line "why" such as
    /// "5 Wild · 16 pts so far, Minnesota up 1-0". `None` when we
    /// can't resolve the team's roster.
    pub top_stack: Option<LeagueOutlookStack>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LeagueOutlookStack {
    pub nhl_team: String,
    pub rostered: i32,
    /// Cup-win probability for this NHL team, lifted from the same
    /// cached race-odds payload. Zero when not available.
    pub cup_win_prob: f32,
}


/// A single matchup on today's slate, surfaced at the top level of
/// `PulseResponse` so the dashboard's Live Rankings section can
/// cross-reference with each fantasy team's rostered NHL teams
/// without a second fetch.
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GameMatchup {
    pub home_team: String,
    pub away_team: String,
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
    /// Excludes tied series — those roll up to [`players_tied`].
    pub players_trailing: usize,
    /// Sum: players on teams whose series is currently tied. Kept separate
    /// from `players_trailing` because a 0-0 / 1-1 / 2-2 tie isn't losing —
    /// counting it as "trailing" read as a bug to users.
    #[serde(default)]
    pub players_tied: usize,
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
    /// NHL player id — required for linking to the public player profile
    /// page (`nhl.com/player/{id}`).
    pub nhl_id: i64,
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
