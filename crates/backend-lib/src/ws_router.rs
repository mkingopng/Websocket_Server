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
use crate::messages::ClientMessage;
use crate::messages::ServerMessage;

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

    // Create a channel for sending messages to the client websocket
    let (client_tx, mut client_rx) = mpsc::channel(32);
    
    // Create a separate channel for ServerMessage
    let (server_tx, mut server_rx) = mpsc::channel::<ServerMessage>(32);

    // Create WebSocket handler
    let mut handler = WebSocketHandler::new(state.clone());

    // Track which meet this client is connected to (for unregistering later)
    let mut connected_meet_id: Option<String> = None;

    // Task 1: Forward messages from client_rx to the websocket
    let send_task = tokio::spawn(async move {
        while let Some(msg) = client_rx.recv().await {
            if let Err(e) = ws_tx.send(msg).await {
                eprintln!("Failed to send message: {e}");
                break;
            }
        }
    });
    
    // Task 2: Convert ServerMessages to WebSocket Messages
    let client_tx_clone = client_tx.clone();
    tokio::spawn(async move {
        while let Some(server_msg) = server_rx.recv().await {
            // Convert ServerMessage to JSON string
            if let Ok(json) = serde_json::to_string(&server_msg) {
                // Send as WebSocket text message
                if let Err(e) = client_tx_clone.send(Message::Text(json.into())).await {
                    eprintln!("Failed to convert server message: {e}");
                    break;
                }
            }
        }
    });

    // Handle incoming messages
    while let Some(msg) = ws_rx.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                // Parse the message
                match serde_json::from_str::<ClientMessage>(&text) {
                    Ok(client_msg) => {
                        // Extract meet_id from message if present to update connected_meet_id
                        let meet_id = match &client_msg {
                            ClientMessage::CreateMeet { meet_id, .. } => Some(meet_id.clone()),
                            ClientMessage::JoinMeet { meet_id, .. } => Some(meet_id.clone()),
                            ClientMessage::UpdateInit { meet_id, .. } => Some(meet_id.clone()),
                            ClientMessage::ClientPull { meet_id, .. } => Some(meet_id.clone()),
                            ClientMessage::PublishMeet { meet_id, .. } => Some(meet_id.clone()),
                        };
                        
                        // If this is the first message with a meet_id, register the client
                        if connected_meet_id.is_none() && meet_id.is_some() {
                            let meet_id_val = meet_id.clone().unwrap();
                            handler.register_client(&meet_id_val, server_tx.clone());
                            connected_meet_id = meet_id.clone();
                        }
                        
                        match handler.handle_message(client_msg).await {
                            Ok(response) => {
                                // Serialize response to JSON
                                let response_json = serde_json::to_string(&response)
                                    .unwrap_or_else(|_| String::from("{\"type\":\"Error\",\"payload\":{\"code\":\"SERIALIZATION_ERROR\",\"message\":\"Failed to serialize response\"}}"));
                                
                                // Send response directly without using server_tx
                                if let Err(e) = client_tx.send(Message::Text(response_json.into())).await {
                                    eprintln!("Failed to send response: {e}");
                                    break;
                                }
                            },
                            Err(e) => {
                                let err_msg = ServerMessage::Error {
                                    code: "INTERNAL_ERROR".to_string(),
                                    message: e.to_string(),
                                };
                                
                                if let Ok(err_str) = serde_json::to_string(&err_msg) {
                                    if let Err(e) = client_tx.send(Message::Text(err_str.into())).await {
                                        eprintln!("Failed to send error message: {e}");
                                        break;
                                    }
                                }
                            }
                        }
                    },
                    Err(e) => {
                        // Handle malformed message
                        let err_msg = ServerMessage::MalformedMessage {
                            err_msg: e.to_string(),
                        };
                        
                        if let Ok(err_str) = serde_json::to_string(&err_msg) {
                            if let Err(e) = client_tx.send(Message::Text(err_str.into())).await {
                                eprintln!("Failed to send error message: {e}");
                                break;
                            }
                        }
                    }
                }
            },
            Ok(Message::Close(_)) => break,
            _ => (),
        }
    }

    // Unregister client when connection closes
    if let Some(meet_id) = connected_meet_id {
        handler.unregister_client(&meet_id);
    }

    // Update metrics when connection closes
    let _ = gauge!("ws.active", &[("value", "-1")]);

    // Cancel the send task
    send_task.abort();
} 