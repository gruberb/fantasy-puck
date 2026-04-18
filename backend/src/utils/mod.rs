//! Legacy utility grab-bag. The pure math modules that used to live
//! here now live in `domain::prediction`; the DB-backed wrappers moved
//! to `infra::prediction`. What's left is request-path helpers and
//! still-mixed code that will migrate next iteration.

pub mod api;
pub mod fantasy;
pub mod historical_seed;
pub mod nhl;
pub mod player_pool;
pub mod playoff_ingest;
pub mod scheduler;
