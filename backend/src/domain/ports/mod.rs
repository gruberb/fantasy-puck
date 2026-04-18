//! Ports — trait definitions for the swappable edges of the system.
//!
//! Each port has exactly one production adapter today, but they are
//! defined as traits so that
//!
//! - an alternate NHL data source (e.g. SportRadar) can replace
//!   [`nhl_source::NhlDataSource`] without touching handlers;
//! - [`prediction::PredictionService`] can swap from in-process
//!   Anthropic calls to a gRPC call to a separately-deployed model
//!   server without touching handlers;
//! - [`draft_engine::DraftEngine`] can swap from the in-process
//!   WebSocket implementation to an external draft orchestrator,
//!   with the only observable effect being the final set of
//!   picks written to the database.
//!
//! Production wiring happens in `main.rs`. Handlers see only
//! `Arc<dyn NhlDataSource>`, `Arc<dyn PredictionService>`, etc.,
//! via `api::AppState`.

pub mod draft_engine;
pub mod nhl_source;
pub mod prediction;
