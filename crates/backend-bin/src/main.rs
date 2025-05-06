// ============================
// crates/backend-bin/src/main.rs
// ============================
//! Backend server for the application.
use backend_lib::{config::Settings, storage::FlatFileStorage, ws_router, AppState};
use std::fs;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::time::{interval, Duration};
use tracing::{debug, info};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing with better defaults
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(true)
        .init();

    info!("Starting OpenLifter WebSocket server...");

    // Initialize configuration
    let config = Settings::load()?;
    debug!("Configuration loaded: {:?}", config);

    // Ensure the data directory exists
    let data_dir = "data";
    fs::create_dir_all(data_dir).expect("Failed to create data directory");
    fs::create_dir_all(format!("{data_dir}/current-meets"))
        .expect("Failed to create current-meets directory");
    fs::create_dir_all(format!("{data_dir}/finished-meets"))
        .expect("Failed to create finished-meets directory");
    fs::create_dir_all(format!("{data_dir}/sessions"))
        .expect("Failed to create sessions directory");
    debug!("Data directories created");

    // Create storage
    let storage = FlatFileStorage::new(data_dir)?;
    info!("Storage initialized with path: {}", data_dir);

    // Create application state
    let state = Arc::new(AppState::new(storage, &config).await?);
    info!("Application state initialized");

    // Setup a background task for session cleanup
    let state_clone = state.clone();
    tokio::spawn(async move {
        // Run cleanup every 15 minutes
        let mut interval = interval(Duration::from_secs(15 * 60));
        loop {
            interval.tick().await;
            info!("Running scheduled session cleanup");
            state_clone.sessions.cleanup_expired_sessions().await;
        }
    });
    debug!("Session cleanup task scheduled");

    // Setup a background task for auth rate limiter cleanup
    let auth_rate_limiter = state.auth_rate_limiter.clone();
    tokio::spawn(async move {
        // Run cleanup every hour
        let mut interval = interval(Duration::from_secs(60 * 60));
        loop {
            interval.tick().await;
            info!("Running scheduled auth rate limiter cleanup");
            auth_rate_limiter.cleanup();
        }
    });
    debug!("Auth rate limiter cleanup task scheduled");

    // Create the router using the optimized WebSocket router
    let app = ws_router::create_router(state);
    info!("Router created");

    // Start the server
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = TcpListener::bind(&addr).await?;
    info!("Server listening on {addr}");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}
