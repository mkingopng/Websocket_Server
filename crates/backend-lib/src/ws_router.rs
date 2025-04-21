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
use crate::{AppState, error::AppError, handlers::live};
use metrics::{counter, gauge};
use std::sync::Arc;
use openlifter_common::{ClientToServer, ServerToClient};
use crate::storage::Storage;

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
    ws.on_upgrade(move |socket| async move {
        if let Err(e) = handle_connection(socket, state).await {
            eprintln!("WebSocket error: {e}");
        }
    })
}

/// Handle an individual WebSocket connection
async fn handle_connection<S: Storage + Send + Sync + Clone + 'static>(
    socket: WebSocket,
    state: Arc<AppState<S>>,
) -> Result<(), AppError> {
    // Update metrics
    let _ = counter!("ws.connection", &[("value", "1")]);
    let _ = gauge!("ws.active", &[("value", "1")]);

    // Split the socket
    let (mut ws_tx, mut ws_rx) = socket.split();

    // Create a channel for sending messages back to the client
    let (tx, mut rx) = mpsc::channel(32);

    // Spawn task to forward messages from channel to websocket
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if let Err(e) = ws_tx.send(msg).await {
                return Err(AppError::Internal(format!("Failed to send message: {e}")));
            }
        }
        Ok(())
    });

    // Handle incoming messages
    while let Some(Ok(msg)) = ws_rx.next().await {
        match msg {
            Message::Text(text) => {
                match serde_json::from_str::<ClientToServer>(&text) {
                    Ok(client_msg) => {
                        if let Err(e) = live::handle_client_message(client_msg, &state, tx.clone()).await {
                            let err = ServerToClient::MalformedMessage { 
                                err_msg: format!("Failed to handle message: {e}") 
                            };
                            let json = serde_json::to_string(&err)?;
                            tx.send(Message::Text(json.into())).await?;
                        }
                    }
                    Err(e) => {
                        let err = ServerToClient::MalformedMessage { 
                            err_msg: format!("Failed to parse message: {e}") 
                        };
                        let json = serde_json::to_string(&err)?;
                        tx.send(Message::Text(json.into())).await?;
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
    Ok(())
} 