//! WebSocket handler for VIEW web server.
//!
//! On connect: sends an immediate full `WebSnapshot` as JSON, then pushes
//! a fresh snapshot every 500 ms until the client disconnects.
//!
//! Message format (text frames, JSON):
//! ```json
//! {
//!   "type": "snapshot",
//!   "agents": [...],
//!   "events": [...],
//!   "terminals": [...],
//!   "total_events_received": 42,
//!   "timestamp": "2026-04-23T15:00:00+07:00"
//! }
//! ```

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use serde_json::json;
use std::time::Duration;
use tokio::time;

use crate::SharedState;

/// Axum route handler — upgrades an HTTP request to a WebSocket connection.
pub async fn handler(
    ws: WebSocketUpgrade,
    State(state): State<SharedState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| drive_socket(socket, state))
}

/// Drives a single WebSocket connection: push snapshot every 500 ms.
async fn drive_socket(mut socket: WebSocket, state: SharedState) {
    // Send immediate snapshot on connect so the client doesn't wait.
    if send_snapshot(&mut socket, &state).await.is_err() {
        return;
    }

    let mut ticker = time::interval(Duration::from_millis(500));
    ticker.tick().await; // consume the first tick (already sent above)

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                if send_snapshot(&mut socket, &state).await.is_err() {
                    break;
                }
            }
            // Drain any incoming messages (pings, close frames).
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(data))) => {
                        // Echo pong — ignore errors on close.
                        let _ = socket.send(Message::Pong(data)).await;
                    }
                    _ => {} // ignore text/binary frames from client
                }
            }
        }
    }
}

/// Serialize `WebSnapshot` and send as a text frame.
/// Returns `Err(())` if the connection is broken.
async fn send_snapshot(socket: &mut WebSocket, state: &SharedState) -> Result<(), ()> {
    let snap = {
        let app = state.read();
        app.web_snapshot()
    };

    // Wrap with a "type" discriminant so clients can extend the protocol later.
    let payload = json!({
        "type": "snapshot",
        "agents": snap.agents,
        "events": snap.events,
        "terminals": snap.terminals,
        "total_events_received": snap.total_events_received,
        "timestamp": snap.timestamp,
    });

    let text = match serde_json::to_string(&payload) {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!("ws: failed to serialize snapshot: {e}");
            return Err(());
        }
    };

    socket
        .send(Message::Text(text.into()))
        .await
        .map_err(|_| ())
}
