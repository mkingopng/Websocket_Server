// ============================
// openlifter-backend/src/bin/server.rs
// ============================
//! Tokio / Axum entry‑point for the WebSocket server.

use axum::{Router};
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};

use openlifter_backend::{AppState, ws_router};

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

    // Build our application with a route
    let app = Router::new()
        .merge(ws_router::router(app_state.clone()))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        );

    // Run it with hyper on localhost:3000
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = TcpListener::bind(addr).await.unwrap();
    tracing::info!("listening on {}", addr);
    
    axum::serve(listener, app.into_service()).await.unwrap();
} 
