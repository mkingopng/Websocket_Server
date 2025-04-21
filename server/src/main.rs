// ============================
// server/src/main.rs
// ============================
//! Tokio / Axum entry‑point for the WebSocket server.

mod ws_router;
mod meet_actor;
mod auth;
mod storage;
mod error;

use axum::{routing::get, Router};
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use dashmap::DashMap;
use std::{sync::Arc};
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};

pub type MeetId = String;

/// Handle we keep for every live meet
pub type MeetMap = DashMap<MeetId, meet_actor::MeetHandle>;

pub use crate::error::AppError;
pub use crate::auth::SessionManager;
pub use crate::storage::FlatFileStorage;


#[derive(Clone)]
pub struct AppState {
    /// Map: live meet id → channels handle (cmd + relay)
    pub meets: Arc<MeetMap>,
    pub auth: Arc<SessionManager>,
    pub storage: Arc<FlatFileStorage>,
}

impl AppState {
    pub fn new() -> Self {
        AppState {
            meets: Arc::new(DashMap::new()),
            auth: Arc::new(SessionManager::new()),
            storage: Arc::new(FlatFileStorage::new("data").expect("Failed to initialize storage")),
        }
    }
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Create app state
    let app_state = AppState::new();

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Build our application with a route
    let app = Router::new()
        .route("/ws", get(ws_router))
        .layer(cors)
        .with_state(app_state);

    // Run it
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("listening on {}", addr);
    
    let listener = TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}
