//! Port: narrative generation for Pulse, Insights, and Race-Odds.
//!
//! Handlers never call Anthropic (or whatever model host) directly.
//! They see this trait through `Arc<dyn PredictionService>` on
//! `AppState`. Today the only production adapter is
//! [`crate::infra::prediction::claude::ClaudeNarrator`], which wraps
//! the Anthropic `/v1/messages` endpoint. A gRPC-backed adapter
//! pointing at an out-of-process model server would implement the
//! same trait and slot into the composition root in `main.rs` with
//! no handler changes.
//!
//! Method shape: each narrative has its own typed input (the
//! response payload it is writing *about*) and returns an
//! `Option<String>`. Returning `None` is the allowed failure mode —
//! handlers degrade by omitting the narrative block; no response
//! fails outright just because the model call did.

use crate::api::dtos::pulse::PulseResponse;

#[async_trait::async_trait]
pub trait PredictionService: Send + Sync {
    /// Write a ~4-7 sentence personal dispatch for the Pulse page.
    /// Called once per (league, team, hockey-day) and cached until
    /// the next game-end transition of a rostered game.
    async fn pulse_narrative(&self, pulse: &PulseResponse) -> Option<String>;
}
