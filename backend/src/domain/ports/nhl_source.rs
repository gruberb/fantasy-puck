//! Port: a read-only view into NHL hockey data.
//!
//! The production adapter ([`crate::infra::nhl::client::NhlClient`])
//! calls the undocumented NHL API at `api-web.nhle.com`. Any
//! alternative — a SportRadar adapter, a static-file replay adapter
//! for tests, a database-mirror adapter — can implement this trait
//! and be swapped in from the composition root in `main.rs`.
//!
//! The trait is defined but not yet consumed by handlers. Phase 1 of
//! the data-pipeline redesign (see `docs/DATA-PIPELINE-REDESIGN.md`)
//! moves the trait consumers from direct `NhlClient` calls to
//! `Arc<dyn NhlDataSource>`. Until then this module is a
//! placeholder that pins the contract.

// The concrete method surface mirrors what the pollers need:
// schedule, boxscore, landing, skater/goalie leaderboards, team
// roster, standings, playoff carousel, player game log. It is
// intentionally narrower than the full `NhlClient` surface — the
// goal is to expose the minimal read API the rest of the code
// needs, not to mirror every NHL endpoint.
//
// Signatures are intentionally not committed yet. They will be
// filled in Phase 1 when the poller uses them for real, so that
// domain types (not the current ad-hoc serde::Value blobs) are
// the return shapes and we don't lock in the wrong contract.

#[doc(hidden)]
pub trait NhlDataSource: Send + Sync {}
