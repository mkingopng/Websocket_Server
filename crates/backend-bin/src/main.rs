use axum::Router;
use backend_lib::{AppState, config::Settings, storage::FlatFileStorage};
use std::net::SocketAddr;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize configuration
    let config = Settings::load()?;

    // Create storage
    let storage = FlatFileStorage::new("data")?;

    // Create application state
    let state = AppState::new(storage, config)?;

    // Build our application with some routes
    let app = Router::new()
        .with_state(state);

    // Run it
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("listening on {}", addr);
    
    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service())
        .await?;

    Ok(())
} 