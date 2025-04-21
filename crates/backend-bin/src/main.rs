use axum::{
    Router,
    extract::{
        ws::{WebSocket, WebSocketUpgrade},
        State,
    },
    routing::get,
    response::IntoResponse,
};
use backend_lib::{AppState, config::Settings, storage::FlatFileStorage};
use std::net::SocketAddr;
use tokio::net::TcpListener;

async fn handle_socket(mut socket: WebSocket, _state: State<AppState<FlatFileStorage>>) {
    println!("New WebSocket connection established");
    
    while let Some(msg) = socket.recv().await {
        let Ok(msg) = msg else {
            return;
        };
        
        println!("Received message: {msg:?}");
        
        if let Err(e) = socket.send(msg).await {
            println!("Error sending message: {e}");
            return;
        }
    }
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState<FlatFileStorage>>,
) -> impl IntoResponse {
    println!("WebSocket upgrade request received");
    ws.on_upgrade(move |socket| handle_socket(socket, State(state)))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Initialize configuration
    let config = Settings::load()?;

    // Create storage
    let storage = FlatFileStorage::new("data")?;

    // Create application state
    let state = AppState::new(storage, config)?;

    // Create the router
    let app = Router::new()
        .route("/ws", get(ws_handler))
        .with_state(state);

    // Start the server
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = TcpListener::bind(&addr).await?;
    println!("listening on {addr}");

    axum::serve(listener, app).await?;

    Ok(())
} 