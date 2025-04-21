// ============================
// openlifter-backend-lib/src/ws_router.rs
// ============================
//! WebSocket router and connection handling.
use axum::{
    extract::{
        ws::{WebSocket, Message, WebSocketUpgrade},
        State,
    },
    routing::get,
    Router,
    response::IntoResponse,
};
use futures_util::{StreamExt, SinkExt};
use tokio::sync::mpsc;
use metrics::{counter, gauge};
use std::sync::Arc;
use crate::AppState;
use crate::storage::Storage;
use crate::websocket::WebSocketHandler;

/// Create the WebSocket router
pub fn create_router<S: Storage + Send + Sync + Clone + 'static>(state: Arc<AppState<S>>) -> Router {
    Router::new()
        .route("/ws", get(ws_handler))
        .with_state(state)
}

/// WebSocket connection handler
async fn ws_handler<S: Storage + Send + Sync + Clone + 'static>(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState<S>>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| {
        let state = state.clone();
        async move {
            handle_connection(socket, state).await;
        }
    })
}

/// Handle an individual WebSocket connection
async fn handle_connection<S: Storage + Send + Sync + Clone + 'static>(
    socket: WebSocket,
    state: Arc<AppState<S>>,
) {
    // Update metrics
    let _ = counter!("ws.connection", &[("value", "1")]);
    let _ = gauge!("ws.active", &[("value", "1")]);

    // Split the socket
    let (mut ws_tx, mut ws_rx) = socket.split();

    // Create a channel for sending messages back to the client
    let (tx, mut rx) = mpsc::channel(32);

    // Create WebSocket handler
    let handler = WebSocketHandler::new(state);

    // Spawn task to forward messages from channel to websocket
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if let Err(e) = ws_tx.send(msg).await {
                eprintln!("Failed to send message: {}", e);
                break;
            }
        }
    });

    // Handle incoming messages
    while let Some(Ok(msg)) = ws_rx.next().await {
        match msg {
            Message::Text(text) => {
                match serde_json::from_str(&text) {
                    Ok(client_msg) => {
                        let err_tx = tx.clone();
                        if let Err(e) = handler.handle_message(client_msg).await {
                            let err = serde_json::json!({
                                "type": "Error",
                                "payload": {
                                    "code": "INTERNAL_ERROR",
                                    "message": e.to_string()
                                }
                            });
                            let err_str = err.to_string();
                            if let Err(e) = err_tx.send(Message::Text(err_str.into())).await {
                                eprintln!("Failed to send error message: {}", e);
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        let err = serde_json::json!({
                            "type": "Error",
                            "payload": {
                                "code": "MALFORMED_MESSAGE",
                                "message": e.to_string()
                            }
                        });
                        let err_str = err.to_string();
                        if let Err(e) = tx.send(Message::Text(err_str.into())).await {
                            eprintln!("Failed to send error message: {}", e);
                            break;
                        }
                    }
                }
            }
            Message::Close(_) => break,
            _ => (),
        }
    }

    // Update metrics when connection closes
    let _ = gauge!("ws.active", &[("value", "-1")]);

    // Cancel the send task
    send_task.abort();
} 