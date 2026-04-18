//! Port: the component that orchestrates a fantasy draft from open
//! to closed and writes the final picks into the database.
//!
//! The production adapter is the in-process WebSocket-driven draft
//! under `crate::ws`. The whole implementation could be lifted into
//! a separate service and the only observable side effect at the
//! core is the set of `draft_picks` rows written for a league — the
//! rest of the app reads those rows like any other fantasy league
//! data.

#[doc(hidden)]
pub trait DraftEngine: Send + Sync {}
