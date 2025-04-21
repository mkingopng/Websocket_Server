// ============================
// openlifter-backend-lib/src/ws_router.rs
// ============================
//! WebSocket router and connection handling.
use crate::messages::{ClientMessage, ServerMessage};
use crate::storage::Storage;
use crate::validation;
use crate::websocket::WebSocketHandler;
use crate::AppState;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use metrics::{counter, gauge};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Create the WebSocket router
pub fn create_router<S: Storage + Send + Sync + Clone + 'static>(
    state: Arc<AppState<S>>,
) -> Router {
    Router::new()
        .route("/ws", get(ws_handler))
        .with_state(state)
}

/// Handler for WebSocket connections
pub async fn ws_handler<S: Storage + Send + Sync + Clone + 'static>(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState<S>>>,
) -> impl IntoResponse {
    // Update metrics
    let _ = counter!("ws.connection", &[("value", "1")]);
    let _ = gauge!("ws.active", &[("value", "1")]);

    // Upgrade the connection to a WebSocket
    ws.on_upgrade(move |socket| handle_connection(socket, state))
}

#[allow(clippy::too_many_lines)]
async fn handle_connection<S: Storage + Send + Sync + Clone + 'static>(
    socket: WebSocket,
    state: Arc<AppState<S>>,
) {
    let (mut tx, rx) = socket.split();

    // Create a channel for sending messages to the client websocket
    let (client_tx, mut client_rx) = mpsc::channel(32);

    // Create a separate channel for ServerMessage
    let (server_tx, mut server_rx) = mpsc::channel::<ServerMessage>(32);

    // Create WebSocket handler
    let mut handler = WebSocketHandler::new(state);

    // Track the meet ID this connection is registered for
    let mut connected_meet_id = None;

    // Task 1: Forward messages from the client channel to the WebSocket
    let send_task = tokio::spawn(async move {
        while let Some(message) = client_rx.recv().await {
            if tx.send(message).await.is_err() {
                break;
            }
        }
    });

    // Task 2: Convert ServerMessages to WebSocket Messages
    let client_tx_clone = client_tx.clone();
    tokio::spawn(async move {
        while let Some(server_msg) = server_rx.recv().await {
            let json = serde_json::to_string(&server_msg).unwrap_or_default();
            if client_tx_clone
                .send(Message::Text(json.into()))
                .await
                .is_err()
            {
                break;
            }
        }
    });

    // Main task: Process incoming WebSocket messages
    let mut rx = rx;
    while let Some(Ok(message)) = rx.next().await {
        match message {
            Message::Text(text) => {
                // Parse the message as a client message
                match serde_json::from_str::<ClientMessage>(&text) {
                    Ok(client_msg) => {
                        // Validate the message
                        match validation::validate_client_message(&client_msg) {
                            Ok(()) => {
                                // Extract meet_id from message if present to update connected_meet_id
                                let meet_id = match &client_msg {
                                    ClientMessage::CreateMeet { meet_id, .. }
                                    | ClientMessage::JoinMeet { meet_id, .. }
                                    | ClientMessage::UpdateInit { meet_id, .. }
                                    | ClientMessage::ClientPull { meet_id, .. }
                                    | ClientMessage::PublishMeet { meet_id, .. } => {
                                        Some(meet_id.clone())
                                    },
                                };

                                // If this is the first message with a meet_id, register the client
                                if connected_meet_id.is_none() && meet_id.is_some() {
                                    let meet_id_val = meet_id.clone().unwrap();
                                    handler.register_client(&meet_id_val, server_tx.clone());
                                    connected_meet_id = meet_id;
                                }

                                match handler.handle_message(client_msg).await {
                                    Ok(response) => {
                                        // Serialize response to JSON
                                        let response_json = serde_json::to_string(&response)
                                            .unwrap_or_else(|_| String::from("{\"type\":\"Error\",\"payload\":{\"code\":\"SERIALIZATION_ERROR\",\"message\":\"Failed to serialize response\"}}"));

                                        // Send response directly without using server_tx
                                        if let Err(e) = client_tx
                                            .send(Message::Text(response_json.into()))
                                            .await
                                        {
                                            eprintln!("Failed to send response: {e}");
                                            break;
                                        }
                                    },
                                    Err(e) => {
                                        // Handle error
                                        let err_msg = ServerMessage::Error {
                                            code: "INTERNAL_ERROR".to_string(),
                                            message: e.to_string(),
                                        };

                                        if let Ok(err_str) = serde_json::to_string(&err_msg) {
                                            if let Err(e) =
                                                client_tx.send(Message::Text(err_str.into())).await
                                            {
                                                eprintln!("Failed to send error message: {e}");
                                                break;
                                            }
                                        }
                                    },
                                }
                            },
                            Err(validation_err) => {
                                // Handle validation error
                                let err_msg = ServerMessage::Error {
                                    code: "VALIDATION_ERROR".to_string(),
                                    message: validation_err.to_string(),
                                };

                                if let Ok(err_str) = serde_json::to_string(&err_msg) {
                                    if let Err(e) =
                                        client_tx.send(Message::Text(err_str.into())).await
                                    {
                                        eprintln!("Failed to send validation error message: {e}");
                                        break;
                                    }
                                }
                            },
                        }
                    },
                    Err(e) => {
                        // Handle JSON parsing error
                        let err_msg = ServerMessage::MalformedMessage {
                            err_msg: e.to_string(),
                        };

                        if let Ok(err_str) = serde_json::to_string(&err_msg) {
                            if let Err(e) = client_tx.send(Message::Text(err_str.into())).await {
                                eprintln!("Failed to send error message: {e}");
                                break;
                            }
                        }
                    },
                }
            },
            Message::Close(_) => break,
            _ => {}, // Ignore other message types for now
        }
    }

    // Cleanup: unregister client when connection drops
    if let Some(meet_id) = connected_meet_id {
        handler.unregister_client(&meet_id);
    }

    // Update metrics
    let _ = counter!("ws.disconnection", &[("value", "1")]);
    let _ = gauge!("ws.active", &[("value", "-1")]);

    // Cancel the send task
    send_task.abort();
}
