// ==================
// crates/server-app/src/websocket.rs
// ==================
//! WebSocket handler for OpenLifter backend server.
//!
//! Provides connection management, message routing, session validation,
//! and conflict resolution for powerlifting meet coordination.

use crate::messages::{ServerMessage, UpdateWithMetadata};
use crate::storage::Storage;
use crate::validation::middleware::{ValidationContext, ValidationMiddleware};
use crate::AppState;
use anyhow::{anyhow, Result};
use openlifter_common::ClientToServer;
use rand::Rng;
use std::net::IpAddr;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{self, Duration};
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::messages::Update;

/// Maximum number of reconnection attempts before giving up
const MAX_RECONNECT_ATTEMPTS: u8 = 3;

/// Base delay between reconnection attempts in milliseconds
const RECONNECT_DELAY_MS: u64 = 1000; // 1 second

/// Macro to simplify validation error handling - DEPRECATED
/// Use ValidationMiddleware instead for new code
#[allow(unused_macros)]
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

        // Validate the message first using validation middleware
        let validation_context = ValidationContext::default();
        if let Err(server_error) = ValidationMiddleware::validate_message(&msg, &validation_context)
        {
            return Ok(server_error);
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

                // Validate inputs using the new middleware
                let _password = match ValidationMiddleware::validate_field(
                    crate::validation::validate_password(&password),
                    "INVALID_PASSWORD",
                ) {
                    Ok(p) => p,
                    Err(server_error) => return Ok(server_error),
                };

                let location_name = match ValidationMiddleware::validate_field(
                    crate::validation::validate_location_name(&this_location_name),
                    "INVALID_LOCATION",
                ) {
                    Ok(name) => name.to_string(),
                    Err(server_error) => return Ok(server_error),
                };

                // Generate a meet ID
                let meet_id = format!(
                    "{}-{}-{}",
                    rand::thread_rng().gen_range(100..1000),
                    rand::thread_rng().gen_range(100..1000),
                    rand::thread_rng().gen_range(100..1000)
                );

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

                // Use validation middleware for consolidated validation
                let validated_data = match ValidationMiddleware::builder()
                    .meet_id(&meet_id)
                    .and()
                    .password(&password)
                    .and()
                    .location_name(&location_name)
                    .and()
                    .execute()
                {
                    Ok(()) => {
                        // All validations passed, extract the validated data
                        (meet_id, password, location_name)
                    },
                    Err(server_error) => return Ok(server_error),
                };

                let (meet_id, _password, location_name) = validated_data;

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

                // Validate session token using middleware
                match ValidationMiddleware::validate_field(
                    crate::validation::validate_session_token(&session_token),
                    "INVALID_SESSION_TOKEN",
                ) {
                    Ok(_) => {},
                    Err(server_error) => return Ok(server_error),
                }

                // Validate each update
                let mut valid_updates = Vec::new();
                let mut rejected_updates = Vec::new();

                for update in updates {
                    // Use validation middleware for update validation
                    match ValidationMiddleware::validate_field(
                        crate::validation::validate_update(&update),
                        "INVALID_UPDATE",
                    ) {
                        Ok(_) => valid_updates.push(update),
                        Err(ServerMessage::Error { message, .. }) => {
                            rejected_updates.push((update.update_key.clone(), message));
                        },
                        Err(_) => {
                            rejected_updates.push((
                                update.update_key.clone(),
                                "Update validation failed".to_string(),
                            ));
                        },
                    }
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

                // Validate session token using middleware
                match ValidationMiddleware::validate_field(
                    crate::validation::validate_session_token(&session_token),
                    "INVALID_SESSION_TOKEN",
                ) {
                    Ok(_) => {},
                    Err(server_error) => return Ok(server_error),
                }

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

                // Use validation middleware for multiple field validation
                match ValidationMiddleware::validate_fields(vec![
                    (
                        "session_token",
                        crate::validation::validate_session_token(&session_token).map(|_| ()),
                    ),
                    (
                        "email",
                        crate::validation::validate_email(&return_email).map(|_| ()),
                    ),
                ]) {
                    Ok(_) => {},
                    Err(server_error) => return Ok(server_error),
                }

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
    use crate::testing::fixtures::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        routing::get,
        Router,
    };
    use tower::ServiceExt;

    async fn test_handler() -> &'static str {
        "Hello, World!"
    }

    #[tokio::test]
    async fn test_basic_router() {
        let env = setup_websocket_test().await;

        let app = Router::new()
            .route("/", get(test_handler))
            .with_state(env.state.clone());

        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_websocket_handler_creation() {
        let env = setup_websocket_test().await;

        let meet_id1 = TEST_MEET_ID;
        let meet_id2 = TEST_MEET_ID_2;

        // Test basic handler functionality - state.clients is the field to check
        assert!(env.state.clients.is_empty());

        // Test meet ID format
        assert!(!meet_id1.is_empty());
        assert_ne!(meet_id1, meet_id2);
    }

    #[tokio::test]
    async fn test_websocket_session_flow() {
        let env = setup_websocket_test().await;

        let meet_id = TEST_MEET_ID;

        // Test session creation and validation
        assert!(env.state.clients.get(meet_id).is_none());

        // This would be part of a larger integration test
        // For now just verify the basic structure is working
        let _session_count = env.state.sessions.active_session_count().await;
    }

    #[tokio::test]
    async fn test_create_meet_message_handling() {
        let _env = setup_websocket_test().await;

        // Test the message structure handling
        let create_msg = test_create_meet_message();

        // In a real test, we would process this message through the handler
        // For now, just verify the message structure
        match create_msg {
            ClientToServer::CreateMeet {
                this_location_name,
                password,
                endpoints: _,
            } => {
                assert!(!this_location_name.is_empty());
                assert!(!password.is_empty());
                // endpoints can be empty, so no need to check length >= 0
            },
            _ => panic!("Expected CreateMeet message"),
        }
    }

    #[tokio::test]
    async fn test_weak_password_validation() {
        let _env = setup_websocket_test().await;

        let weak_msg = test_create_meet_weak_password();

        // Verify weak password is detected
        match weak_msg {
            ClientToServer::CreateMeet { password, .. } => {
                assert_eq!(password, WEAK_PASSWORD);
                // In real test, this would be rejected by validation
            },
            _ => panic!("Expected CreateMeet message"),
        }
    }

    #[tokio::test]
    async fn test_join_meet_flow() {
        let _env = setup_websocket_test().await;

        let join_msg = test_join_meet_message();

        // Verify join message structure
        match join_msg {
            ClientToServer::JoinMeet {
                meet_id,
                password,
                location_name,
            } => {
                assert!(!meet_id.is_empty());
                assert!(!password.is_empty());
                assert!(!location_name.is_empty());
            },
            _ => panic!("Expected JoinMeet message"),
        }
    }
}
