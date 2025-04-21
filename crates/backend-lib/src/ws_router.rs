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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Settings;
    use crate::messages::{ClientMessage, ServerMessage};
    use crate::storage::FlatFileStorage;
    use crate::AppState;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::sync::mpsc;

    // Helper to set up a test environment for WebSocketHandler
    fn setup() -> (
        WebSocketHandler<FlatFileStorage>,
        Arc<AppState<FlatFileStorage>>,
        TempDir,
    ) {
        let temp_dir = TempDir::new().unwrap();
        let storage = FlatFileStorage::new(temp_dir.path()).unwrap();

        // Create default settings
        let settings = Settings::default();

        // Create app state
        let state = Arc::new(AppState::new(storage.clone(), &settings).unwrap());

        // Create handler
        let handler = WebSocketHandler::new(state.clone());

        (handler, state, temp_dir)
    }

    #[tokio::test]
    async fn test_router_creation() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FlatFileStorage::new(temp_dir.path()).unwrap();
        let settings = Settings::default();
        let state = Arc::new(AppState::new(storage.clone(), &settings).unwrap());

        // Create router
        let _router = create_router(state);

        // Just verify it creates a router without panicking
        // If we get this far, the test passes
    }

    #[tokio::test]
    async fn test_handler_process_message() {
        let (mut handler, _state, _temp_dir) = setup();

        // Create a meet message
        let create_meet = ClientMessage::CreateMeet {
            meet_id: "test-meet".to_string(),
            password: "Password123!".to_string(),
            location_name: "Test Location".to_string(),
            priority: 5,
        };

        // Handle the message directly with the handler
        let result = handler.handle_message(create_meet).await;

        // Verify result
        assert!(result.is_ok());

        // Check the response
        let response = result.unwrap();
        match response {
            ServerMessage::MeetCreated {
                meet_id,
                session_token,
            } => {
                assert_eq!(meet_id, "test-meet");
                assert!(!session_token.is_empty());
            },
            _ => panic!("Expected MeetCreated response, got {response:?}"),
        }
    }

    #[tokio::test]
    async fn test_error_serialization() {
        // Test error serialization
        let error_msg = ServerMessage::Error {
            code: "TEST_ERROR".to_string(),
            message: "This is a test error".to_string(),
        };

        // Serialize to JSON
        let json = serde_json::to_string(&error_msg).unwrap();

        // Print JSON for debugging
        println!("Serialized JSON: {json}");

        // Verify serialization
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["type"].as_str().unwrap(), "Error");
        assert_eq!(parsed["payload"]["code"].as_str().unwrap(), "TEST_ERROR");
        assert_eq!(
            parsed["payload"]["message"].as_str().unwrap(),
            "This is a test error"
        );
    }

    #[tokio::test]
    async fn test_validation_errors() {
        // Test validation
        let invalid_meet = ClientMessage::CreateMeet {
            meet_id: String::new(), // Invalid empty meet ID
            password: "Password123!".to_string(),
            location_name: "Test Location".to_string(),
            priority: 5,
        };

        // Validate the message
        let result = validation::validate_client_message(&invalid_meet);

        // Verify validation error
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("Invalid meet ID"));
    }

    #[tokio::test]
    async fn test_message_handling_workflow() {
        let (mut handler, _state, _temp_dir) = setup();

        // Create a meet
        let create_result = handler
            .handle_message(ClientMessage::CreateMeet {
                meet_id: "workflow-test".to_string(),
                password: "Password123!".to_string(),
                location_name: "Workflow Test".to_string(),
                priority: 5,
            })
            .await
            .unwrap();

        // Extract session token using let...else
        let ServerMessage::MeetCreated { session_token, .. } = create_result else {
            panic!("Expected MeetCreated response")
        };

        // Register a client channel
        let (tx, _rx) = mpsc::channel::<ServerMessage>(10);
        handler.register_client("workflow-test", tx);

        // Send an update
        let update_result = handler
            .handle_message(ClientMessage::UpdateInit {
                meet_id: "workflow-test".to_string(),
                session_token: session_token.clone(),
                updates: vec![crate::messages::Update {
                    location: "test.item1".to_string(),
                    value: "{\"name\":\"Test Item\",\"value\":123}".to_string(),
                    timestamp: 12345,
                }],
            })
            .await
            .unwrap();

        // Verify update result
        match update_result {
            ServerMessage::UpdateAck {
                meet_id,
                update_ids,
            } => {
                assert_eq!(meet_id, "workflow-test");
                assert_eq!(update_ids.len(), 1);
            },
            _ => panic!("Expected UpdateAck response"),
        }

        // Pull updates
        let pull_result = handler
            .handle_message(ClientMessage::ClientPull {
                meet_id: "workflow-test".to_string(),
                session_token,
                last_server_seq: 0,
            })
            .await
            .unwrap();

        // Verify pull result
        match pull_result {
            ServerMessage::ServerPull {
                meet_id,
                last_server_seq,
                ..
            } => {
                assert_eq!(meet_id, "workflow-test");
                assert_eq!(last_server_seq, 0); // No updates yet in our implementation
            },
            _ => panic!("Expected ServerPull response"),
        }
    }
}
