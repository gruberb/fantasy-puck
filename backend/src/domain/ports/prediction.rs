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

use crate::api::dtos::teams::TeamPointsResponse;

#[async_trait::async_trait]
pub trait PredictionService: Send + Sync {
    /// Produce the markdown narrative for the Pulse "Your Read" block
    /// and the fantasy-team detail page. The caller populates every
    /// field on `team` except the narrative string itself;
    /// implementations read league rank, concentration, and per-player
    /// bucket/grade/recent-games and emit four `### Heading` sections
    /// (Yesterday / Where You Stand / Player-by-Player / What to Expect). Cached
    /// by the handler; returning `None` lets the UI fall back to a
    /// static summary without failing the request.
    async fn team_diagnosis(&self, team: &TeamPointsResponse) -> Option<String>;
}
