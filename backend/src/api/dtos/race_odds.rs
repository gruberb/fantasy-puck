//! Response shape for the `/api/race-odds` endpoint.
//!
//! Wraps the pure-domain [`TeamOdds`] / [`PlayerOdds`] types from
//! [`crate::domain::prediction::race_sim`] with a top-level envelope
//! that carries the generation timestamp, the mode (league race vs.
//! global Fantasy Champion), and the simulation knobs used so
//! consumers can display methodology.

use serde::{Deserialize, Serialize};

use crate::domain::prediction::race_sim::{NhlTeamOdds, PlayerOdds, TeamOdds};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RaceOddsResponse {
    pub generated_at: String,
    pub mode: RaceOddsMode,
    pub trials: usize,
    pub k_factor: f32,
    /// Fantasy-team race odds. Populated in [`RaceOddsMode::League`];
    /// empty in Champion mode.
    pub team_odds: Vec<TeamOdds>,
    /// Top skater leaderboard by projected final playoff points. Populated
    /// in [`RaceOddsMode::Champion`]; empty in League mode.
    pub champion_leaderboard: Vec<PlayerOdds>,
    /// Per-NHL-team playoff projections (cup odds + expected games). The
    /// Insights page's Stanley Cup view and Pulse's MyStakes section both
    /// read from this array.
    #[serde(default)]
    pub nhl_teams: Vec<NhlTeamOdds>,
    /// Optional head-to-head framing against the caller's closest rival by
    /// projected finish. Populated only in League mode when `my_team_id` is
    /// resolvable and at least two teams exist.
    #[serde(default)]
    pub rivalry: Option<RivalryCard>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RaceOddsMode {
    /// League-scoped: compute race odds across the league's fantasy teams.
    League,
    /// Global: compute Fantasy Champion leaderboard across top NHL skaters.
    Champion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RivalryCard {
    pub my_team_name: String,
    pub rival_team_name: String,
    pub my_win_prob: f32,
    pub rival_win_prob: f32,
    /// `P(my_final > rival_final)` from the same simulation sweep.
    pub my_head_to_head_prob: f32,
    pub my_projected_mean: f32,
    pub rival_projected_mean: f32,
}
