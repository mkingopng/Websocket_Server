use crate::messages::{ClientMessage, ServerMessage, Update, UpdateWithMetadata};
use crate::storage::Storage;
use crate::AppState;
use anyhow::{anyhow, Result};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{self, Duration};
use uuid::Uuid;

const MAX_RECONNECT_ATTEMPTS: u8 = 5;
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

            // Send to all connected clients except ourselves
            let mut failed_clients = 0;
            for client in clients.iter() {
                if self
                    .client_tx
                    .as_ref()
                    .is_none_or(|tx| !std::ptr::eq(tx, client))
                {
                    if let Err(e) = self.try_send_with_retry(client, relay_msg.clone()).await {
                        println!("Failed to relay update to client: {e}");
                        failed_clients += 1;
                    }
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

    // Handle incoming client messages
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
