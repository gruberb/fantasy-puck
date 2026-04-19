use std::collections::HashMap;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Player data from NHL API
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Player {
    pub id: u32,
    pub first_name: HashMap<String, String>,
    pub last_name: HashMap<String, String>,
    #[serde(default)]
    pub sweater_number: Option<u32>,
    pub team_abbrev: String,
    pub position: String,
    pub value: f64,
}

/// Stats categories from NHL API response
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StatsLeaders {
    pub goals_sh: Vec<Player>,
    pub plus_minus: Vec<Player>,
    pub assists: Vec<Player>,
    pub goals_pp: Vec<Player>,
    pub faceoff_leaders: Vec<Player>,
    pub penalty_mins: Vec<Player>,
    pub goals: Vec<Player>,
    pub points: Vec<Player>,
    pub toi: Vec<Player>,
}

/// Goalie stats categories from NHL API response.
///
/// The `/v1/goalie-stats-leaders/{season}/{game_type}` payload mirrors
/// the skater leaderboard: each category is its own `Vec<Player>` where
/// `Player.value` carries the metric for that category.
///
/// Every field is `#[serde(default)]` so a partial payload (some
/// categories missing) still deserialises — the NHL endpoint
/// occasionally drops categories early in a season before enough games
/// have been played to rank every one.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GoalieStatsLeaders {
    #[serde(default)]
    pub wins: Vec<Player>,
    #[serde(default)]
    pub save_pctg: Vec<Player>,
    #[serde(default)]
    pub goals_against_average: Vec<Player>,
    #[serde(default)]
    pub shutouts: Vec<Player>,
    #[serde(default)]
    pub save_pctg_5v5: Vec<Player>,
}

/// Venue information for games
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GameVenue {
    pub default: String,
}

// Define a struct to hold the combined game data
#[derive(Debug)]
pub struct GameData {
    pub game_state: GameState,
    pub home_score: Option<i32>,
    pub away_score: Option<i32>,
    /// Period number (1–3 reg, 4+ playoff OT). None outside a playing state.
    pub period_number: Option<i16>,
    /// Upstream `periodType` — `REG`, `OT`, `SO`. Stored raw; the API
    /// handler maps it to a human label at render time.
    pub period_type: Option<String>,
}

/// Team information for schedule
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TeamInfo {
    pub id: u32,
    pub abbrev: String,
    pub common_name: Option<HashMap<String, String>>,
    pub place_name: Option<HashMap<String, String>>,
}

/// Today's schedule from NHL API
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TodaySchedule {
    #[serde(rename = "gameWeek")]
    pub game_week: Vec<GameDay>,
}

impl TodaySchedule {
    pub fn games_for_date(&self, date: &str) -> Vec<TodayGame> {
        self.game_week
            .iter()
            .find_map(|day| (day.date == date).then_some(day.games.clone()))
            .unwrap_or_default()
    }
}

/// Game day within the schedule
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameDay {
    pub date: String,
    pub games: Vec<TodayGame>,
}

/// Game score information
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GameScore {
    pub away: i32,
    pub home: i32,
}

/// Period descriptor for games
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PeriodDescriptor {
    #[serde(default)]
    pub number: Option<i32>,
    #[serde(default)]
    pub period_type: Option<String>,
    #[serde(default)]
    pub ot_periods: Option<i32>,
    #[serde(default)]
    pub max_regulation_periods: Option<i32>,
}

