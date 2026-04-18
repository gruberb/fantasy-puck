// Re-export commonly used items.
//
// The crate is organized in three architectural layers per
// Bulletproof Rust Web's project-structure conventions:
//
// - `domain`  — pure business logic (no axum / sqlx / reqwest).
//               Exposes `ports::*` traits that `infra` implements.
// - `infra`   — adapters for Postgres (`infra::db`), the NHL API
//               (`infra::nhl`), Anthropic (`infra::prediction`),
//               and scheduled background jobs (`infra::jobs`).
// - `api`     — Axum handlers, DTOs, routes, extractors, middleware.
//
// `main.rs` is the composition root: it constructs the concrete
// adapters, wires them into `api::AppState` behind trait objects
// where applicable, and spawns the jobs.
pub use config::Config;
pub use error::{Error, Result};
pub use infra::db::FantasyDb;
pub use infra::nhl::client::NhlClient;

// Define modules
pub mod api;
pub mod auth;
pub mod config;
pub mod domain;
pub mod error;
pub mod infra;
pub mod tuning;
pub mod ws;
