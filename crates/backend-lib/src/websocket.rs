// ==================
// crates/backend-lib/src/websocket.rs
// ==================
//! WebSocket Handler Module
//!
//! This module implements the WebSocket handler for the `OpenLifter` backend server.
//! It provides functionality for:
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
//! The handler implements several mechanisms for handling network interruptions:
//! - Automatic reconnection attempts when sessions expire or connections drop
//! - Exponential backoff for retry attempts
//! - Message delivery guarantees with retry logic
//!
//! # Conflict Resolution
//! When multiple clients update the same "location" (data entity), the handler
//! resolves conflicts based on client priority levels, with higher priority updates
//! taking precedence.

use crate::messages::{ClientMessage, ServerMessage, Update, UpdateWithMetadata};
use crate::storage::Storage;
use crate::AppState;
use anyhow::{anyhow, Result};
use std::net::IpAddr;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{self, Duration};
use uuid::Uuid;

/// Maximum number of reconnection attempts before giving up
const MAX_RECONNECT_ATTEMPTS: u8 = 5;
/// Base delay between reconnection attempts in milliseconds
const RECONNECT_DELAY_MS: u64 = 1000; // 1 second

/// WebSocket handler for processing messages
pub struct WebSocketHandler<S: Storage + Send + Sync + Clone + 'static> {
    state: Arc<AppState<S>>,
    client_id: String,
    client_tx: Option<mpsc::Sender<ServerMessage>>,
    client_priority: u8,
    reconnect_attempts: u8,
    client_ip: Option<IpAddr>,
}

