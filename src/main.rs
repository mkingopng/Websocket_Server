// src/main.rs
use std::sync::Arc;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio::time::{interval, Duration};
use backend_lib::{
    AppState, 
    config::Settings, 
    storage::FlatFileStorage,
    ws_router,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Initialize configuration
    // Try to load with explicit path if default doesn't work
    let config = Settings::load().or_else(|_| {
        println!("Trying to load config from ../../config/default.toml");
        Settings::load_from("../../config/default.toml")
    })?;

    // Create storage
    let storage = FlatFileStorage::new("data")?;

    // Create application state
    let state = Arc::new(AppState::new(storage, &config)?);

    // Setup a background task for session cleanup
    let state_clone = state.clone();
    tokio::spawn(async move {
        // Run cleanup every 15 minutes
        let mut interval = interval(Duration::from_secs(15 * 60));
        loop {
            interval.tick().await;
            println!("Running scheduled session cleanup");
            state_clone.sessions.cleanup_expired_sessions().await;
        }
    });
    
    // Setup a background task for auth rate limiter cleanup
    let auth_rate_limiter = state.auth_rate_limiter.clone();
    tokio::spawn(async move {
        // Run cleanup every hour
        let mut interval = interval(Duration::from_secs(60 * 60));
        loop {
            interval.tick().await;
            println!("Running scheduled auth rate limiter cleanup");
            auth_rate_limiter.cleanup();
        }
    });

    // Create the router using the optimized WebSocket router
    let app = ws_router::create_router(state);

    // Start the server
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = TcpListener::bind(&addr).await?;
    println!("listening on {addr}");

    axum::serve(listener, app).await?;

    Ok(())
} 