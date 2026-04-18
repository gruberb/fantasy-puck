//! Pure prediction engine: bracket Monte Carlo, team-strength ratings,
//! per-player projection blend, and calibration helpers. No IO, no
//! framework dependencies.
//!
//! Intended extraction boundary — see `PREDICTION_SERVICE.md`. Callers
//! that need DB-backed wrappers go through `infra::prediction`.

pub mod backtest;
pub mod playoff_elo;
pub mod player_projection;
pub mod race_sim;
pub mod series_projection;
pub mod team_ratings;
