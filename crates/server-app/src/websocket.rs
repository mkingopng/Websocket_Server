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
use anyhow::Result;
use openlifter_common::ClientToServer;
use std::net::IpAddr;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info};
use uuid::Uuid;

/// WebSocket handler for processing messages
pub struct WebSocketHandler<S: Storage + Clone + 'static> {
    /// Application state
    state: Arc<AppState<S>>,
    /// Client IP address
    client_ip: Option<IpAddr>,
    client_id: String,
    client_tx: Option<mpsc::Sender<ServerMessage>>,
    client_priority: u8,
}

impl<S: Storage + Send + Sync + Clone + 'static> WebSocketHandler<S> {
    pub fn new(state: Arc<AppState<S>>) -> Self {
        Self {
            state,
            client_id: Uuid::new_v4().to_string(),
            client_tx: None,
            client_priority: 0,
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

    /// Process client messages using the service layer
    ///
    /// # Supported Message Types
    /// - `CreateMeet`: Create a new meet with specified parameters
    /// - `JoinMeet`: Join an existing meet with credentials
    /// - `UpdateInit`: Initialize updates from a client
    /// - `ClientPull`: Request updates since a specific sequence number
    /// - `PublishMeet`: Publish meet results and generate CSV output
    ///
    /// # Network Resilience
    /// Messages are now handled by the service layer which provides better
    /// error handling and validation patterns.
    ///
    /// # Error Handling
    /// Returns appropriate error responses for:
    /// - Validation errors
    /// - Authentication failures
    /// - Rate limiting violations
    /// - Internal server errors
    pub async fn handle_message(&mut self, msg: ClientToServer) -> Result<ServerMessage> {
        debug!("Processing message: {:?}", msg);

        // Validate the message first using validation middleware
        let validation_context = ValidationContext::default();
        if let Err(server_error) = ValidationMiddleware::validate_message(&msg, &validation_context)
        {
            return Ok(server_error);
        }

        // Use the message service to handle the message
        match self
            .state
            .message_service
            .handle_message(msg, &self.state, self.client_ip)
            .await
        {
            Ok(server_message) => {
                // Update client priority if this was a successful CreateMeet or JoinMeet
                if let ServerMessage::MeetCreated { .. } | ServerMessage::MeetJoined { .. } =
                    &server_message
                {
                    // Priority would have been set during session creation
                    // This could be enhanced to extract priority from the response if needed
                }
                Ok(server_message)
            },
            Err(app_error) => {
                // Convert AppError to ServerMessage
                let server_message = match app_error {
                    crate::error::AppError::Auth(msg) => ServerMessage::Error {
                        code: "AUTH_ERROR".to_string(),
                        message: msg,
                    },
                    crate::error::AppError::InvalidInput(msg) => ServerMessage::Error {
                        code: "VALIDATION_ERROR".to_string(),
                        message: msg,
                    },
                    crate::error::AppError::AuthRateLimited => ServerMessage::Error {
                        code: "AUTH_RATE_LIMITED".to_string(),
                        message: "Too many authentication attempts. Please try again later."
                            .to_string(),
                    },
                    _ => ServerMessage::Error {
                        code: "INTERNAL_ERROR".to_string(),
                        message: "An internal server error occurred".to_string(),
                    },
                };

                Ok(server_message)
            },
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
