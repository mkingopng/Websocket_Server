use std::sync::Arc;
use std::net::SocketAddr;
use tokio::net::TcpListener;
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
        println!("Trying to load config from alternate locations");
        Settings::load_from("config/default.toml").or_else(|_| {
            println!("Trying root config/default.toml");
            Settings::load_from("./config/default.toml")
        })
    })?;

    // Create storage
    let storage = FlatFileStorage::new("data")?;

    // Create application state
    let state = Arc::new(AppState::new(storage, &config)?);

    // Create the router using the optimized WebSocket router
    let app = ws_router::create_router(state);

    // Start the server
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = TcpListener::bind(&addr).await?;
    println!("listening on {addr}");

    axum::serve(listener, app).await?;

    Ok(())
} 