// Modify the existing TodayGame struct to include the new fields
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TodayGame {
    pub id: u32,
    pub season: u32,
    pub game_type: u8,
    #[serde(rename = "startTimeUTC")]
    pub start_time_utc: String,
    pub venue: GameVenue,
    pub away_team: TeamInfo,
    pub home_team: TeamInfo,
    pub game_state: GameState,
    #[serde(rename = "easternUTCOffset")]
    pub eastern_utc_offset: Option<String>, // Time offset for Eastern time
    #[serde(default)]
    pub game_score: Option<GameScore>,
    #[serde(default)]
    pub period_descriptor: Option<PeriodDescriptor>, // Add period information
    #[serde(default)]
    pub series_status: Option<SeriesStatus>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SeriesStatus {
    pub round: u32,
    pub series_title: String,
    pub top_seed_team_abbrev: String,
    pub top_seed_wins: u32,
    pub bottom_seed_team_abbrev: String,
    pub bottom_seed_wins: u32,
    pub game_number_of_series: u32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GameBoxscore {
    pub player_by_game_stats: PlayerByGameStats,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlayerByGameStats {
    pub away_team: TeamGameStats,
    pub home_team: TeamGameStats,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TeamGameStats {
    pub forwards: Vec<BoxscorePlayer>,
    pub defense: Vec<BoxscorePlayer>,
    pub goalies: Vec<BoxscorePlayer>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BoxscoreTeam {
    pub id: Option<u32>,        // Add this field to help identify teams
    pub abbrev: Option<String>, // Add this field to help identify teams
    #[serde(rename = "teamStats")]
    pub team_stats: BoxTeamStats,
    pub forwards: Vec<BoxscorePlayer>,
    pub defense: Vec<BoxscorePlayer>,
    pub goalies: Vec<BoxscorePlayer>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BoxTeamStats {
    pub goals: Option<i32>,
    pub assists: Option<i32>,
    pub points: Option<i32>,
    pub pim: Option<i32>,
    pub shots: Option<i32>,
    pub power_play_goals: Option<i32>,
    pub power_play_opportunities: Option<i32>,
    pub face_off_win_percentage: Option<f32>,
    pub blocked: Option<i32>,
    pub takeaways: Option<i32>,
    pub giveaways: Option<i32>,
    pub hits: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BoxscorePlayer {
    pub player_id: i32,
    pub sweater_number: i32,
    pub name: HashMap<String, String>,
    pub position: String,
    pub goals: Option<i32>,
    pub assists: Option<i32>,
    pub points: Option<i32>,
    pub plus_minus: Option<i32>,
    pub pim: Option<i32>,
    pub hits: Option<i32>,
    pub power_play_goals: Option<i32>,
    pub sog: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SkaterStats {
    pub goals: i32,
    pub assists: i32,
    pub plus_minus: i32,
    pub shots: i32,
    pub hits: i32,
    pub blocked_shots: i32,
    pub pim: i32,
    pub faceoffs: Option<Faceoffs>,
    pub power_play_goals: i32,
    pub power_play_assists: i32,
    pub short_handed_goals: i32,
    pub short_handed_assists: i32,
    pub time_on_ice: String,
    pub even_time_on_ice: String,
    pub power_play_time_on_ice: String,
    pub short_handed_time_on_ice: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Faceoffs {
    pub faceoff_wins: i32,
    pub faceoff_taken: i32,
    pub faceoff_pct: f32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GoalieStats {
    pub even_shots_against: i32,
    pub even_saves: i32,
    pub power_play_shots_against: i32,
    pub power_play_saves: i32,
    pub short_handed_shots_against: i32,
    pub short_handed_saves: i32,
    pub saves: i32,
    pub shots_against: i32,
    pub save_pct: f32,
    pub even_strength_save_pct: f32,
    pub power_play_save_pct: f32,
    pub short_handed_save_pct: f32,
    pub time_on_ice: String,
    pub goals: i32,
    pub assists: i32,
    pub pim: i32,
}

/// A single game entry under the
/// `/schedule/playoff-series/{season}/{letter}` endpoint. The response
/// reliably contains `id`, `startTimeUTC`, scored `homeTeam`/`awayTeam`,
/// and `gameState` for every game in the series — unlike the
/// `/schedule/{date}` path, which intermittently drops late-round games
/// when queried retroactively.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayoffSeriesGames {
    pub round: Option<i64>,
    pub season: Option<i64>,
    pub series_letter: Option<String>,
    #[serde(default)]
    pub games: Vec<PlayoffSeriesGame>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayoffSeriesGame {
    pub id: u32,
    #[serde(default)]
    pub game_type: i64,
    #[serde(rename = "startTimeUTC", default)]
    pub start_time_utc: Option<String>,
    #[serde(default)]
    pub game_state: GameState,
    pub home_team: PlayoffSeriesTeam,
    pub away_team: PlayoffSeriesTeam,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayoffSeriesTeam {
    #[serde(default)]
    pub id: i64,
    pub abbrev: String,
    #[serde(default)]
    pub score: Option<i32>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayoffCarousel {
    pub season_id: i64,
    pub current_round: i64,
    pub rounds: Vec<Round>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Round {
    pub round_number: i64,
    pub round_label: String,
    pub round_abbrev: String,
    pub series: Vec<Series>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Series {
    pub series_letter: String,
    pub round_number: i64,
    pub series_label: String,
    pub series_link: String,
    pub bottom_seed: BottomSeed,
    pub top_seed: TopSeed,
    pub needed_to_win: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BottomSeed {
    pub id: i64,
    pub abbrev: String,
    pub wins: i64,
    pub logo: String,
    pub dark_logo: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopSeed {
    pub id: i64,
    pub abbrev: String,
    pub wins: i64,
    pub logo: String,
    pub dark_logo: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlayerGameLog {
    pub season_id: u32,
    pub game_type_id: u8,
    pub player_stats_seasons: Vec<PlayerStatsSeason>,
    pub game_log: Vec<GameLogEntry>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlayerStatsSeason {
    pub season: u32,
    pub game_types: Vec<u8>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GameLogEntry {
    pub game_id: u32,
    pub team_abbrev: String,
    pub home_road_flag: String,
    pub game_date: String,
    pub goals: i32,
    pub assists: i32,
    pub common_name: CommonName,
    pub opponent_common_name: CommonName,
    pub points: i32,
    pub plus_minus: i32,
    pub power_play_goals: i32,
    pub power_play_points: i32,
    pub game_winning_goals: i32,
    pub ot_goals: i32,
    pub shots: i32,
    pub shifts: i32,
    pub shorthanded_goals: i32,
    pub shorthanded_points: i32,
    pub opponent_abbrev: String,
    pub pim: i32,
    pub toi: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CommonName {
    pub default: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum GameState {
    Live,
    Final,
    Off,
    Crit,
    Preview,
    /// NHL pre-game warm-up — returned as `"PRE"` by the schedule
    /// endpoint, which wasn't covered by `Preview`. Without the
    /// explicit rename, `"PRE"` fell through to [`GameState::Unknown`]
    /// and the live poller (which filters on `LIVE/CRIT/PRE`) never
    /// selected those games even once they were seconds from puck
    /// drop.
    #[serde(rename = "PRE")]
    Pre,
    Fut,
    #[default]
    #[serde(other)]
    Unknown,
}

impl GameState {
    pub fn is_completed(&self) -> bool {
        matches!(self, GameState::Final | GameState::Off)
    }

    pub fn is_live(&self) -> bool {
        matches!(self, GameState::Live | GameState::Crit)
    }

    pub fn is_upcoming(&self) -> bool {
        matches!(self, GameState::Preview | GameState::Pre | GameState::Fut)
    }
}

impl FromStr for GameState {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "LIVE" => Ok(GameState::Live),
            "FINAL" => Ok(GameState::Final),
            "OFF" => Ok(GameState::Off),
            "CRIT" => Ok(GameState::Crit),
            "FUT" => Ok(GameState::Fut),
            "PREVIEW" => Ok(GameState::Preview),
            "PRE" => Ok(GameState::Pre),
            _ => Ok(GameState::Unknown),
        }
    }
}

// Player matching and stat calculation utilities are in utils/nhl.rs
