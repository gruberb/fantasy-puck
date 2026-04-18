//! Pure-domain modules. Nothing under this tree depends on `sqlx`,
//! `axum`, `reqwest`, or any other IO framework — the logic can be
//! lifted into a standalone crate or service without pulling those
//! transitive deps.
//!
//! Per the plan in `PREDICTION_SERVICE.md`, the `prediction` submodule
//! is the first candidate for extraction.

pub mod prediction;
