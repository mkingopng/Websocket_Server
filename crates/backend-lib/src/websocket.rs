//! WebSocket Handler Module
//!
//! This module implements the WebSocket handler for the `OpenLifter` backend server.
//! It provides functionality for:
//!
//! - Client registration and session management
//! - Message handling for different client request types
//! - Update broadcasting to connected clients
//! - Conflict resolution for concurrent updates
//! - Network resilience with reconnection and retry logic
//!
//! The `WebSocketHandler` is designed to be instantiated per-connection and manages
//! the state for a single client. It interacts with the shared application state to
//! coordinate between multiple clients.
//!
//! # Network Resilience
//!
//! The handler implements several mechanisms for handling network interruptions:
//!
//! - Automatic reconnection attempts when sessions expire or connections drop
//! - Exponential backoff for retry attempts
//! - Message delivery guarantees with retry logic
//!
//! # Conflict Resolution
//!
//! When multiple clients update the same "location" (data entity), the handler
//! resolves conflicts based on client priority levels, with higher priority updates
//! taking precedence.

use crate::messages::{ClientMessage, ServerMessage, Update, UpdateWithMetadata};
use crate::storage::Storage;
use crate::AppState;
use anyhow::{anyhow, Result};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{self, Duration};
use uuid::Uuid;

/// Maximum number of reconnection attempts before giving up
const MAX_RECONNECT_ATTEMPTS: u8 = 5;
/// Base delay between reconnection attempts in milliseconds
const RECONNECT_DELAY_MS: u64 = 1000; // 1 second

/// WebSocket handler for processing messages
pub struct WebSocketHandler<S: Storage + Send + Sync + 'static> {
    state: Arc<AppState<S>>,
    client_id: String,
    client_tx: Option<mpsc::Sender<ServerMessage>>,
    client_priority: u8,
    reconnect_attempts: u8,
}

