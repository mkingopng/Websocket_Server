// ============================
// openlifter-backend-lib/src/ws_router.rs
// ============================
//! WebSocket router and connection handling.
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use openlifter_common::{ClientToServer, ServerToClient};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc};
use crate::{AppState, error::AppError, handlers::live::handle_client_message};
use metrics::{counter, gauge};

/// Create the WebSocket router
pub fn router(app_state: AppState) -> Router<AppState> {
    Router::new()
        .route("/ws", get(ws_handler))
        .route("/healthz", get(health_check))
        .with_state(app_state)
}

/// Health check endpoint
async fn health_check() -> impl IntoResponse {
    "OK"
}

/// WebSocket connection handler
#[axum::debug_handler]
async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

/// Handle an individual WebSocket connection
async fn handle_socket(socket: WebSocket, state: AppState) {
    // Update metrics
    counter!("ws.connection", 1);
    gauge!("ws.active", 1.0, "action" => "inc");
    
    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = mpsc::channel(32);

    // Helper function to handle errors consistently
    let handle_error = |e: AppError| -> ServerToClient {
        ServerToClient::MalformedMessage { err_msg: e.to_string() }
    };

    // Helper function to send error messages
    let send_error = |tx: &mpsc::Sender<Message>, err: ServerToClient| {
        if let Ok(json) = serde_json::to_string(&err) {
            let _ = tx.try_send(Message::Text(json.into()));
        }
    };

    // Handle incoming messages
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let Message::Text(msg_text) = msg {
                match serde_json::from_str::<ClientToServer>(&msg_text) {
                    Ok(client_msg) => {
                        match handle_client_message(client_msg, &state, tx.clone()).await {
                            Ok(_) => (),
                            Err(e) => send_error(&tx, handle_error(e)),
                        }
                    }
                    Err(e) => {
                        let err = ServerToClient::MalformedMessage { 
                            err_msg: format!("Failed to parse message: {}", e) 
                        };
                        send_error(&tx, err);
                    }
                }
            }
        }
    });

    // Handle outgoing messages
    let mut send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = (&mut recv_task) => send_task.abort(),
        _ = (&mut send_task) => recv_task.abort(),
    };
    
    // Update metrics
    gauge!("ws.active", -1.0, "action" => "dec");
} 