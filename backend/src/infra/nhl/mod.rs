//! Outbound adapter for the undocumented NHL API at `api-web.nhle.com`.
//!
//! - [`client`] — the `NhlClient` that performs HTTP calls, caches
//!   responses, and enforces rate-limit backoff. It is the concrete
//!   implementation of the [`crate::domain::ports::nhl_source::NhlDataSource`]
//!   port.
//! - [`constants`] — compile-time NHL team metadata (abbreviations, names).
//! - [`urls`] — pure URL constructors for headshots, logos, and the
//!   public team-name lookup that the frontend links to.

pub mod client;
pub mod constants;
pub mod urls;
