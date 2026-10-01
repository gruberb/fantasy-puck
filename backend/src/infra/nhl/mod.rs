//! Outbound adapter for the undocumented NHL API at `api-web.nhle.com`.
//!
//! - [`client`]: the `NhlClient` that performs HTTP calls, caches
//!   responses, and enforces rate-limit backoff.
//! - [`constants`]: NHL endpoint builders and team metadata.

pub mod client;
pub mod constants;
