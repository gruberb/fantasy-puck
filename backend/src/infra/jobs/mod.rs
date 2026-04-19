//! Background jobs. Each submodule is spawned from `main.rs` or the
//! cron scheduler and has exclusive write access to some subset of
//! Postgres tables. Nothing in this module is called from the
//! request path.

pub mod edge_refresher;
pub mod historical_seed;
pub mod live_poller;
pub mod meta_poller;
pub mod player_pool;
pub mod playoff_ingest;
pub mod rehydrate;
pub mod scheduler;