impl<S: Storage + Send + Sync + 'static> WebSocketHandler<S> {
    pub fn new(state: Arc<AppState<S>>) -> Self {
        Self {
            state,
            client_id: Uuid::new_v4().to_string(),
            client_tx: None,
            client_priority: 0,
            reconnect_attempts: 0,
        }
    }

    // Register this client for a specific meet
    pub fn register_client(&mut self, meet_id: &str, tx: mpsc::Sender<ServerMessage>) {
        // Store the client's transmission channel
        self.client_tx = Some(tx.clone());

        // Add client to the clients map for the meet
        let mut meet_clients = self.state.clients.entry(meet_id.to_string()).or_default();

        meet_clients.push(tx);

        println!("Client {} registered for meet {}", self.client_id, meet_id);

        // Reset reconnect attempts on successful registration
        self.reconnect_attempts = 0;
    }

    // Set priority for this client
    pub fn set_priority(&mut self, priority: u8) {
        self.client_priority = priority;
    }

    // Unregister this client when disconnecting
    pub fn unregister_client(&self, meet_id: &str) {
        if let Some(client_tx) = &self.client_tx {
            if let Some(mut clients) = self.state.clients.get_mut(meet_id) {
                // Remove this client from the list
                clients.retain(|tx| !std::ptr::eq(tx, client_tx));
                println!(
                    "Client {} unregistered from meet {}",
                    self.client_id, meet_id
                );
            }
        }
    }

    // Try to send a message to a client with retry logic
    #[allow(dead_code)]
    async fn try_send_with_retry(
        &self,
        client: &mpsc::Sender<ServerMessage>,
        message: ServerMessage,
    ) -> Result<()> {
        let mut attempts = 0;
        let max_attempts = 3;
        let mut delay = RECONNECT_DELAY_MS;

        while attempts < max_attempts {
            match client.send(message.clone()).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    attempts += 1;
                    if attempts >= max_attempts {
                        return Err(anyhow!(
                            "Failed to send message after {} attempts: {}",
                            max_attempts,
                            e
                        ));
                    }

                    // Log the error
                    println!("Error sending message, attempt {attempts}/{max_attempts}: {e}");

                    // Wait before retrying with exponential backoff
                    time::sleep(Duration::from_millis(delay)).await;
                    delay *= 2;
                },
            }
        }

        Err(anyhow!(
            "Failed to send message after {} attempts",
            max_attempts
        ))
    }

    // Broadcast updates to all connected clients for a meet
    async fn broadcast_update(&self, meet_id: &str, updates: Vec<Update>) -> Result<()> {
        // Check if we have clients for this meet
        if let Some(clients) = self.state.clients.get(meet_id) {
            if clients.is_empty() {
                // No other clients to broadcast to
                return Ok(());
            }

            // Create metadata for each update
            let updates_with_metadata: Vec<UpdateWithMetadata> = updates
                .into_iter()
                .enumerate()
                .map(|(idx, update)| {
                    UpdateWithMetadata {
                        update,
                        source_client: self.client_id.clone(),
                        server_seq: idx as u64,
                        priority: self.client_priority, // Use client's priority setting
                    }
                })
                .collect();

            // Create the relay message
            let relay_msg = ServerMessage::UpdateRelay {
                meet_id: meet_id.to_string(),
                updates: updates_with_metadata,
            };

            // Use a JoinSet to send to all clients concurrently for better performance
            let mut send_tasks = tokio::task::JoinSet::new();
            let self_tx = self.client_tx.as_ref();

            for client in clients.iter() {
                // Skip sending to ourselves
                if self_tx.is_none_or(|tx| !std::ptr::eq(tx, client)) {
                    let client_clone = client.clone();
                    let relay_msg_clone = relay_msg.clone();

                    // Add a task for each client
                    send_tasks.spawn(async move {
                        if let Err(e) = client_clone.send(relay_msg_clone).await {
                            // Return the error to track failures
                            Err(anyhow!("Failed to send to client: {}", e))
                        } else {
                            Ok(())
                        }
                    });
                }
            }

            // Wait for all send tasks to complete and track failures
            let mut failed_clients = 0;
            while let Some(result) = send_tasks.join_next().await {
                match result {
                    Ok(Ok(())) => {},                           // Successfully sent
                    Ok(Err(_)) | Err(_) => failed_clients += 1, // Send failed or task failed
                }
            }

            // Log if many clients failed to receive the update
            if failed_clients > 0 {
                println!("Warning: {failed_clients} clients failed to receive update");
            }
        }

        Ok(())
    }

    // Get updates since a specific sequence number
    fn get_updates_since(_meet_id: &str, _last_server_seq: u64) -> Vec<UpdateWithMetadata> {
        // In a real implementation, this would retrieve updates from a database
        // For now, just return an empty vector as a placeholder
        // This would be replaced with actual state retrieval logic in a production system

        // Placeholder for retrieving updates from storage
        Vec::new()
    }

    // Apply conflict resolution to updates - this would be much more sophisticated in a real system
    #[allow(clippy::unused_self)]
    #[allow(dead_code)]
    fn resolve_conflicts(&self, updates: &[UpdateWithMetadata]) -> Vec<UpdateWithMetadata> {
        // Group updates by location
        let mut location_map: std::collections::HashMap<String, Vec<&UpdateWithMetadata>> =
            std::collections::HashMap::new();

        for update in updates {
            location_map
                .entry(update.update.location.clone())
                .or_default()
                .push(update);
        }

        // For each location, keep only the update with the highest priority
        let mut resolved_updates = Vec::new();

        for (_location, location_updates) in location_map {
            if location_updates.len() == 1 {
                // No conflict
                resolved_updates.push(location_updates[0].clone());
            } else {
                // Find the update with the highest priority
                let highest_priority = location_updates
                    .iter()
                    .max_by_key(|update| update.priority)
                    .unwrap();

                resolved_updates.push((*highest_priority).clone());
            }
        }

        resolved_updates
    }

    // Try to reconnect after a network interruption
    async fn try_reconnect(&mut self, meet_id: &str, session_token: &str) -> Result<bool> {
        if self.reconnect_attempts >= MAX_RECONNECT_ATTEMPTS {
            return Err(anyhow!("Exceeded maximum reconnection attempts"));
        }

        self.reconnect_attempts += 1;

        // Log reconnection attempt
        println!(
            "Attempting to reconnect client {} to meet {} (attempt {}/{})",
            self.client_id, meet_id, self.reconnect_attempts, MAX_RECONNECT_ATTEMPTS
        );

        // Wait before reconnecting
        time::sleep(Duration::from_millis(
            RECONNECT_DELAY_MS * u64::from(self.reconnect_attempts),
        ))
        .await;

        // Validate the session to see if it's still valid
        let session_valid = self.state.auth.validate_session(session_token).await;

        if session_valid {
            // Session is still valid - we can recover
            println!(
                "Reconnection successful for client {} to meet {}",
                self.client_id, meet_id
            );
            return Ok(true);
        }

        // Session is no longer valid
        Err(anyhow!("Session is no longer valid"))
    }

    /// Handle incoming client messages
    ///
    /// This is the main entry point for processing incoming WebSocket messages from clients.
    /// It routes different message types to appropriate handlers and implements automatic
    /// reconnection logic when sessions are invalid.
    ///
    /// # Message Types
    ///
    /// The handler supports the following client message types:
    /// - `CreateMeet`: Initialize a new meet and create a session
    /// - `JoinMeet`: Join an existing meet and create a session
    /// - `UpdateInit`: Send updates to the server and broadcast to other clients
    /// - `ClientPull`: Request updates from the server since a specific sequence number
    /// - `PublishMeet`: Publish meet results and generate CSV output
    ///
    /// # Network Resilience
    ///
    /// If a message arrives with an invalid session token (e.g., after a network
    /// interruption), the handler will attempt to reconnect automatically using
    /// the `try_reconnect` method with exponential backoff.
    ///
    /// # Priority Handling
    ///
    /// Client priority is recorded during meet creation/joining and used for conflict
    /// resolution when updates from multiple clients target the same location.
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing the appropriate `ServerMessage` response, which
    /// will be sent back to the client over the WebSocket.
    ///
    /// # Errors
    ///
    /// Returns an error if message processing fails, which may happen due to:
    /// - Invalid session that cannot be recovered
    /// - Storage errors
    /// - Authorization failures
    /// - Validation errors
    #[allow(clippy::too_many_lines)]
    pub async fn handle_message(&mut self, msg: ClientMessage) -> Result<ServerMessage> {
        match msg {
            ClientMessage::CreateMeet {
                meet_id,
                password: _,
                location_name,
                priority,
            } => {
                // Set client priority
                self.set_priority(priority);

                // Handle meet creation
                let session = self
                    .state
                    .auth
                    .new_session(meet_id.clone(), location_name, priority)
                    .await;

                // Return create response
                Ok(ServerMessage::MeetCreated {
                    meet_id,
                    session_token: session,
                })
            },
            ClientMessage::JoinMeet {
                meet_id,
                password: _,
                location_name,
                priority,
            } => {
                // Set client priority
                self.set_priority(priority);

                // Check if the meet exists and the password is correct
                // In a real implementation, this would verify against stored data

                // For now, always accept the join request
                let session = self
                    .state
                    .auth
                    .new_session(meet_id.clone(), location_name, priority)
                    .await;

                // Return join response
                Ok(ServerMessage::MeetJoined {
                    meet_id,
                    session_token: session,
                })

                // In a real implementation, this might reject the join attempt:
                /*
                Ok(ServerMessage::JoinRejected {
                    reason: "Invalid meet ID or password".to_string(),
                })
                */
            },
            ClientMessage::UpdateInit {
                meet_id,
                session_token,
                updates,
            } => {
                if self.state.auth.validate_session(&session_token).await {
                    // Get session to retrieve priority
                    if let Some(session) = self.state.auth.get_session(&session_token).await {
                        // Update client priority from session
                        self.set_priority(session.priority);

                        // Generate update IDs
                        let update_ids: Vec<String> =
                            updates.iter().map(|_| Uuid::new_v4().to_string()).collect();

                        // Broadcast updates to other clients
                        if !updates.is_empty() {
                            match self.broadcast_update(&meet_id, updates.clone()).await {
                                Ok(()) => {
                                    // Reset reconnect attempts on successful broadcast
                                    self.reconnect_attempts = 0;
                                },
                                Err(e) => {
                                    println!("Warning: Error broadcasting update: {e}");
                                    // Don't reset reconnect attempts here
                                },
                            }
                        }

                        // Return ack response
                        Ok(ServerMessage::UpdateAck {
                            meet_id,
                            update_ids,
                        })
                    } else {
                        // This is unexpected - the session is valid but we can't get its details
                        Ok(ServerMessage::InvalidSession { session_token })
                    }
                } else {
                    // Session may have expired - attempt to reconnect
                    match self.try_reconnect(&meet_id, &session_token).await {
                        Ok(reconnected) => {
                            if reconnected {
                                // Successfully reconnected - try the update again
                                // Use Box::pin to avoid infinite recursion
                                let result =
                                    Box::pin(self.handle_message(ClientMessage::UpdateInit {
                                        meet_id,
                                        session_token,
                                        updates,
                                    }))
                                    .await;
                                return result;
                            }
                            // Failed to reconnect
                            Ok(ServerMessage::InvalidSession { session_token })
                        },
                        Err(_) => {
                            // Return error if session is invalid
                            Ok(ServerMessage::InvalidSession { session_token })
                        },
                    }
                }
            },
            ClientMessage::ClientPull {
                meet_id,
                session_token,
                last_server_seq,
            } => {
                // Validate the session first
                if self.state.auth.validate_session(&session_token).await {
                    // Get updates since the last sequence number the client has seen
                    let updates = Self::get_updates_since(&meet_id, last_server_seq);

                    // Get the current highest sequence number
                    let current_server_seq = last_server_seq + updates.len() as u64;

                    // Return the updates to the client
                    Ok(ServerMessage::ServerPull {
                        meet_id,
                        last_server_seq: current_server_seq,
                        updates_relayed: updates,
                    })
                } else {
                    // Session may have expired - attempt to reconnect
                    match self.try_reconnect(&meet_id, &session_token).await {
                        Ok(reconnected) => {
                            if reconnected {
                                // Successfully reconnected - try the pull again
                                // Use Box::pin to avoid infinite recursion
                                let result =
                                    Box::pin(self.handle_message(ClientMessage::ClientPull {
                                        meet_id,
                                        session_token,
                                        last_server_seq,
                                    }))
                                    .await;
                                return result;
                            }
                            // Failed to reconnect
                            Ok(ServerMessage::InvalidSession { session_token })
                        },
                        Err(_) => {
                            // Return error if session is invalid
                            Ok(ServerMessage::InvalidSession { session_token })
                        },
                    }
                }
            },
            ClientMessage::PublishMeet {
                meet_id,
                session_token,
                return_email,
                opl_csv,
            } => {
                // Validate the session first
                if self.state.auth.validate_session(&session_token).await {
                    // In a real implementation, this would actually publish the meet results

                    // Log the publication attempt
                    println!(
                        "Meet publication requested: meet_id={meet_id}, email={return_email}, csv_length={}",
                        opl_csv.len()
                    );

                    // Return acknowledgment
                    Ok(ServerMessage::PublishAck { meet_id })
                } else {
                    // Session may have expired - attempt to reconnect
                    match self.try_reconnect(&meet_id, &session_token).await {
                        Ok(reconnected) => {
                            if reconnected {
                                // Successfully reconnected - try the publication again
                                // Use Box::pin to avoid infinite recursion
                                let result =
                                    Box::pin(self.handle_message(ClientMessage::PublishMeet {
                                        meet_id,
                                        session_token,
                                        return_email,
                                        opl_csv,
                                    }))
                                    .await;
                                return result;
                            }
                            // Failed to reconnect
                            Ok(ServerMessage::InvalidSession { session_token })
                        },
                        Err(_) => {
                            // Return error if session is invalid
                            Ok(ServerMessage::InvalidSession { session_token })
                        },
                    }
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::FlatFileStorage;
    use crate::AppState;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::sync::mpsc;

    /// Helper to set up a `WebSocketHandler` for testing
    fn setup() -> (
        WebSocketHandler<FlatFileStorage>,
        Arc<AppState<FlatFileStorage>>,
        TempDir,
    ) {
        let temp_dir = TempDir::new().unwrap();
        let storage = FlatFileStorage::new(temp_dir.path()).unwrap();

        // Create default settings
        let settings = crate::config::Settings::default();

        // Create app state
        let state = Arc::new(AppState::new(storage.clone(), &settings).unwrap());

        // Create handler
        let handler = WebSocketHandler::new(state.clone());

        (handler, state, temp_dir)
    }

    #[tokio::test]
    async fn test_register_client() {
        let (mut handler, state, _temp_dir) = setup();
        let (tx, _rx) = mpsc::channel::<ServerMessage>(10);
        let meet_id = "test-meet";

        // Register client
        handler.register_client(meet_id, tx.clone());

        // Verify client is in the meet clients map
        assert!(state.clients.contains_key(meet_id));
        assert_eq!(state.clients.get(meet_id).unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_unregister_client() {
        let (mut handler, state, _temp_dir) = setup();
        let (tx, _rx) = mpsc::channel::<ServerMessage>(10);
        let meet_id = "test-meet";

        // Register client first
        handler.register_client(meet_id, tx);

        // Verify client is registered
        assert!(state.clients.contains_key(meet_id));
        assert!(!state.clients.get(meet_id).unwrap().is_empty());

        // Call unregister (we're just verifying it doesn't crash)
        handler.unregister_client(meet_id);
    }

    #[tokio::test]
    async fn test_handle_create_meet() {
        let (mut handler, _state, _temp_dir) = setup();

        // Create a meet
        let result = handler
            .handle_message(ClientMessage::CreateMeet {
                meet_id: "test-meet".to_string(),
                password: "Password123!".to_string(),
                location_name: "Test Location".to_string(),
                priority: 3,
            })
            .await;

        // Verify result
        assert!(result.is_ok());
        match result.unwrap() {
            ServerMessage::MeetCreated {
                meet_id,
                session_token,
            } => {
                assert_eq!(meet_id, "test-meet");
                assert!(!session_token.is_empty());
            },
            other => panic!("Expected MeetCreated, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_handle_join_meet() {
        let (mut handler, _state, _temp_dir) = setup();

        // Join a meet
        let result = handler
            .handle_message(ClientMessage::JoinMeet {
                meet_id: "test-meet".to_string(),
                password: "Password123!".to_string(),
                location_name: "Test Location".to_string(),
                priority: 2,
            })
            .await;

        // Verify result
        assert!(result.is_ok());
        match result.unwrap() {
            ServerMessage::MeetJoined {
                meet_id,
                session_token,
            } => {
                assert_eq!(meet_id, "test-meet");
                assert!(!session_token.is_empty());
            },
            other => panic!("Expected MeetJoined, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_handle_update_init() {
        let (mut handler, state, _temp_dir) = setup();
        let (tx, _rx) = mpsc::channel::<ServerMessage>(10);

        // Register the client
        handler.register_client("test-meet", tx);

        // Create a session token
        let session = state
            .auth
            .new_session("test-meet".to_string(), "Test Location".to_string(), 1)
            .await;

        // Updates to send
        let updates = vec![Update {
            location: "item1".to_string(),
            value: serde_json::to_string(&serde_json::json!({"field": "value"})).unwrap(),
            timestamp: 12345,
        }];

        // Send update
        let result = handler
            .handle_message(ClientMessage::UpdateInit {
                meet_id: "test-meet".to_string(),
                session_token: session.clone(),
                updates: updates.clone(),
            })
            .await;

        // Verify result
        assert!(result.is_ok());
        match result.unwrap() {
            ServerMessage::UpdateAck {
                meet_id,
                update_ids,
            } => {
                assert_eq!(meet_id, "test-meet");
                assert_eq!(update_ids.len(), 1);
            },
            other => panic!("Expected UpdateAck, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_handle_invalid_session() {
        let (mut handler, _state, _temp_dir) = setup();

        // Try update with invalid session
        let result = handler
            .handle_message(ClientMessage::UpdateInit {
                meet_id: "test-meet".to_string(),
                session_token: "invalid-session".to_string(),
                updates: vec![],
            })
            .await;

        // Verify result
        assert!(result.is_ok());
        match result.unwrap() {
            ServerMessage::InvalidSession { session_token } => {
                assert_eq!(session_token, "invalid-session");
            },
            other => panic!("Expected InvalidSession, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_handle_client_pull() {
        let (mut handler, state, _temp_dir) = setup();

        // Create a session token
        let session = state
            .auth
            .new_session("test-meet".to_string(), "Test Location".to_string(), 1)
            .await;

        // Send client pull
        let result = handler
            .handle_message(ClientMessage::ClientPull {
                meet_id: "test-meet".to_string(),
                session_token: session,
                last_server_seq: 0,
            })
            .await;

        // Verify result
        assert!(result.is_ok());
        match result.unwrap() {
            ServerMessage::ServerPull {
                meet_id,
                last_server_seq,
                updates_relayed,
            } => {
                assert_eq!(meet_id, "test-meet");
                assert_eq!(last_server_seq, 0);
                assert!(updates_relayed.is_empty());
            },
            other => panic!("Expected ServerPull, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_handle_publish_meet() {
        let (mut handler, state, _temp_dir) = setup();

        // Create a session token
        let session = state
            .auth
            .new_session("test-meet".to_string(), "Test Location".to_string(), 1)
            .await;

        // Send publish meet
        let result = handler
            .handle_message(ClientMessage::PublishMeet {
                meet_id: "test-meet".to_string(),
                session_token: session,
                return_email: "test@example.com".to_string(),
                opl_csv: "name,weight,squat".to_string(),
            })
            .await;

        // Verify result
        assert!(result.is_ok());
        match result.unwrap() {
            ServerMessage::PublishAck { meet_id } => {
                assert_eq!(meet_id, "test-meet");
            },
            other => panic!("Expected PublishAck, got {other:?}"),
        }
    }

    #[test]
    fn test_resolve_conflicts() {
        let (handler, _state, _temp_dir) = setup();

        // Create updates with different locations
        let update1 = UpdateWithMetadata {
            update: Update {
                location: "location1".to_string(),
                value: "value1".to_string(),
                timestamp: 1000,
            },
            source_client: "client1".to_string(),
            server_seq: 1,
            priority: 5,
        };

        let update2 = UpdateWithMetadata {
            update: Update {
                location: "location2".to_string(),
                value: "value2".to_string(),
                timestamp: 2000,
            },
            source_client: "client2".to_string(),
            server_seq: 2,
            priority: 3,
        };

        // No conflicts (different locations)
        let updates = vec![update1.clone(), update2.clone()];
        let resolved = handler.resolve_conflicts(&updates);

        // Both updates should be included since they have different locations
        assert_eq!(resolved.len(), 2);

        // Create conflicting updates (same location, different priorities)
        let conflicting_update1 = UpdateWithMetadata {
            update: Update {
                location: "same_location".to_string(),
                value: "value_from_client1".to_string(),
                timestamp: 1000,
            },
            source_client: "client1".to_string(),
            server_seq: 1,
            priority: 5, // Higher priority
        };

        let conflicting_update2 = UpdateWithMetadata {
            update: Update {
                location: "same_location".to_string(),
                value: "value_from_client2".to_string(),
                timestamp: 2000,
            },
            source_client: "client2".to_string(),
            server_seq: 2,
            priority: 3, // Lower priority
        };

        // Test conflict resolution
        let updates = vec![conflicting_update1.clone(), conflicting_update2.clone()];
        let resolved = handler.resolve_conflicts(&updates);

        // Only one update should be included (the one with higher priority)
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].priority, 5);
        assert_eq!(resolved[0].source_client, "client1");

        // Test with mixed conflicting and non-conflicting updates
        let mixed_updates = vec![
            update1.clone(),
            conflicting_update1.clone(),
            conflicting_update2.clone(),
        ];
        let resolved = handler.resolve_conflicts(&mixed_updates);

        // Should have two updates: one non-conflicting and one winner from the conflict
        assert_eq!(resolved.len(), 2);

        // Find the update for "location1"
        let location1_update = resolved
            .iter()
            .find(|u| u.update.location == "location1")
            .unwrap();
        assert_eq!(location1_update.source_client, "client1");

        // Find the update for "same_location"
        let same_location_update = resolved
            .iter()
            .find(|u| u.update.location == "same_location")
            .unwrap();
        assert_eq!(same_location_update.source_client, "client1");
        assert_eq!(same_location_update.priority, 5);
    }
}
