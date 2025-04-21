use axum::{
    Router,
    extract::{ws::{WebSocket, WebSocketUpgrade}, State},
    routing::get,
    response::IntoResponse,
};
use backend_lib::{AppState, config::Settings, storage::FlatFileStorage};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use futures_util::{SinkExt};

async fn handle_socket(mut socket: WebSocket, state: State<AppState>) {
    println!("New WebSocket connection established");
    
    // Echo any messages received
    while let Some(msg) = socket.recv().await {
        println!("Received message: {:?}", msg);
        if let Ok(msg) = msg {
            if let Err(e) = socket.send(msg).await {
                println!("Error sending message: {}", e);
                break;
            }
        } else {
            println!("Error receiving message");
            break;
        }
    }
    println!("WebSocket connection closed");
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    println!("WebSocket upgrade request received");
    ws.on_upgrade(move |socket| handle_socket(socket, State(state)))
}

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
        .route("/ws", get(ws_handler))
        .with_state(state);

    // Run it
    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    println!("listening on {}", addr);
    
    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service())
        .await?;

    Ok(())
} 