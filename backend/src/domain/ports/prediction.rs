//! Port: text-generation and projection services used by Insights,
//! Pulse, and Race-Odds.
//!
//! The production adapter ([`crate::infra::prediction`]) wraps
//! Anthropic's `/v1/messages` endpoint. Phase 5.5 of the
//! data-pipeline redesign moves callers to a `PredictionService`
//! trait so the narrative generator can be swapped to a gRPC call
//! without touching handlers.
//!
//! Signatures are placeholders until that phase — the trait exists
//! today so `main.rs` can start constructing the production adapter
//! behind `Arc<dyn PredictionService>`.

#[doc(hidden)]
pub trait PredictionService: Send + Sync {}
