use std::sync::Arc;

use axum::{
    extract::{
        ws::{Message, WebSocket},
        Path, State, WebSocketUpgrade,
    },
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use tracing::{info, warn};

use crate::api::routes::AppState;

/// WebSocket upgrade handler for draft sessions.
pub async fn ws_draft(
    ws: WebSocketUpgrade,
    Path(session_id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_draft_ws(socket, session_id, state))
}

/// Manages a single WebSocket connection for a draft session.
///
/// - Subscribes to the DraftHub broadcast channel for the session.
/// - Forwards broadcast messages to the WebSocket client.
/// - Handles incoming ping/pong; ignores other client messages.
async fn handle_draft_ws(socket: WebSocket, session_id: String, state: Arc<AppState>) {
    info!("WebSocket connected for draft session {}", session_id);

    let mut rx = state.draft_hub.subscribe(&session_id).await;
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let mut ping_interval = tokio::time::interval(crate::tuning::http::WS_PING_INTERVAL);

    loop {
        tokio::select! {
            // Server-side ping to keep connection alive through proxies
            _ = ping_interval.tick() => {
                if ws_sender.send(Message::Ping(vec![].into())).await.is_err() {
                    break;
                }
            }

            // Forward broadcast events to the WebSocket client
            msg = rx.recv() => {
                match msg {
                    Ok(text) => {
                        if ws_sender.send(Message::Text(text.into())).await.is_err() {
                            // Client disconnected
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        warn!("WebSocket client lagged by {} messages for session {}", n, session_id);
                        // Continue; the receiver auto-skips missed messages
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        // Channel was closed (session ended)
                        break;
                    }
                }
            }

            // Handle incoming WebSocket messages from the client
            msg = ws_receiver.next() => {
                match msg {
                    Some(Ok(Message::Ping(data))) => {
                        if ws_sender.send(Message::Pong(data)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Pong(_))) => {
                        // No-op
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        // Client disconnected
                        break;
                    }
                    Some(Ok(_)) => {
                        // Ignore text/binary messages from client
                    }
                    Some(Err(e)) => {
                        warn!("WebSocket error for session {}: {}", session_id, e);
                        break;
                    }
                }
            }
        }
    }

    info!("WebSocket disconnected for draft session {}", session_id);
}
