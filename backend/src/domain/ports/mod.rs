//! Ports: trait definitions for the swappable edges of the system.
//!
//! [`prediction::PredictionService`] can move from in-process OpenRouter
//! calls to a separately deployed model server without touching
//! handlers. Production wiring happens in `main.rs`; handlers see
//! `Arc<dyn PredictionService>` via `api::AppState`.

pub mod prediction;
