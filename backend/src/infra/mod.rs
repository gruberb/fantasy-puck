//! Infrastructure adapters. Everything in this layer implements a
//! trait from `crate::domain::ports`, wraps an external SDK, or owns
//! a Postgres connection pool.
//!
//! Handlers depend on `infra` only via the composition root in
//! `main.rs` and the traits published in `domain::ports`. `domain`
//! itself never depends on anything under `infra`.

pub mod calibrate;
pub mod db;
pub mod jobs;
pub mod nhl;
pub mod prediction;
