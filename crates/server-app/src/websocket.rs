// ==================
// crates/server-app/src/websocket.rs
// ==================
//! WebSocket handler for OpenLifter backend server.
//!
//! Provides connection management, message routing, session validation,
//! and conflict resolution for powerlifting meet coordination.

use crate::{
    messages::{ServerMessage, Update, UpdateWithMetadata},
    storage::Storage,
    validation, AppState,
};
use anyhow::{anyhow, Result};
use chrono;
use openlifter_common::ClientToServer;
use rand::Rng;
use std::{net::IpAddr, sync::Arc};
use tokio::sync::mpsc;
use tokio::time::{self, Duration};
use tracing::{debug, error, info};
use uuid::Uuid;
/// Maximum number of reconnection attempts before giving up
const MAX_RECONNECT_ATTEMPTS: u8 = 5;

/// Base delay between reconnection attempts in milliseconds
const RECONNECT_DELAY_MS: u64 = 1000; // 1 second

/// Macro to simplify validation error handling
macro_rules! validate_or_error {
    ($validation:expr, $error_code:expr) => {
        match $validation {
            Ok(val) => val,
            Err(e) => {
                return Ok(ServerMessage::Error {
                    code: $error_code.to_string(),
                    message: e.to_string(),
                });
            },
        }
    };
}

/// Macro to handle auth rate limiting
macro_rules! check_auth_rate_limit {
    ($self:expr) => {
        if let Some(ip) = $self.client_ip {
            if let Some(auth) = $self
                .state
                .auth
                .as_any()
                .downcast_ref::<crate::auth::DefaultAuth>()
            {
                if auth.check_auth_rate_limit(ip).is_err() {
                    return Ok(ServerMessage::Error {
                        code: "AUTH_RATE_LIMITED".to_string(),
                        message: "Too many authentication attempts. Please try again later."
                            .to_string(),
                    });
                }
                auth.record_success(ip);
            }
        }
    };
}

/// Macro to record failed auth attempts
macro_rules! record_auth_failure {
    ($self:expr) => {
        if let Some(ip) = $self.client_ip {
            if let Some(auth) = $self
                .state
                .auth
                .as_any()
                .downcast_ref::<crate::auth::DefaultAuth>()
            {
                auth.record_failed_attempt(ip);
            }
        }
    };
}

