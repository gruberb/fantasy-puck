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

        let (tx, rx) = broadcast::channel(64);
        channels.insert(session_id.to_string(), tx);
        rx
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
