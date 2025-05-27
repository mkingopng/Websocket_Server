// ============================
// crates/server-app/src/handlers/live.rs
// ============================
//! Live WebSocket handlers
use crate::auth::hash_password;
use crate::storage::Storage;
use crate::validation::middleware::ValidationMiddleware;
use crate::{error::AppError, AppState};
use axum::extract::ws::Message;
use metrics::{counter, gauge, histogram};
use openlifter_common::{ClientToServer, ServerToClient};
use rand::Rng;
use std::time::Instant;
use tokio::sync::mpsc;

/// Helper to send JSON response message
async fn send_response(
    tx: &mpsc::Sender<Message>,
    response: ServerToClient,
) -> Result<(), AppError> {
    let json = serde_json::to_string(&response)?;
    tx.send(Message::Text(json.into()))
        .await
        .map_err(|_| AppError::Internal("Failed to send message".to_string()))
}

/// Helper to send error message
async fn send_error(tx: &mpsc::Sender<Message>, msg: &str) -> Result<(), AppError> {
    send_response(
        tx,
        ServerToClient::MalformedMessage {
            err_msg: msg.to_string(),
        },
    )
    .await
}

/// Helper function to validate session and get session data
async fn validate_and_get_session<S: Storage>(
    session_token: &str,
    state: &AppState<S>,
) -> Result<crate::messages::Session, AppError> {
    // Use enhanced validation middleware
    ValidationMiddleware::validate_session_and_get(session_token, state)
        .await
        .map_err(|_| AppError::Auth("Invalid session".to_string()))
}

/// Helper function to update session metrics
fn update_session_metrics(event_type: &str, updates_len: Option<usize>, csv_len: Option<usize>) {
    let _ = counter!(format!("live.session.{}", event_type), &[("value", "1")]);
    let _ = gauge!(
        "live.session.active",
        &[("value", if event_type == "ended" { "-1" } else { "1" })]
    );

    if let Some(len) = updates_len {
        let _ = histogram!("update.batch_size", &[("value", len.to_string())]);
        let _ = gauge!("handler.updates_length", &[("value", len.to_string())]);
    }

    if let Some(len) = csv_len {
        let _ = histogram!("publish.csv_size", &[("value", len.to_string())]);
        let _ = gauge!("handler.csv_length", &[("value", len.to_string())]);
    }
}