impl<S: Storage + Send + Sync + Clone + 'static> WebSocketHandler<S> {
    pub fn new(state: Arc<AppState<S>>) -> Self {
        Self {
            state,
            client_id: Uuid::new_v4().to_string(),
            client_tx: None,
            client_priority: 0,
            reconnect_attempts: 0,
            client_ip: None,
        }
    }

    /// Set client IP address
    pub fn set_client_ip(&mut self, ip: IpAddr) {
        self.client_ip = Some(ip);
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
    #[allow(dead_code)]
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

    /// Initiate state recovery for a meet
    /// This method is called when the server detects a state inconsistency
    /// or after restart. It broadcasts a request to all connected clients
    /// to send their update logs.
    pub async fn initiate_state_recovery(&self, meet_id: &str, last_known_seq: u64) -> Result<()> {
        println!("State recovery needed for meet {meet_id}: last_known_seq={last_known_seq}");

        // Create recovery request message
        let recovery_msg = ServerMessage::StateRecoveryRequest {
            meet_id: meet_id.to_string(),
            last_known_seq,
        };

        // Send to all connected clients for this meet
        if let Some(clients) = self.state.clients.get(meet_id) {
            let client_count = clients.len();

            // Use a JoinSet to send to all clients concurrently
            let mut send_tasks = tokio::task::JoinSet::new();

            for client in clients.iter() {
                let client_clone = client.clone();
                let recovery_msg_clone = recovery_msg.clone();

                send_tasks.spawn(async move { client_clone.send(recovery_msg_clone).await });
            }

            // Wait for tasks to complete
            while let Some(result) = send_tasks.join_next().await {
                // Just log errors
                if let Err(e) = result {
                    println!("Error sending recovery request: {e}");
                }
            }

            println!("State recovery requested from {client_count} clients for meet {meet_id}");
        } else {
            println!("No clients connected for meet {meet_id}, recovery not possible");
        }

        Ok(())
    }

    /// Handle a state recovery response from a client
    /// This method processes updates from a client during state recovery,
    /// resolving conflicts and updating the server's state.
    async fn handle_state_recovery_response(
        &self,
        meet_id: &str,
        session_token: &str,
        _last_seq_num: u64,
        updates: Vec<Update>,
        priority: u8,
    ) -> Result<ServerMessage> {
        // Validate session
        let session_valid = self.state.auth.validate_session(session_token).await;
        if !session_valid {
            return Ok(ServerMessage::InvalidSession {
                session_token: session_token.to_string(),
            });
        }

        println!(
            "Processing state recovery response from client {} with {} updates",
            self.client_id,
            updates.len()
        );

        // Get handle to the meet actor using if let instead of match
        let meet_handle = if let Some(handle) = self.state.meet_handles.get(meet_id) {
            handle.clone()
        } else {
            // Create a new meet actor if one doesn't exist
            let storage = self.state.storage.clone();
            let handle = crate::meet_actor::spawn_meet_actor(meet_id, storage).await;
            self.state
                .meet_handles
                .insert(meet_id.to_string(), handle.clone());
            handle
        };

        // Process the recovery updates
        let (new_seq, updates_recovered) = match meet_handle
            .recover_state(self.client_id.clone(), priority, updates)
            .await
        {
            Ok(result) => result,
            Err(e) => {
                return Ok(ServerMessage::Error {
                    code: "RECOVERY_ERROR".to_string(),
                    message: e.to_string(),
                });
            },
        };

        // Notify the client that recovery is complete
        Ok(ServerMessage::StateRecovered {
            meet_id: meet_id.to_string(),
            new_seq_num: new_seq,
            updates_recovered,
        })
    }

    /// # Handle incoming client messages
    /// This is the main entry point for processing incoming WebSocket messages from clients.
    /// It routes different message types to appropriate handlers and implements automatic
    /// reconnection logic when sessions are invalid.
    ///
    /// # Message Types
    /// The handler supports the following client message types:
    /// - `CreateMeet`: Initialize a new meet and create a session
    /// - `JoinMeet`: Join an existing meet and create a session
    /// - `UpdateInit`: Send updates to the server and broadcast to other clients
    /// - `ClientPull`: Request updates from the server since a specific sequence number
    /// - `PublishMeet`: Publish meet results and generate CSV output
    /// - `StateRecoveryResponse`: Handle state recovery responses
    ///
    /// # Network Resilience
    /// If a message arrives with an invalid session token (e.g., after a network
    /// interruption), the handler will attempt to reconnect automatically using
    /// the `try_reconnect` method with exponential backoff.
    ///
    /// # State Recovery
    /// If sequence gaps or state inconsistency is detected, the handler will
    /// automatically trigger state recovery by requesting updates from all connected
    /// clients.
    ///
    /// # Priority Handling
    /// Client priority is recorded during meet creation/joining and used for conflict
    /// resolution when updates from multiple clients target the same location.
    ///
    /// # Returns
    /// Returns a `Result` containing the appropriate `ServerMessage` response, which
    /// will be sent back to the client over the WebSocket.
    ///
    /// # Errors
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
                password,
                location_name,
                priority,
            } => {
                // Validate inputs
                let meet_id = match crate::validation::validate_meet_id(&meet_id) {
                    Ok(id) => id,
                    Err(e) => {
                        return Ok(ServerMessage::Error {
                            code: "INVALID_MEET_ID".to_string(),
                            message: e.to_string(),
                        });
                    },
                };

                // Check meet ID uniqueness
                if !crate::validation::is_meet_id_unique(meet_id) {
                    return Ok(ServerMessage::Error {
                        code: "MEET_ID_EXISTS".to_string(),
                        message: "Meet ID already exists".to_string(),
                    });
                }

                // Validate password
                match crate::validation::validate_password(&password) {
                    Ok(_) => {},
                    Err(e) => {
                        return Ok(ServerMessage::Error {
                            code: "INVALID_PASSWORD".to_string(),
                            message: e.to_string(),
                        });
                    },
                }

                // Validate location name
                let location_name = match crate::validation::validate_location_name(&location_name)
                {
                    Ok(name) => name.to_string(),
                    Err(e) => {
                        return Ok(ServerMessage::Error {
                            code: "INVALID_LOCATION".to_string(),
                            message: e.to_string(),
                        });
                    },
                };

                // Check auth rate limit
                if let Some(ip) = self.client_ip {
                    if let Some(auth) = self
                        .state
                        .auth
                        .as_any()
                        .downcast_ref::<crate::auth::DefaultAuth>()
                    {
                        if auth.check_auth_rate_limit(ip).is_err() {
                            println!("Auth rate limit exceeded for IP {ip}");
                            return Ok(ServerMessage::Error {
                                code: "AUTH_RATE_LIMITED".to_string(),
                                message:
                                    "Too many authentication attempts. Please try again later."
                                        .to_string(),
                            });
                        }

                        // Record success
                        auth.record_success(ip);
                    }
                }

                // Register the meet ID as used
                crate::validation::register_meet_id(meet_id);

                // Set client priority
                self.set_priority(priority);

                // Handle meet creation
                let session = self
                    .state
                    .auth
                    .new_session(meet_id.to_string(), location_name, priority)
                    .await;

                // Return create response
                Ok(ServerMessage::MeetCreated {
                    meet_id: meet_id.to_string(),
                    session_token: session,
                })
            },
            ClientMessage::JoinMeet {
                meet_id,
                password,
                location_name,
                priority,
            } => {
                // Validate inputs
                let meet_id = match crate::validation::validate_meet_id(&meet_id) {
                    Ok(id) => id,
                    Err(e) => {
                        return Ok(ServerMessage::Error {
                            code: "INVALID_MEET_ID".to_string(),
                            message: e.to_string(),
                        });
                    },
                };

                // Validate password
                match crate::validation::validate_password(&password) {
                    Ok(_) => {},
                    Err(e) => {
                        return Ok(ServerMessage::Error {
                            code: "INVALID_PASSWORD".to_string(),
                            message: e.to_string(),
                        });
                    },
                }

                // Validate location name
                let location_name = match crate::validation::validate_location_name(&location_name)
                {
                    Ok(name) => name.to_string(),
                    Err(e) => {
                        return Ok(ServerMessage::Error {
                            code: "INVALID_LOCATION".to_string(),
                            message: e.to_string(),
                        });
                    },
                };

                // Check auth rate limit
                if let Some(ip) = self.client_ip {
                    if let Some(auth) = self
                        .state
                        .auth
                        .as_any()
                        .downcast_ref::<crate::auth::DefaultAuth>()
                    {
                        if auth.check_auth_rate_limit(ip).is_err() {
                            println!("Auth rate limit exceeded for IP {ip}");
                            return Ok(ServerMessage::Error {
                                code: "AUTH_RATE_LIMITED".to_string(),
                                message:
                                    "Too many authentication attempts. Please try again later."
                                        .to_string(),
                            });
                        }

                        // Record success
                        auth.record_success(ip);
                    }
                }

                // Set client priority
                self.set_priority(priority);

                // Check if the meet exists and the password is correct
                // In a real implementation, this would verify against stored data

                // For now, always accept the join request
                let session = self
                    .state
                    .auth
                    .new_session(meet_id.to_string(), location_name, priority)
                    .await;

                // Return join response
                Ok(ServerMessage::MeetJoined {
                    meet_id: meet_id.to_string(),
                    session_token: session,
                })
            },
            ClientMessage::UpdateInit {
                meet_id,
                session_token,
                updates,
            } => {
                // Validate meet ID
                let meet_id = match crate::validation::validate_meet_id(&meet_id) {
                    Ok(id) => id.to_string(),
                    Err(e) => {
                        return Ok(ServerMessage::Error {
                            code: "INVALID_MEET_ID".to_string(),
                            message: e.to_string(),
                        });
                    },
                };

                // First check if session is valid to catch InvalidSession before validation errors
                if !self.state.auth.validate_session(&session_token).await {
                    // If failed login, record it
                    if let Some(ip) = self.client_ip {
                        if let Some(auth) = self
                            .state
                            .auth
                            .as_any()
                            .downcast_ref::<crate::auth::DefaultAuth>()
                        {
                            auth.record_failed_attempt(ip);
                        }
                    }

                    // Session is invalid, try to reconnect
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
                            return Ok(ServerMessage::InvalidSession { session_token });
                        },
                        Err(_) => {
                            // Return error if session is invalid
                            return Ok(ServerMessage::InvalidSession { session_token });
                        },
                    }
                }

                // Validate session token
                match crate::validation::validate_session_token(&session_token) {
                    Ok(_) => {},
                    Err(e) => {
                        return Ok(ServerMessage::Error {
                            code: "INVALID_SESSION_TOKEN".to_string(),
                            message: e.to_string(),
                        });
                    },
                }

                // Validate each update
                let mut valid_updates = Vec::new();
                let mut rejected_updates = Vec::new();

                for update in updates {
                    // Basic validation of location
                    if update.location.is_empty() {
                        rejected_updates.push((
                            update.location.clone(),
                            "Update location cannot be empty".to_string(),
                        ));
                        continue;
                    }

                    // Basic validation of JSON structure in value
                    if let Err(err) = serde_json::from_str::<serde_json::Value>(&update.value) {
                        rejected_updates.push((
                            update.location.clone(),
                            format!("Invalid JSON in update value: {err}"),
                        ));
                        continue;
                    }

                    // If all checks pass, keep the update
                    valid_updates.push(update);
                }

                // If any updates were rejected, return early with rejection info
                if !rejected_updates.is_empty() {
                    return Ok(ServerMessage::UpdateRejected {
                        meet_id,
                        updates_rejected: rejected_updates,
                    });
                }

                // Get session to retrieve priority
                if let Some(session) = self.state.auth.get_session(&session_token).await {
                    // Update client priority from session
                    self.set_priority(session.priority);

                    // Get handle to the meet actor using if let instead of unwrap
                    let meet_handle = if let Some(handle) = self.state.meet_handles.get(&meet_id) {
                        handle.clone()
                    } else {
                        // Create a new meet actor if one doesn't exist
                        let storage = self.state.storage.clone();
                        let handle = crate::meet_actor::spawn_meet_actor(&meet_id, storage).await;
                        self.state
                            .meet_handles
                            .insert(meet_id.clone(), handle.clone());
                        handle
                    };

                    // Create openlifter_common::Update from our messages::Update
                    let ol_updates = valid_updates
                        .iter()
                        .map(|u| openlifter_common::Update {
                            update_key: u.location.clone(),
                            update_value: serde_json::from_str(&u.value)
                                .unwrap_or(serde_json::Value::Null),
                            #[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
                            local_seq_num: u.timestamp as u64, // Use timestamp as sequence number
                            after_server_seq_num: 0, // Default to 0
                        })
                        .collect();

                    match meet_handle
                        .apply_updates(self.client_id.clone(), session.priority, ol_updates)
                        .await
                    {
                        Ok(update_acks) => {
                            // Register client for this meet if not already
                            if let Some(tx) = &self.client_tx {
                                self.register_client(&meet_id, tx.clone());
                            }

                            // Convert to a format expected by UpdateAck
                            let update_ids =
                                update_acks.iter().map(|(id, _)| id.to_string()).collect();

                            // Return response with server-assigned sequence numbers
                            Ok(ServerMessage::UpdateAck {
                                meet_id,
                                update_ids,
                            })
                        },
                        Err(e) => {
                            if let crate::error::AppError::NeedsRecovery {
                                meet_id,
                                last_known_seq,
                            } = e
                            {
                                // Automatically initiate state recovery
                                println!(
                                    "State recovery needed for meet {meet_id}: last_known_seq={last_known_seq}"
                                );

                                // Initiate state recovery
                                match self.initiate_state_recovery(&meet_id, last_known_seq).await {
                                    Ok(()) => Ok(ServerMessage::StateRecoveryRequest {
                                        meet_id,
                                        last_known_seq,
                                    }),
                                    Err(e) => Ok(ServerMessage::Error {
                                        code: "RECOVERY_ERROR".to_string(),
                                        message: e.to_string(),
                                    }),
                                }
                            } else {
                                // Create a list of rejected updates
                                let updates_rejected = vec![("all".to_string(), e.to_string())];
                                Ok(ServerMessage::UpdateRejected {
                                    meet_id,
                                    updates_rejected,
                                })
                            }
                        },
                    }
                } else {
                    // Session not found but token was valid (should not happen)
                    Ok(ServerMessage::Error {
                        code: "SESSION_ERROR".to_string(),
                        message: "Session token is valid but session not found".to_string(),
                    })
                }
            },
            ClientMessage::ClientPull {
                meet_id,
                session_token,
                last_server_seq,
            } => {
                // Validate meet ID
                let meet_id = match crate::validation::validate_meet_id(&meet_id) {
                    Ok(id) => id.to_string(),
                    Err(e) => {
                        return Ok(ServerMessage::Error {
                            code: "INVALID_MEET_ID".to_string(),
                            message: e.to_string(),
                        });
                    },
                };

                // Validate session token
                match crate::validation::validate_session_token(&session_token) {
                    Ok(_) => {},
                    Err(e) => {
                        return Ok(ServerMessage::Error {
                            code: "INVALID_SESSION_TOKEN".to_string(),
                            message: e.to_string(),
                        });
                    },
                }

                if self.state.auth.validate_session(&session_token).await {
                    // Get session to retrieve priority
                    if let Some(_session) = self.state.auth.get_session(&session_token).await {
                        // Get handle to the meet actor
                        let updates = if let Some(handle) = self.state.meet_handles.get(&meet_id) {
                            // Get updates since last_server_seq
                            match handle.get_updates_since(last_server_seq).await {
                                Ok(updates) => {
                                    // Convert UpdateWithServerSeq to UpdateWithMetadata
                                    let updates_with_metadata: Vec<UpdateWithMetadata> = updates
                                        .iter()
                                        .map(|u| {
                                            let update = Update {
                                                location: u.update.update_key.clone(),
                                                value: u.update.update_value.to_string(),
                                                #[allow(
                                                    clippy::cast_possible_wrap,
                                                    clippy::cast_sign_loss
                                                )]
                                                timestamp: u.update.local_seq_num as i64,
                                            };
                                            UpdateWithMetadata {
                                                update,
                                                source_client: u.source_client_id.clone(),
                                                server_seq: u.server_seq_num,
                                                priority: u.source_client_priority,
                                            }
                                        })
                                        .collect();
                                    updates_with_metadata
                                },
                                Err(e) => {
                                    return Ok(ServerMessage::Error {
                                        code: "PULL_ERROR".to_string(),
                                        message: e.to_string(),
                                    });
                                },
                            }
                        } else {
                            // Meet does not exist yet (no updates)
                            Vec::new()
                        };

                        // Register client for this meet if not already
                        if let Some(tx) = &self.client_tx {
                            self.register_client(&meet_id, tx.clone());
                        }

                        // Return updates
                        Ok(ServerMessage::ServerPull {
                            meet_id,
                            last_server_seq,
                            updates_relayed: updates,
                        })
                    } else {
                        // Session not found but token was valid (should not happen)
                        Ok(ServerMessage::Error {
                            code: "SESSION_ERROR".to_string(),
                            message: "Session token is valid but session not found".to_string(),
                        })
                    }
                } else {
                    // If failed login, record it
                    if let Some(ip) = self.client_ip {
                        if let Some(auth) = self
                            .state
                            .auth
                            .as_any()
                            .downcast_ref::<crate::auth::DefaultAuth>()
                        {
                            auth.record_failed_attempt(ip);
                        }
                    }

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
                // Validate meet ID
                let meet_id = match crate::validation::validate_meet_id(&meet_id) {
                    Ok(id) => id.to_string(),
                    Err(e) => {
                        return Ok(ServerMessage::Error {
                            code: "INVALID_MEET_ID".to_string(),
                            message: e.to_string(),
                        });
                    },
                };

                // Validate session token
                match crate::validation::validate_session_token(&session_token) {
                    Ok(_) => {},
                    Err(e) => {
                        return Ok(ServerMessage::Error {
                            code: "INVALID_SESSION_TOKEN".to_string(),
                            message: e.to_string(),
                        });
                    },
                }

                // Validate email
                match crate::validation::validate_email(&return_email) {
                    Ok(_) => {},
                    Err(e) => {
                        return Ok(ServerMessage::Error {
                            code: "INVALID_EMAIL".to_string(),
                            message: e.to_string(),
                        });
                    },
                }

                // Sanitize the CSV content
                let sanitized_csv = crate::validation::sanitize_string(&opl_csv);

                if self.state.auth.validate_session(&session_token).await {
                    // TODO: Implement meet publishing
                    println!(
                        "Publishing meet {meet_id} with return email {return_email} (CSV length: {})",
                        sanitized_csv.len()
                    );

                    // Ideally, this would store the meet in a published state
                    // and send the CSV data to OpenPowerlifting

                    // Return success response
                    Ok(ServerMessage::PublishAck { meet_id })
                } else {
                    // Return error if session is invalid
                    Ok(ServerMessage::InvalidSession { session_token })
                }
            },
            ClientMessage::StateRecoveryResponse {
                meet_id,
                session_token,
                last_seq_num,
                updates,
                priority,
            } => {
                // Validate meet ID
                let meet_id = match crate::validation::validate_meet_id(&meet_id) {
                    Ok(id) => id.to_string(),
                    Err(e) => {
                        return Ok(ServerMessage::Error {
                            code: "INVALID_MEET_ID".to_string(),
                            message: e.to_string(),
                        });
                    },
                };

                // Validate session token
                match crate::validation::validate_session_token(&session_token) {
                    Ok(_) => {},
                    Err(e) => {
                        return Ok(ServerMessage::Error {
                            code: "INVALID_SESSION_TOKEN".to_string(),
                            message: e.to_string(),
                        });
                    },
                }

                // Validate updates (similar to UpdateInit)
                let mut valid_updates = Vec::new();

                for update in updates {
                    // Basic validation of location
                    if update.location.is_empty() {
                        continue;
                    }

                    // Basic validation of JSON structure in value
                    if serde_json::from_str::<serde_json::Value>(&update.value).is_err() {
                        continue;
                    }

                    // If all checks pass, keep the update
                    valid_updates.push(update);
                }

                // Process state recovery response
                self.handle_state_recovery_response(
                    &meet_id,
                    &session_token,
                    last_seq_num,
                    valid_updates,
                    priority,
                )
                .await
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

    #[allow(clippy::too_many_lines)]
    #[tokio::test]
    async fn test_handle_state_recovery_response() {
        let (mut handler, _state, _temp_dir) = setup();

        // Create a meet first
        let create_result = handler
            .handle_message(ClientMessage::CreateMeet {
                meet_id: "recovery-test".to_string(),
                password: "Password123!".to_string(),
                location_name: "Recovery Test".to_string(),
                priority: 5,
            })
            .await
            .unwrap();

        // Extract session token using let...else pattern
        let ServerMessage::MeetCreated {
            meet_id: _,
            session_token,
        } = create_result
        else {
            panic!("Expected MeetCreated response")
        };

        // Create some initial updates
        let initial_updates = vec![
            Update {
                location: "test.item1".to_string(),
                value: r#"{"name":"Item 1","value":123}"#.to_string(),
                timestamp: 12345,
            },
            Update {
                location: "test.item2".to_string(),
                value: r#"{"name":"Item 2","value":456}"#.to_string(),
                timestamp: 12346,
            },
        ];

        // Send recovery response
        let recovery_result = handler
            .handle_message(ClientMessage::StateRecoveryResponse {
                meet_id: "recovery-test".to_string(),
                session_token: session_token.clone(),
                last_seq_num: 0,
                updates: initial_updates,
                priority: 5,
            })
            .await
            .unwrap();

        // Verify the result
        match recovery_result {
            ServerMessage::StateRecovered {
                meet_id,
                new_seq_num,
                updates_recovered,
            } => {
                assert_eq!(meet_id, "recovery-test");
                assert_eq!(new_seq_num, 2); // Two updates should have been processed
                assert_eq!(updates_recovered, 2);
            },
            _ => panic!("Expected StateRecovered response"),
        }

        // Now test with conflicting updates
        let conflicting_updates = vec![
            // This should be accepted as it's a new key
            Update {
                location: "test.item3".to_string(),
                value: r#"{"name":"Item 3","value":789}"#.to_string(),
                timestamp: 12347,
            },
            // This should be rejected as it's an existing key with same priority (5)
            Update {
                location: "test.item1".to_string(),
                value: r#"{"name":"Item 1 Updated","value":999}"#.to_string(),
                timestamp: 12348,
            },
        ];

        // Send second recovery response
        let second_recovery_result = handler
            .handle_message(ClientMessage::StateRecoveryResponse {
                meet_id: "recovery-test".to_string(),
                session_token: session_token.clone(),
                last_seq_num: 2,
                updates: conflicting_updates,
                priority: 5, // Same priority, so conflict should be ignored
            })
            .await
            .unwrap();

        // Verify the result
        match second_recovery_result {
            ServerMessage::StateRecovered {
                meet_id,
                new_seq_num,
                updates_recovered,
            } => {
                assert_eq!(meet_id, "recovery-test");
                assert_eq!(new_seq_num, 3); // Only one new update should have been processed
                assert_eq!(updates_recovered, 1);
            },
            _ => panic!("Expected StateRecovered response"),
        }

        // Now test with higher priority updates
        let higher_priority_updates = vec![
            // This should be accepted as it's a higher priority
            Update {
                location: "test.item1".to_string(),
                value: r#"{"name":"Item 1 Override","value":1000}"#.to_string(),
                timestamp: 12349,
            },
        ];

        // Send third recovery response with higher priority
        let third_recovery_result = handler
            .handle_message(ClientMessage::StateRecoveryResponse {
                meet_id: "recovery-test".to_string(),
                session_token,
                last_seq_num: 3,
                updates: higher_priority_updates,
                priority: 10, // Higher priority, so conflict should be accepted
            })
            .await
            .unwrap();

        // Verify the result
        match third_recovery_result {
            ServerMessage::StateRecovered {
                meet_id,
                new_seq_num: _,
                updates_recovered,
            } => {
                assert_eq!(meet_id, "recovery-test");
                assert_eq!(updates_recovered, 1); // The override should be accepted
            },
            _ => panic!("Expected StateRecovered response"),
        }
    }
}
