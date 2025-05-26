// ============================
// crates/server-app/src/handlers/live.rs
// ============================
//! Live WebSocket handlers.
use crate::auth::{
    hash_password, validate_password_strength, verify_password, PasswordRequirements,
};
use crate::storage::Storage;
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
            // Validate password strength
            let requirements = PasswordRequirements::default();
            if !validate_password_strength(&password, &requirements) {
                send_error(
                    &tx,
                    &format!(
                        "Password must be at least {} characters and contain uppercase, lowercase, digit, and special character",
                        requirements.min_length
                    ),
                ).await?;
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

            update_session_metrics("created", None, None);
        },

        ClientToServer::JoinMeet {
            meet_id,
            password,
            location_name,
        } => {
            // Get meet info and verify password
            let meet_info = state.storage.get_meet_info(&meet_id).await?;

            if !verify_password(&meet_info.password_hash, &password) {
                send_error(&tx, "Invalid password").await?;
                return Ok(());
            }

            // Find priority and create session
            let priority = meet_info
                .endpoints
                .iter()
                .find(|e| e.location_name == location_name)
                .map_or(0, |e| e.priority);

            let session_token = state
                .auth
                .new_session(meet_id, location_name, priority)
                .await;

            send_response(&tx, ServerToClient::MeetJoined { session_token }).await?;
            update_session_metrics("joined", None, None);
        },

        ClientToServer::UpdateInit {
            session_token,
            updates,
        } => {
            // Validate session and get meet handle
            let session = state
                .auth
                .get_session(&session_token)
                .await
                .ok_or_else(|| AppError::Auth("Invalid session".to_string()))?;

            let handle = state
                .meet_handles
                .get(&session.meet_id)
                .ok_or(AppError::MeetNotFound)?;

            let updates_len = updates.len();

            // Convert update formats
            let backend_updates = updates
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
                },
                Err(e) => {
                    send_error(&tx, &e.to_string()).await?;
                },
            }

            update_session_metrics("updated", Some(updates_len), None);
        },

        ClientToServer::ClientPull {
            session_token,
            last_server_seq,
        } => {
            // Validate session and get updates
            let session = state
                .auth
                .get_session(&session_token)
                .await
                .ok_or_else(|| AppError::Auth("Invalid session".to_string()))?;

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
            return_email: _,
            opl_csv,
        } => {
            // Validate session
            state
                .auth
                .get_session(&session_token)
                .await
                .ok_or_else(|| AppError::Auth("Invalid session".to_string()))?;

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
    use crate::{config::Settings, storage::FlatFileStorage};
    use openlifter_common::{ClientToServer, EndpointPriority, ServerToClient};
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::sync::mpsc;

    /// Test environment setup
    async fn setup() -> (
        Arc<AppState<FlatFileStorage>>,
        mpsc::Sender<Message>,
        mpsc::Receiver<Message>,
        TempDir,
    ) {
        let temp_dir = TempDir::new().unwrap();
        let storage = FlatFileStorage::new(temp_dir.path()).unwrap();
        let mut settings = Settings::default();
        settings.storage.path = temp_dir.path().to_path_buf();

        std::fs::create_dir_all(temp_dir.path().join("sessions")).unwrap();

        let state = Arc::new(AppState::new(storage, &settings).await.unwrap());
        let (tx, rx) = mpsc::channel(32);
        (state, tx, rx, temp_dir)
    }

    #[tokio::test]
    async fn test_create_meet_valid() {
        let (state, tx, mut rx, _) = setup().await;
        let msg = ClientToServer::CreateMeet {
            this_location_name: "Test Location".to_string(),
            password: "Password123!".to_string(),
            endpoints: vec![EndpointPriority {
                location_name: "Test Location".to_string(),
                priority: 5,
            }],
        };

        assert!(handle_client_message(msg, &state, tx).await.is_ok());

        if let Some(Message::Text(json)) = rx.recv().await {
            let response: ServerToClient = serde_json::from_str(&json).unwrap();
            assert!(matches!(response, ServerToClient::MeetCreated { .. }));
        }
    }

    #[tokio::test]
    async fn test_create_meet_weak_password() {
        let (state, tx, mut rx, _) = setup().await;
        let msg = ClientToServer::CreateMeet {
            this_location_name: "Test Location".to_string(),
            password: "weak".to_string(),
            endpoints: vec![],
        };

        handle_client_message(msg, &state, tx).await.unwrap();

        if let Some(Message::Text(json)) = rx.recv().await {
            let response: ServerToClient = serde_json::from_str(&json).unwrap();
            assert!(matches!(response, ServerToClient::MalformedMessage { .. }));
        }
    }

    #[tokio::test]
    async fn test_join_meet_valid() {
        let (state, tx, mut rx, _) = setup().await;

        // First create a meet to join
        let create_msg = ClientToServer::CreateMeet {
            this_location_name: "Test Location".to_string(),
            password: "Password123!".to_string(),
            endpoints: vec![EndpointPriority {
                location_name: "Test Location".to_string(),
                priority: 5,
            }],
        };

        handle_client_message(create_msg, &state, tx.clone())
            .await
            .unwrap();

        // Get the created meet ID from the response
        let meet_id = if let Some(Message::Text(json)) = rx.recv().await {
            let response: ServerToClient = serde_json::from_str(&json).unwrap();
            match response {
                ServerToClient::MeetCreated { meet_id, .. } => meet_id,
                _ => panic!("Expected MeetCreated response"),
            }
        } else {
            panic!("Expected response message")
        };

        let msg = ClientToServer::JoinMeet {
            meet_id,
            password: "Password123!".to_string(),
            location_name: "Test Location".to_string(),
        };

        assert!(handle_client_message(msg, &state, tx).await.is_ok());
    }

    #[tokio::test]
    async fn test_update_init() {
        let (state, tx, mut rx, _) = setup().await;

        // Create a meet and get session token
        let create_msg = ClientToServer::CreateMeet {
            this_location_name: "Test Location".to_string(),
            password: "Password123!".to_string(),
            endpoints: vec![EndpointPriority {
                location_name: "Test Location".to_string(),
                priority: 5,
            }],
        };

        handle_client_message(create_msg, &state, tx.clone())
            .await
            .unwrap();

        // Get the session token from the response
        let session_token = if let Some(Message::Text(json)) = rx.recv().await {
            let response: ServerToClient = serde_json::from_str(&json).unwrap();
            match response {
                ServerToClient::MeetCreated { session_token, .. } => session_token,
                _ => panic!("Expected MeetCreated response"),
            }
        } else {
            panic!("Expected response message")
        };

        let msg = ClientToServer::UpdateInit {
            session_token,
            updates: vec![],
        };

        assert!(handle_client_message(msg, &state, tx).await.is_ok());
    }

    #[tokio::test]
    async fn test_client_pull() {
        let (state, tx, mut rx, _) = setup().await;

        // Create a meet and get session token
        let create_msg = ClientToServer::CreateMeet {
            this_location_name: "Test Location".to_string(),
            password: "Password123!".to_string(),
            endpoints: vec![EndpointPriority {
                location_name: "Test Location".to_string(),
                priority: 5,
            }],
        };

        handle_client_message(create_msg, &state, tx.clone())
            .await
            .unwrap();

        // Get the session token from the response
        let session_token = if let Some(Message::Text(json)) = rx.recv().await {
            let response: ServerToClient = serde_json::from_str(&json).unwrap();
            match response {
                ServerToClient::MeetCreated { session_token, .. } => session_token,
                _ => panic!("Expected MeetCreated response"),
            }
        } else {
            panic!("Expected response message")
        };

        let msg = ClientToServer::ClientPull {
            session_token,
            last_server_seq: 0,
        };

        assert!(handle_client_message(msg, &state, tx).await.is_ok());
    }

    #[tokio::test]
    async fn test_publish_meet() {
        let (state, tx, mut rx, _) = setup().await;

        // Create a meet and get session token
        let create_msg = ClientToServer::CreateMeet {
            this_location_name: "Test Location".to_string(),
            password: "Password123!".to_string(),
            endpoints: vec![EndpointPriority {
                location_name: "Test Location".to_string(),
                priority: 5,
            }],
        };

        handle_client_message(create_msg, &state, tx.clone())
            .await
            .unwrap();

        // Get the session token from the response
        let session_token = if let Some(Message::Text(json)) = rx.recv().await {
            let response: ServerToClient = serde_json::from_str(&json).unwrap();
            match response {
                ServerToClient::MeetCreated { session_token, .. } => session_token,
                _ => panic!("Expected MeetCreated response"),
            }
        } else {
            panic!("Expected response message")
        };

        let msg = ClientToServer::PublishMeet {
            session_token,
            return_email: "test@example.com".to_string(),
            opl_csv: "Name,Weight\nJohn,93".to_string(),
        };

        assert!(handle_client_message(msg, &state, tx).await.is_ok());
    }
}