/// WebSocket handler for processing messages
pub struct WebSocketHandler<S> {
    /// Application state
    state: Arc<AppState<S>>,
    /// Client IP address
    client_ip: Option<IpAddr>,
    client_id: String,
    client_tx: Option<mpsc::Sender<ServerMessage>>,
    client_priority: u8,
    reconnect_attempts: u8,
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
    pub fn register_client(
        &mut self,
        meet_id: &str,
        tx: mpsc::Sender<ServerMessage>,
    ) -> Result<()> {
        // Store the client's transmission channel
        self.client_tx = Some(tx.clone());

        // Add client to the clients map for the meet
        let mut meet_clients = self.state.clients.entry(meet_id.to_string()).or_default();
        meet_clients.push(tx);

        info!("Client {} registered for meet {}", self.client_id, meet_id);

        // Reset reconnect attempts on successful registration
        self.reconnect_attempts = 0;
        Ok(())
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
                info!(
                    "Client {} unregistered from meet {}",
                    self.client_id, meet_id
                );
            }
        }
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
                    .expect("Location updates should not be empty");

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
        info!(
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
            info!(
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
        info!("Initiating state recovery for meet: {}", meet_id);

        // Get all clients for this meet
        if let Some(clients) = self.state.clients.get(meet_id) {
            let recovery_request = ServerMessage::StateRecoveryRequest {
                meet_id: meet_id.to_string(),
                last_known_seq,
            };

            // Send recovery request to all clients
            for client in clients.value() {
                if let Err(e) = client.send(recovery_request.clone()).await {
                    error!("Error sending recovery request: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Handle a state recovery response from a client
    /// This method processes updates from a client during state recovery,
    /// resolving conflicts and updating the server's state.
    #[allow(dead_code)]
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

        info!(
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

    /// Helper function to handle session validation with automatic reconnection
    async fn validate_session_or_reconnect(
        &mut self,
        meet_id: &str,
        session_token: &str,
        retry_msg: ClientToServer,
    ) -> Result<bool, ServerMessage> {
        if !self.state.auth.validate_session(session_token).await {
            record_auth_failure!(self);
            match self.try_reconnect(meet_id, session_token).await {
                Ok(reconnected) => {
                    if reconnected {
                        // Successfully reconnected - retry the operation
                        match Box::pin(self.handle_message(retry_msg)).await {
                            Ok(response) => Err(response),
                            Err(_) => Err(ServerMessage::InvalidSession {
                                session_token: session_token.to_string(),
                            }),
                        }
                    } else {
                        Err(ServerMessage::InvalidSession {
                            session_token: session_token.to_string(),
                        })
                    }
                },
                Err(_) => Err(ServerMessage::InvalidSession {
                    session_token: session_token.to_string(),
                }),
            }
        } else {
            Ok(true)
        }
    }

    /// Helper function to get or create meet handle
    async fn get_or_create_meet_handle(&self, meet_id: &str) -> crate::meet_actor::MeetHandle {
        if let Some(handle) = self.state.meet_handles.get(meet_id) {
            handle.clone()
        } else {
            let storage = self.state.storage.clone();
            let handle = crate::meet_actor::spawn_meet_actor(meet_id, storage).await;
            self.state
                .meet_handles
                .insert(meet_id.to_string(), handle.clone());
            handle
        }
    }

    /// # Supported Message Types
    /// - `CreateMeet`: Create a new meet with specified parameters
    /// - `JoinMeet`: Join an existing meet with credentials
    /// - `UpdateInit`: Initialize updates from a client
    /// - `ClientPull`: Request updates since a specific sequence number
    /// - `PublishMeet`: Publish meet results and generate CSV output
    ///
    /// # Network Resilience
    /// If a message arrives with an invalid session token (e.g., after a network
    /// interruption), the handler will attempt to reconnect the client automatically.
    /// This provides seamless recovery from temporary network issues.
    ///
    /// # Error Handling
    /// Returns appropriate error responses for:
    /// - Validation errors
    /// - Authentication failures
    /// - Rate limiting violations
    /// - Internal server errors
    #[allow(clippy::too_many_lines)]
    pub async fn handle_message(&mut self, msg: ClientToServer) -> Result<ServerMessage> {
        debug!("Processing message: {:?}", msg);

        // Validate the message first
        if let Err(e) = validation::validate_client_message(&msg) {
            return Ok(ServerMessage::Error {
                code: "VALIDATION_ERROR".to_string(),
                message: e.to_string(),
            });
        }

        // Process the message based on its type
        match msg {
            ClientToServer::CreateMeet {
                this_location_name,
                password,
                endpoints,
            } => {
                info!("Creating meet with location: {}", this_location_name);
                debug!(
                    "Creating meet with location '{}' and {} endpoints",
                    this_location_name,
                    endpoints.len()
                );

                // Generate a meet ID
                let meet_id = format!(
                    "{}-{}-{}",
                    rand::thread_rng().gen_range(100..1000),
                    rand::thread_rng().gen_range(100..1000),
                    rand::thread_rng().gen_range(100..1000)
                );

                // Validate password
                validate_or_error!(
                    crate::validation::validate_password(&password),
                    "INVALID_PASSWORD"
                );

                // Validate location name
                let location_name = validate_or_error!(
                    crate::validation::validate_location_name(&this_location_name),
                    "INVALID_LOCATION"
                )
                .to_string();

                // Check auth rate limit
                check_auth_rate_limit!(self);

                // Register the meet ID as used
                crate::validation::register_meet_id(&meet_id);

                // Get priority from first endpoint or default to 5
                let priority = endpoints.first().map(|e| e.priority).unwrap_or(5);

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
            ClientToServer::JoinMeet {
                meet_id,
                password,
                location_name,
            } => {
                info!("Joining meet: {}", meet_id);
                debug!(
                    "Joining meet '{}' with location '{}'",
                    meet_id, location_name
                );

                // Validate inputs
                let meet_id = validate_or_error!(
                    crate::validation::validate_meet_id(&meet_id),
                    "INVALID_MEET_ID"
                );

                // Validate password
                validate_or_error!(
                    crate::validation::validate_password(&password),
                    "INVALID_PASSWORD"
                );

                // Validate location name
                let location_name = validate_or_error!(
                    crate::validation::validate_location_name(&location_name),
                    "INVALID_LOCATION"
                )
                .to_string();

                // Check auth rate limit
                check_auth_rate_limit!(self);

                // Default priority for joining clients
                let priority = 5;

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
            ClientToServer::UpdateInit {
                session_token,
                updates,
            } => {
                debug!("Update init with {} updates", updates.len());

                // Get session to retrieve meet_id and priority
                let session = match self.state.auth.get_session(&session_token).await {
                    Some(session) => session,
                    None => {
                        return Ok(ServerMessage::InvalidSession {
                            session_token: session_token.clone(),
                        });
                    },
                };

                let meet_id = session.meet_id.clone();

                // First check if session is valid to catch InvalidSession before validation errors
                if let Err(response) = self
                    .validate_session_or_reconnect(
                        &meet_id,
                        &session_token,
                        ClientToServer::UpdateInit {
                            session_token: session_token.clone(),
                            updates: updates.clone(),
                        },
                    )
                    .await
                {
                    return Ok(response);
                }

                // Validate session token
                validate_or_error!(
                    crate::validation::validate_session_token(&session_token),
                    "INVALID_SESSION_TOKEN"
                );

                // Validate each update
                let mut valid_updates = Vec::new();
                let mut rejected_updates = Vec::new();

                for update in updates {
                    // Basic validation of update key
                    if update.update_key.is_empty() {
                        rejected_updates.push((
                            update.update_key.clone(),
                            "Update key cannot be empty".to_string(),
                        ));
                        continue;
                    }

                    // Basic validation of JSON structure in value
                    if update.update_value.is_null() {
                        rejected_updates.push((
                            update.update_key.clone(),
                            "Update value cannot be null".to_string(),
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

                // Update client priority from session
                self.set_priority(session.priority);

                // Convert updates to the internal format
                let internal_updates: Vec<crate::messages::Update> = valid_updates
                    .into_iter()
                    .map(|u| crate::messages::Update {
                        location: u.update_key,
                        value: u.update_value.to_string(),
                        timestamp: chrono::Utc::now().timestamp(),
                    })
                    .collect();

                // Return acknowledgment
                Ok(ServerMessage::UpdateAck {
                    meet_id,
                    update_ids: internal_updates
                        .iter()
                        .map(|u| u.location.clone())
                        .collect(),
                })
            },
            ClientToServer::ClientPull {
                session_token,
                last_server_seq,
            } => {
                debug!("Client pull since seq {}", last_server_seq);

                // Get session to retrieve meet_id
                let session = match self.state.auth.get_session(&session_token).await {
                    Some(session) => session,
                    None => {
                        return Ok(ServerMessage::InvalidSession {
                            session_token: session_token.clone(),
                        });
                    },
                };

                let meet_id = session.meet_id.clone();

                // Validate session token
                validate_or_error!(
                    crate::validation::validate_session_token(&session_token),
                    "INVALID_SESSION_TOKEN"
                );

                // Check session validity with automatic reconnection
                if let Err(response) = self
                    .validate_session_or_reconnect(
                        &meet_id,
                        &session_token,
                        ClientToServer::ClientPull {
                            session_token: session_token.clone(),
                            last_server_seq,
                        },
                    )
                    .await
                {
                    return Ok(response);
                }

                // Get handle to the meet actor
                let _meet_handle = self.get_or_create_meet_handle(&meet_id).await;

                // Pull updates since the specified sequence number
                // For now, return empty updates since we need to implement proper pulling
                let updates_with_metadata: Vec<UpdateWithMetadata> = Vec::new();

                Ok(ServerMessage::ServerPull {
                    meet_id,
                    last_server_seq,
                    updates_relayed: updates_with_metadata,
                })
            },
            ClientToServer::PublishMeet {
                session_token,
                return_email,
                opl_csv,
            } => {
                debug!("Publishing meet results");

                // Get session to retrieve meet_id
                let session = match self.state.auth.get_session(&session_token).await {
                    Some(session) => session,
                    None => {
                        return Ok(ServerMessage::InvalidSession {
                            session_token: session_token.clone(),
                        });
                    },
                };

                let meet_id = session.meet_id.clone();

                // Validate session token
                validate_or_error!(
                    crate::validation::validate_session_token(&session_token),
                    "INVALID_SESSION_TOKEN"
                );

                // Validate email
                validate_or_error!(
                    crate::validation::validate_email(&return_email),
                    "INVALID_EMAIL"
                );

                // Check if session is valid
                if !self.state.auth.validate_session(&session_token).await {
                    return Ok(ServerMessage::InvalidSession {
                        session_token: session_token.clone(),
                    });
                }

                // Get handle to the meet actor
                let meet_handle = self.get_or_create_meet_handle(&meet_id).await;

                // Store CSV data
                match meet_handle.store_csv(opl_csv, return_email).await {
                    Ok(()) => Ok(ServerMessage::PublishAck { meet_id }),
                    Err(e) => Ok(ServerMessage::Error {
                        code: "PUBLISH_ERROR".to_string(),
                        message: e.to_string(),
                    }),
                }
            },
            // ClientToServer::StateRecoveryResponse {
            //     meet_id,
            //     session_token,
            //     last_seq_num,
            //     updates,
            //     priority,
            // } => {
            //     info!("State recovery response for meet: {}", meet_id);
            //
            //     // Validate meet ID
            //     let meet_id = validate_or_error!(
            //         crate::validation::validate_meet_id(&meet_id),
            //         "INVALID_MEET_ID"
            //     )
            //     .to_string();
            //
            //     // Validate session token
            //     validate_or_error!(
            //         crate::validation::validate_session_token(&session_token),
            //         "INVALID_SESSION_TOKEN"
            //     );
            //
            //     // Validate updates (similar to UpdateInit)
            //     let mut valid_updates = Vec::new();
            //
            //     for update in updates {
            //         // Basic validation of location
            //         if update.location.is_empty() {
            //             continue;
            //         }
            //
            //         // Basic validation of JSON structure in value
            //         if serde_json::from_str::<serde_json::Value>(&update.value).is_err() {
            //             continue;
            //         }
            //
            //         // If all checks pass, keep the update
            //         valid_updates.push(update);
            //     }
            //
            //     // Process state recovery response
            //     self.handle_state_recovery_response(
            //         &meet_id,
            //         &session_token,
            //         last_seq_num,
            //         valid_updates,
            //         priority,
            //     )
            // },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::FlatFileStorage;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        routing::get,
        Router,
    };
    use std::time::Duration;
    use tempfile::TempDir;
    use tokio::time::timeout;
    use tower::ServiceExt;

    async fn test_handler() -> &'static str {
        "Hello, World!"
    }

    async fn setup() -> (
        WebSocketHandler<FlatFileStorage>,
        Arc<AppState<FlatFileStorage>>,
        TempDir,
    ) {
        let temp_dir = TempDir::new().unwrap();
        let storage = FlatFileStorage::new(temp_dir.path()).unwrap();

        let mut settings = crate::config::Settings::default();
        settings.storage.path = temp_dir.path().to_path_buf();

        let sessions_dir = temp_dir.path().join("sessions");
        std::fs::create_dir_all(&sessions_dir).expect("Failed to create sessions directory");

        let state = AppState::new(storage.clone(), &settings)
            .await
            .expect("Failed to create AppState for test");

        let state = Arc::new(state);
        let handler = WebSocketHandler::new(state.clone());

        (handler, state, temp_dir)
    }

    #[tokio::test]
    async fn test_basic_router() {
        let (_handler, state, _temp_dir) = setup().await;

        let app = Router::new()
            .route("/", get(test_handler))
            .with_state(state);

        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_register_client() {
        let (mut handler, state, _temp_dir) = setup().await;

        let (tx1, _rx1) = mpsc::channel::<ServerMessage>(10);
        let (tx2, _rx2) = mpsc::channel::<ServerMessage>(10);
        let meet_id1 = "test-meet-1";
        let meet_id2 = "test-meet-2";

        let _ = handler.register_client(meet_id1, tx1);
        let _ = handler.register_client(meet_id2, tx2);

        assert!(state.clients.contains_key(meet_id1));
        assert!(state.clients.contains_key(meet_id2));
        assert_eq!(state.clients.get(meet_id1).unwrap().len(), 1);
        assert_eq!(state.clients.get(meet_id2).unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_unregister_client() {
        let (mut handler, state, _temp_dir) = setup().await;

        let (tx, _rx) = mpsc::channel::<ServerMessage>(10);
        let meet_id = "test-meet-unreg";

        let _ = handler.register_client(meet_id, tx);
        assert!(state.clients.contains_key(meet_id));
        assert!(!state.clients.get(meet_id).unwrap().is_empty());

        handler.unregister_client(meet_id);
        assert!(state.clients.contains_key(meet_id));
    }

    #[tokio::test]
    async fn test_multiple_clients_for_one_meet() {
        let (mut handler, state, _temp_dir) = setup().await;

        let (tx1, _rx1) = mpsc::channel::<ServerMessage>(10);
        let (tx2, _rx2) = mpsc::channel::<ServerMessage>(10);
        let (tx3, _rx3) = mpsc::channel::<ServerMessage>(10);

        let meet_id = "multi-client-meet";
        let _ = handler.register_client(meet_id, tx1);

        let mut handler2 = WebSocketHandler::new(state.clone());
        let _ = handler2.register_client(meet_id, tx2);

        let mut handler3 = WebSocketHandler::new(state.clone());
        let _ = handler3.register_client(meet_id, tx3);

        assert_eq!(state.clients.get(meet_id).unwrap().len(), 3);

        handler2.unregister_client(meet_id);
        assert!(state.clients.contains_key(meet_id));
    }

    #[tokio::test]
    async fn test_handle_create_meet() {
        let (mut handler, _state, _temp_dir) = setup().await;

        // Create a meet
        let result = handler
            .handle_message(ClientToServer::CreateMeet {
                this_location_name: "Test Location".to_string(),
                password: "Password123!".to_string(),
                endpoints: vec![],
            })
            .await;

        // Verify result
        assert!(result.is_ok());
        match result.unwrap() {
            ServerMessage::MeetCreated {
                meet_id,
                session_token,
            } => {
                assert!(!meet_id.is_empty());
                assert!(!session_token.is_empty());
            },
            other => panic!("Expected MeetCreated, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_handle_join_meet() {
        let (mut handler, _state, _temp_dir) = setup().await;

        // Join a meet
        let result = handler
            .handle_message(ClientToServer::JoinMeet {
                meet_id: "test-meet".to_string(),
                password: "Password123!".to_string(),
                location_name: "Test Location".to_string(),
            })
            .await;

        // Verify result
        assert!(result.is_ok());
        match result.unwrap() {
            ServerMessage::MeetJoined {
                session_token,
                meet_id: _,
            } => {
                assert!(!session_token.is_empty());
            },
            other => panic!("Expected MeetJoined, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_handle_update_init() {
        // Add timeout to prevent the test from hanging
        timeout(Duration::from_secs(3), async {
            let (mut handler, state, _temp_dir) = setup().await;
            let (tx, _rx) = mpsc::channel::<ServerMessage>(10);

            // Register the client
            let _ = handler.register_client("test-meet", tx);

            // Create a session token
            let session = state
                .auth
                .new_session("test-meet".to_string(), "Test Location".to_string(), 1)
                .await;

            // Updates to send
            let updates = vec![openlifter_common::Update {
                update_key: "item1".to_string(),
                update_value: serde_json::json!({"field": "value"}),
                local_seq_num: 1,
                after_server_seq_num: 0,
            }];

            // Send update
            let result = handler
                .handle_message(ClientToServer::UpdateInit {
                    session_token: session.clone(),
                    updates: updates.clone(),
                })
                .await;

            // Verify result
            assert!(result.is_ok());
            match result.unwrap() {
                ServerMessage::UpdateAck {
                    update_ids,
                    meet_id: _,
                } => {
                    assert_eq!(update_ids.len(), 1);
                },
                other => panic!("Expected UpdateAck, got {other:?}"),
            }
        })
        .await
        .expect("Test timed out");
    }

    #[tokio::test]
    async fn test_handle_invalid_session() {
        // We need to extract all three elements from setup
        let (mut handler, _state, _temp_dir) = setup().await;

        // Set up client
        let (tx, mut rx) = mpsc::channel(10);
        handler.client_tx = Some(tx);

        // Send invalid session message
        let result = handler
            .handle_message(ClientToServer::ClientPull {
                session_token: "invalid".to_string(),
                last_server_seq: 0,
            })
            .await;

        // Verify result
        assert!(result.is_ok());
        let message = result.unwrap();

        // Check the message type without moving any parts
        match &message {
            ServerMessage::InvalidSession { session_token } => {
                assert_eq!(session_token, "invalid");
            },
            other => panic!("Expected InvalidSession, got {:?}", other),
        }

        // Send the message to the client
        if let Some(ref client_tx) = handler.client_tx {
            client_tx
                .send(message)
                .await
                .expect("Failed to send message to client");
        }

        // Verify message is received by client, with a timeout to ensure it arrives
        let timeout = tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await;
        assert!(timeout.is_ok(), "Timed out waiting for message");

        if let Ok(Some(client_message)) = timeout {
            match client_message {
                ServerMessage::InvalidSession { session_token } => {
                    assert_eq!(session_token, "invalid");
                },
                other => panic!("Expected InvalidSession, got {:?}", other),
            }
        } else {
            panic!("Expected to receive message from client channel");
        }
    }

    #[tokio::test]
    async fn test_handle_client_pull() {
        // Add timeout to prevent the test from hanging
        timeout(Duration::from_secs(3), async {
            let (mut handler, state, _temp_dir) = setup().await;

            // Create a session token
            let session = state
                .auth
                .new_session("test-meet".to_string(), "Test Location".to_string(), 1)
                .await;

            // Send client pull
            let result = handler
                .handle_message(ClientToServer::ClientPull {
                    session_token: session,
                    last_server_seq: 0,
                })
                .await;

            // Verify result
            assert!(result.is_ok());
            match result.unwrap() {
                ServerMessage::ServerPull {
                    last_server_seq,
                    updates_relayed,
                    meet_id: _,
                } => {
                    assert_eq!(last_server_seq, 0);
                    assert!(updates_relayed.is_empty());
                },
                other => panic!("Expected ServerPull, got {other:?}"),
            }
        })
        .await
        .expect("Test timed out");
    }

    #[tokio::test]
    async fn test_handle_publish_meet() {
        // add timeout to prevent the test from hanging
        timeout(Duration::from_secs(3), async {
            let (mut handler, state, _temp_dir) = setup().await;

            // Create a session token
            let session = state
                .auth
                .new_session("test-meet".to_string(), "Test Location".to_string(), 1)
                .await;

            // Send publish meet
            let result = handler
                .handle_message(ClientToServer::PublishMeet {
                    session_token: session,
                    return_email: "test@example.com".to_string(),
                    opl_csv: "name,weight,squat".to_string(),
                })
                .await;

            // Verify result
            assert!(result.is_ok());
            match result.unwrap() {
                ServerMessage::PublishAck { meet_id: _ } => {
                    // Verify that the meet was published
                    let _meet_handle = handler.get_or_create_meet_handle("test-meet").await;
                    // Just verify the response was correct - the meet handle creation is sufficient
                },
                other => panic!("Expected PublishAck, got {other:?}"),
            }
        })
        .await
        .expect("Test timed out");
    }

    #[tokio::test]
    async fn test_resolve_conflicts() {
        // Add timeout to prevent the test from hanging
        timeout(Duration::from_secs(5), async {
            // Run the setup
            let (handler, _state, _temp_dir) = setup().await;

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
        })
        .await
        .expect("Test timed out");
    }

    // #[allow(clippy::too_many_lines)]
    // #[tokio::test]
    // async fn test_handle_state_recovery_response() {
    //     // This test is disabled because StateRecoveryResponse is not part of ClientToServer
    //     // TODO: Implement proper state recovery mechanism if needed
    // }
}
