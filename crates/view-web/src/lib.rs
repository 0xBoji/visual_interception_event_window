//! `view-web` — LAN remote access server for VIEW.
//!
//! Exposes a lightweight Axum HTTP + WebSocket server so any device on
//! the local network can observe agent mesh state in real-time.
//!
//! ## Endpoints
//! | Method | Path | Description |
//! |--------|------|-------------|
//! | GET | `/` | Server health / info (JSON) |
//! | GET | `/api/agents` | Snapshot of all agents (JSON) |
//! | GET | `/api/events` | Last 20 events (JSON) |
//! | GET | `/api/snapshot` | Full `WebSnapshot` (JSON) |
//! | GET | `/ws` | WebSocket — pushes `WebSnapshot` every 500 ms |
//!
//! ## Usage (from view-desktop runtime)
//! ```no_run
//! use std::sync::Arc;
//! use tokio::sync::Mutex;
//! use view_core::app::AppState;
//!
//! # async fn example() {
//! let state = Arc::new(Mutex::new(AppState::new()));
//! view_web::start(state, 23779).await.unwrap();
//! # }
//! ```

pub mod api;
pub mod ws;

use anyhow::Result;
use axum::{routing::get, Router};
use std::sync::Arc;
use parking_lot::RwLock;
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};
use view_core::app::AppState;

/// Shared state type threaded through Axum handlers.
pub type SharedState = Arc<RwLock<AppState>>;

/// Start the VIEW web server, binding to `0.0.0.0:{port}`.
///
/// Spawning this via `tokio::spawn` is recommended so it runs alongside
/// the desktop UI loop without blocking.
///
/// # Errors
/// Returns an error if the TCP listener cannot bind (port already in use, etc.)
pub async fn start(state: SharedState, port: u16) -> Result<()> {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/", get(api::root_info))
        .route("/api/agents", get(api::agents))
        .route("/api/events", get(api::events))
        .route("/api/snapshot", get(api::snapshot))
        .route("/ws", get(ws::handler))
        .with_state(state)
        .layer(cors);

    let addr = format!("0.0.0.0:{port}");
    tracing::info!("VIEW web server listening on http://{addr}");
    tracing::info!("WebSocket endpoint: ws://{addr}/ws");

    let listener = TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
