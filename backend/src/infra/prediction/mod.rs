//! Prediction adapters.
//!
//! - [`elo`] — Postgres-backed inputs for the Elo-driven playoff
//!   projection model. Pure data adapter: pulls features out of the
//!   playoff-history tables and hands them to the pure-domain
//!   simulator in `crate::domain::prediction`.
//! - [`claude`] — Anthropic `/v1/messages` adapter. Implements
//!   [`crate::domain::ports::prediction::PredictionService`] so
//!   handlers never see a raw HTTP client. A future gRPC-backed
//!   replacement can implement the same trait.
//!
//! The Elo adapter and the Claude adapter both live under this
//! module because they are the two "prediction" edges of the
//! system — different protocols, same architectural role.

pub mod claude;
pub mod elo;

// Keep the pre-Phase-5.5 call sites working (`use crate::infra::
// prediction::compute_current_elo;`) by re-exporting the Elo
// adapter's public surface at the module root.
pub use elo::*;
