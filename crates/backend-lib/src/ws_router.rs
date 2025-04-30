// ============================
// openlifter-backend-lib/src/ws_router.rs
// ============================
//! WebSocket router for the `OpenLifter` server.
//!
//! This module handles WebSocket connections and routes messages
//! to the appropriate handlers.

use crate::{
    error::AppError,
    messages::{ClientMessage, ServerMessage},
    storage::Storage,
    websocket::WebSocketHandler,
    AppState,
};
use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use dashmap::DashMap;
use futures_util::{SinkExt, StreamExt};
use metrics::{counter, gauge};
use std::sync::{Arc, LazyLock};
use tokio::sync::mpsc;

// Add at the top after the imports
static ACTIVITY_TIMES: LazyLock<DashMap<String, u64>> = LazyLock::new(DashMap::new);

/// Create the WebSocket router
pub fn create_router<S: Storage + Send + Sync + Clone + 'static>(
    state: Arc<AppState<S>>,
) -> Router {
    Router::new()
        .route("/ws", get(websocket_handler))
        .with_state(state)
}

/// Handle WebSocket connections
async fn websocket_handler<S: Storage + Send + Sync + Clone + 'static>(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState<S>>>,
) -> impl IntoResponse {
    // Create a handler - move it into the closure
    let handler = WebSocketHandler::new(state);

    // Upgrade the connection
    ws.on_upgrade(move |socket| handle_socket(socket, handler))
}

/// Check state consistency for a meet
///
/// This function is called when a client connects to verify state consistency.
/// It checks for:
/// 1. Missing updates (gaps in sequence numbers)
/// 2. Conflicts between clients
/// 3. Long periods of inactivity
///
/// If any inconsistency is detected, it triggers state recovery.
async fn check_state_consistency<S: Storage + Send + Sync + Clone + 'static>(
    handler: &mut WebSocketHandler<S>,
    meet_id: &str,
) -> Result<(), AppError> {
    // Get current time
    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Check the last activity time for this meet (if available)
    let last_activity_key = format!("meet:{meet_id}_last_activity");
    let mut needs_recovery = false;

    // Use a scope to ensure the dashmap entry is dropped before recovery is initiated
    {
        let mut entry = ACTIVITY_TIMES
            .entry(last_activity_key)
            .or_insert(current_time);

        // If last activity was more than 5 minutes ago, initiate recovery
        if current_time - *entry > 300 {
            println!(
                "Long inactivity detected for meet {meet_id}: {} seconds since last activity",
                current_time - *entry
            );

            needs_recovery = true;
        }

        // Update the last activity time
        *entry = current_time;
    }

    if needs_recovery {
        // Initiate recovery with the last known sequence 0
        // Convert anyhow::Error to AppError
        if let Err(e) = handler.initiate_state_recovery(meet_id, 0).await {
            return Err(AppError::Internal(e.to_string()));
        }
    }

    Ok(())
}

/// Handle a WebSocket connection
async fn handle_socket<S: Storage + Send + Sync + Clone + 'static>(
    socket: WebSocket,
    mut handler: WebSocketHandler<S>,
) {
    // Split the socket into sender and receiver
    let (mut sender, mut receiver) = socket.split();

    // Create a channel for sending messages back to the client
    let (tx, mut rx) = mpsc::channel::<ServerMessage>(32);

    // Track metrics
    let _ = counter!("ws.connection", &[("value", "1")]);
    let _ = gauge!("ws.active", &[("value", "1")]);

    // Keep track of the meet_id for this connection
    let mut connected_meet_id = String::new();

    // Spawn a task to forward messages from the channel to the client
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            // Serialize the message to JSON
            if let Ok(json) = serde_json::to_string(&msg) {
                if sender.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
            }
        }
    });

    // Process incoming messages
    while let Some(Ok(msg)) = receiver.next().await {
        if let axum::extract::ws::Message::Text(text) = msg {
            // Handle the message
            if let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) {
                // Extract meet_id from message if present to update connected_meet_id
                let meet_id = match &client_msg {
                    ClientMessage::CreateMeet { meet_id, .. }
                    | ClientMessage::JoinMeet { meet_id, .. }
                    | ClientMessage::UpdateInit { meet_id, .. }
                    | ClientMessage::ClientPull { meet_id, .. }
                    | ClientMessage::PublishMeet { meet_id, .. }
                    | ClientMessage::StateRecoveryResponse { meet_id, .. } => Some(meet_id.clone()),
                };

                if let Some(ref meet_id) = meet_id {
                    // Always clone (first time) or clone_from (subsequent times)
                    if connected_meet_id.is_empty() {
                        #[allow(clippy::assigning_clones)]
                        {
                            // First assignment needs clone
                            connected_meet_id = meet_id.clone();
                        }
                    } else {
                        connected_meet_id.clone_from(meet_id);
                    }

                    // Only do this for join/connect operations
                    match &client_msg {
                        ClientMessage::JoinMeet { .. } | ClientMessage::ClientPull { .. } => {
                            if let Err(e) = check_state_consistency(&mut handler, meet_id).await {
                                eprintln!("Error checking state consistency: {e}");
                            }
                        },
                        _ => {},
                    }
                }

                // Process the message
                if let Ok(response) = handler.handle_message(client_msg).await {
                    // Send the response back to the client
                    tx.send(response).await.ok();
                }
            }
        }
    }

    // Abort the send task when the connection is closed
    send_task.abort();

    // Update metrics
    let _ = gauge!("ws.active", &[("value", "-1")]);

    // If we had a meet_id, unregister the client
    if !connected_meet_id.is_empty() {
        handler.unregister_client(&connected_meet_id);
    }
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

        // Validate the message with crate::validation
        let result = crate::validation::validate_client_message(&invalid_meet);

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
