//! Pure-domain modules. Nothing under this tree depends on `sqlx`,
//! `axum`, `reqwest`, or any other IO framework — the logic can be
//! lifted into a standalone crate or service without pulling those
//! transitive deps.
//!
//! Layout mirrors the Bulletproof Rust Web canonical structure:
//!
//! - [`models`] — entity types (`League`, `FantasyTeam`, `GameBoxscore`, …)
//! - [`ports`] — trait definitions for swappable external edges
//!   (NHL data source, prediction service, draft engine).
//! - [`services`] — stateless business logic that composes models.
//! - [`prediction`] — the race-odds / projection math. Left as its
//!   own top-level module for historical reasons; it is a service
//!   in all but name.

pub mod models;
pub mod ports;
pub mod prediction;
pub mod services;
