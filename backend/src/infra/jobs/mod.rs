//! Background jobs. Each submodule is spawned from `main.rs` or the
//! cron scheduler and has exclusive write access to some subset of
//! Postgres tables. Nothing in this module is called from the
//! request path.

pub mod historical_seed;
pub mod player_pool;
pub mod playoff_ingest;
pub mod scheduler;
