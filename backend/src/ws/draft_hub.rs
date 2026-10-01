use std::collections::HashMap;

use serde::Serialize;
use tokio::sync::{broadcast, RwLock};
use tracing::warn;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum DraftEvent {
    SessionUpdated {
        session_id: String,
        status: String,
        current_round: i32,
        current_pick_index: i32,
        sleeper_status: Option<String>,
        sleeper_pick_index: i32,
    },
    PickMade {
        pick: serde_json::Value,
    },
    SleeperUpdated,
    PlayerPoolUpdated,
}

pub struct DraftHub {
    channels: RwLock<HashMap<String, broadcast::Sender<String>>>,
}

impl Default for DraftHub {
    fn default() -> Self {
        Self::new()
    }
}

impl DraftHub {
    pub fn new() -> Self {
        Self {
            channels: RwLock::new(HashMap::new()),
        }
    }

    /// Subscribe to draft events for a given session.
    /// Creates the channel if it doesn't exist yet.
    pub async fn subscribe(&self, session_id: &str) -> broadcast::Receiver<String> {
        // Try read lock first to avoid write contention
        {
            let channels = self.channels.read().await;
            if let Some(tx) = channels.get(session_id) {
                return tx.subscribe();
            }
        }

        // Channel doesn't exist, create it
        let mut channels = self.channels.write().await;
        // Double-check after acquiring write lock
        if let Some(tx) = channels.get(session_id) {
            return tx.subscribe();
        }

        let (tx, rx) = broadcast::channel(crate::tuning::http::WS_BROADCAST_CAPACITY);
        channels.insert(session_id.to_string(), tx);
        rx
    }

    /// Drop the session's channel once its last subscriber has gone, so
    /// the map only holds sessions with live connections. Callers drop
    /// their receiver first. A concurrent `subscribe` either ran before
    /// (the count is non-zero) or runs after and recreates the channel.
    pub async fn release(&self, session_id: &str) {
        let mut channels = self.channels.write().await;
        if channels
            .get(session_id)
            .is_some_and(|tx| tx.receiver_count() == 0)
        {
            channels.remove(session_id);
        }
    }

    /// Broadcast a draft event to all subscribers of a session.
    pub async fn broadcast(&self, session_id: &str, event: DraftEvent) {
        let channels = self.channels.read().await;
        if let Some(tx) = channels.get(session_id) {
            let msg = match serde_json::to_string(&event) {
                Ok(json) => json,
                Err(e) => {
                    warn!("Failed to serialize draft event: {}", e);
                    return;
                }
            };
            // Ignore send errors (no active receivers)
            let _ = tx.send(msg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn release_drops_channel_only_after_last_subscriber() {
        let hub = DraftHub::new();
        let a = hub.subscribe("s").await;
        let b = hub.subscribe("s").await;
        drop(a);
        hub.release("s").await;
        assert!(hub.channels.read().await.contains_key("s"));
        drop(b);
        hub.release("s").await;
        assert!(!hub.channels.read().await.contains_key("s"));
    }
}