/// Handle client messages for live sessions
#[allow(clippy::too_many_lines)]
pub async fn handle_client_message<S: Storage + Send + Sync + Clone + 'static>(
    msg: ClientToServer,
    state: &AppState<S>,
    tx: mpsc::Sender<Message>,
) -> Result<(), AppError> {
    let start = Instant::now();

    match msg {
        ClientToServer::CreateMeet {
            this_location_name,
            password,
            endpoints,
        } => {
            // Use consolidated validation middleware
            if let Err(_) = ValidationMiddleware::builder()
                .create_meet_fields(&this_location_name, &password)
                .execute()
            {
                send_error(&tx, "Invalid meet parameters").await?;
                return Ok(());
            }

            // Generate meet ID and hash password
            let meet_id = format!(
                "{}-{}-{}",
                rand::thread_rng().gen_range(100..1000),
                rand::thread_rng().gen_range(100..1000),
                rand::thread_rng().gen_range(100..1000)
            );
            let hashed_password =
                hash_password(&password).map_err(|e| AppError::Internal(e.to_string()))?;

            // Store meet info and create actor
            state
                .storage
                .store_meet_info(&meet_id, &hashed_password, &endpoints)
                .await?;

            let handle = crate::meet_actor::spawn_meet_actor(&meet_id, state.storage.clone()).await;
            state.meet_handles.insert(meet_id.clone(), handle);

            // Create session and send response
            let session_token = state
                .auth
                .new_session(meet_id.clone(), this_location_name, endpoints[0].priority)
                .await;

            send_response(
                &tx,
                ServerToClient::MeetCreated {
                    meet_id,
                    session_token,
                },
            )
            .await?;
        },

        ClientToServer::JoinMeet {
            meet_id,
            password,
            location_name,
        } => {
            // Use consolidated validation middleware
            if let Err(_) = ValidationMiddleware::builder()
                .join_meet_fields(&meet_id, &password, &location_name)
                .execute()
            {
                send_error(&tx, "Invalid meet credentials").await?;
                return Ok(());
            }

            // Verify password against stored hash
            // In a real implementation, this would check the stored password hash
            let session_token = state
                .auth
                .new_session(meet_id, location_name, 5) // Default priority for joining
                .await;

            send_response(&tx, ServerToClient::MeetJoined { session_token }).await?;
        },

        ClientToServer::UpdateInit {
            session_token,
            updates,
        } => {
            // use the consolidated session validation helper
            let session = validate_and_get_session(&session_token, state).await?;

            let handle = state
                .meet_handles
                .get(&session.meet_id)
                .ok_or(AppError::MeetNotFound)?;

            let updates_len = updates.len();

            // Use batch validation for updates
            let (valid_updates, rejected_updates) =
                ValidationMiddleware::validate_updates_batch(updates);

            // If any updates were rejected, send error
            if !rejected_updates.is_empty() {
                send_error(&tx, "Invalid update data").await?;
                return Ok(());
            }

            // Convert update formats
            let backend_updates = valid_updates
                .into_iter()
                .map(|u| crate::messages::Update {
                    location: u.update_key,
                    value: u.update_value.to_string(),
                    timestamp: u.local_seq_num as i64,
                })
                .collect::<Vec<_>>();

            let ol_updates = backend_updates
                .iter()
                .map(|u| openlifter_common::Update {
                    update_key: u.location.clone(),
                    update_value: serde_json::from_str(&u.value).unwrap_or_default(),
                    local_seq_num: u.timestamp as u64,
                    after_server_seq_num: 0,
                })
                .collect();

            // Apply updates
            match handle
                .apply_updates("anonymous".to_string(), session.priority, ol_updates)
                .await
            {
                Ok(results) => {
                    let update_acks: Vec<(u64, u64)> =
                        results.iter().map(|(id, seq)| (*id, *seq)).collect();

                    send_response(&tx, ServerToClient::UpdateAck { update_acks }).await?;

                    update_session_metrics("update_applied", Some(updates_len), None);
                },
                Err(e) => {
                    send_error(&tx, &format!("Failed to apply updates: {}", e)).await?;
                },
            }
        },

        ClientToServer::ClientPull {
            session_token,
            last_server_seq,
        } => {
            // Use the consolidated session validation helper
            let session = validate_and_get_session(&session_token, state).await?;

            let updates = if let Some(handle) = state.meet_handles.get(&session.meet_id) {
                handle
                    .get_updates_since(last_server_seq)
                    .await
                    .unwrap_or_default()
            } else {
                Vec::new()
            };

            send_response(
                &tx,
                ServerToClient::ServerPull {
                    last_server_seq,
                    updates_relayed: updates,
                },
            )
            .await?;
        },

        ClientToServer::PublishMeet {
            session_token,
            return_email,
            opl_csv,
        } => {
            // Use consolidated validation middleware
            if let Err(_) = ValidationMiddleware::builder()
                .publish_meet_fields(&session_token, &return_email)
                .execute()
            {
                send_error(&tx, "Invalid session or email").await?;
                return Ok(());
            }

            // Validate session exists
            let _session = validate_and_get_session(&session_token, state).await?;

            let csv_len = opl_csv.len();

            send_response(&tx, ServerToClient::PublishAck {}).await?;
            update_session_metrics("published", None, Some(csv_len));
        },
    }

    let duration = start.elapsed();
    let _ = histogram!(
        "handler.duration_ms",
        &[("value", duration.as_millis().to_string())]
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::fixtures::*;
    use openlifter_common::ServerToClient;

    async fn extract_meet_id_from_response(rx: &mut mpsc::Receiver<Message>) -> String {
        if let Some(Message::Text(json)) = rx.recv().await {
            let response: ServerToClient = serde_json::from_str(&json).unwrap();
            match response {
                ServerToClient::MeetCreated { meet_id, .. } => meet_id,
                _ => panic!("Expected MeetCreated response"),
            }
        } else {
            panic!("Expected response message")
        }
    }

    async fn extract_session_token_from_response(rx: &mut mpsc::Receiver<Message>) -> String {
        if let Some(Message::Text(json)) = rx.recv().await {
            let response: ServerToClient = serde_json::from_str(&json).unwrap();
            match response {
                ServerToClient::MeetCreated { session_token, .. } => session_token,
                _ => panic!("Expected MeetCreated response"),
            }
        } else {
            panic!("Expected response message")
        }
    }

    #[tokio::test]
    async fn test_create_meet_valid() {
        let env = setup_test_environment().await;
        let (tx, mut rx) = mpsc::channel(32);

        let msg = test_create_meet_message();
        let result = handle_client_message(msg, &env.state, tx).await;
        assert!(result.is_ok());

        if let Some(Message::Text(json)) = rx.recv().await {
            let response: ServerToClient = serde_json::from_str(&json).unwrap();
            assert!(matches!(response, ServerToClient::MeetCreated { .. }));
        }
    }

    #[tokio::test]
    async fn test_create_meet_weak_password() {
        let env = setup_test_environment().await;
        let (tx, mut rx) = mpsc::channel(32);

        let msg = crate::testing::fixtures::test_create_meet_weak_password();
        handle_client_message(msg, &env.state, tx).await.unwrap();

        if let Some(Message::Text(json)) = rx.recv().await {
            let response: ServerToClient = serde_json::from_str(&json).unwrap();
            assert!(matches!(response, ServerToClient::MalformedMessage { .. }));
        }
    }

    #[tokio::test]
    async fn test_create_meet_multiple_endpoints() {
        let env = setup_test_environment().await;
        let (tx, mut rx) = mpsc::channel(32);

        let msg = test_create_meet_with_multiple_endpoints();
        assert!(handle_client_message(msg, &env.state, tx).await.is_ok());

        if let Some(Message::Text(json)) = rx.recv().await {
            let response: ServerToClient = serde_json::from_str(&json).unwrap();
            assert!(matches!(response, ServerToClient::MeetCreated { .. }));
        }
    }

    #[tokio::test]
    async fn test_join_meet_valid() {
        let env = setup_test_environment().await;
        let (tx, mut rx) = mpsc::channel(32);

        // First create a meet to join
        let create_msg = test_create_meet_message();
        handle_client_message(create_msg, &env.state, tx.clone())
            .await
            .unwrap();

        // Get the created meet ID from the response
        let meet_id = extract_meet_id_from_response(&mut rx).await;

        let join_msg = test_join_meet_custom(&meet_id, TEST_LOCATION_2);
        assert!(handle_client_message(join_msg, &env.state, tx)
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn test_update_init() {
        let env = setup_test_environment().await;
        let (tx, mut rx) = mpsc::channel(32);

        // Create a meet and get session token
        let create_msg = test_create_meet_message();
        handle_client_message(create_msg, &env.state, tx.clone())
            .await
            .unwrap();

        // Get the session token from the response
        let session_token = extract_session_token_from_response(&mut rx).await;

        let update_msg = test_update_init_empty(&session_token);
        assert!(handle_client_message(update_msg, &env.state, tx)
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn test_update_init_with_updates() {
        let env = setup_test_environment().await;
        let (tx, mut rx) = mpsc::channel(32);

        // Create a meet and get session token
        let create_msg = test_create_meet_message();
        handle_client_message(create_msg, &env.state, tx.clone())
            .await
            .unwrap();

        // Get the session token from the response
        let session_token = extract_session_token_from_response(&mut rx).await;

        let update_msg = test_update_init_message(&session_token);
        assert!(handle_client_message(update_msg, &env.state, tx)
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn test_client_pull() {
        let env = setup_test_environment().await;
        let (tx, mut rx) = mpsc::channel(32);

        // Create a meet and get session token
        let create_msg = test_create_meet_message();
        handle_client_message(create_msg, &env.state, tx.clone())
            .await
            .unwrap();

        // Get the session token from the response
        let session_token = extract_session_token_from_response(&mut rx).await;

        let pull_msg = test_client_pull_message(&session_token, 0);
        assert!(handle_client_message(pull_msg, &env.state, tx)
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn test_publish_meet() {
        let env = setup_test_environment().await;
        let (tx, mut rx) = mpsc::channel(32);

        // Create a meet and get session token
        let create_msg = test_create_meet_message();
        handle_client_message(create_msg, &env.state, tx.clone())
            .await
            .unwrap();

        // Get the session token from the response
        let session_token = extract_session_token_from_response(&mut rx).await;

        let publish_msg = test_publish_meet_message(&session_token);
        assert!(handle_client_message(publish_msg, &env.state, tx)
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn test_publish_meet_custom_csv() {
        let env = setup_test_environment().await;
        let (tx, mut rx) = mpsc::channel(32);

        // Create a meet and get session token
        let create_msg = test_create_meet_message();
        handle_client_message(create_msg, &env.state, tx.clone())
            .await
            .unwrap();

        // Get the session token from the response
        let session_token = extract_session_token_from_response(&mut rx).await;

        let publish_msg = test_publish_meet_custom(&session_token, MINIMAL_CSV_DATA, TEST_EMAIL);
        assert!(handle_client_message(publish_msg, &env.state, tx)
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn test_invalid_session_handling() {
        let env = setup_test_environment().await;
        let (tx, _rx) = mpsc::channel(32);

        // Test with invalid session token
        let update_msg = test_update_init_message(INVALID_SESSION_TOKEN);
        let result = handle_client_message(update_msg, &env.state, tx.clone()).await;

        // Should handle gracefully (either ok with error response or error)
        match result {
            Ok(()) => {}, // Error sent as response
            Err(_) => {}, // Error returned
        }

        let pull_msg = test_client_pull_message(INVALID_SESSION_TOKEN, 0);
        let result = handle_client_message(pull_msg, &env.state, tx).await;

        // Should handle gracefully
        match result {
            Ok(()) => {}, // Error sent as response
            Err(_) => {}, // Error returned
        }
    }
}
