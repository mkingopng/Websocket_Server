// crates/backend-lib/src/handlers/live.rs

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

/** Handler for live session events
This handler processes various live session events like:
- `created`: When a new live session is created
- `joined`: When a user joins a live session
- `updated`: When a live session is updated
- `published`: When a live session is published
- `ended`: When a live session ends

The handler validates the session token and user ID, then processes
the event based on its type. For each event type, it:
1. Validates the session exists and belongs to the user
2. Updates the session state
3. Records metrics
4. Returns appropriate response
Handle a client message */
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
                let err = ServerToClient::MalformedMessage {
                    err_msg: format!("Password must be at least {} characters and contain uppercase, lowercase, digit, and special character", requirements.min_length) 
                };
                let json = serde_json::to_string(&err)?;
                tx.send(Message::Text(json.into()))
                    .await
                    .map_err(|_| AppError::Internal("Failed to send message".to_string()))?;
                return Ok(());
            }

            // Generate a meet ID
            let meet_id = {
                let mut rng = rand::thread_rng();
                format!(
                    "{}-{}-{}",
                    rng.gen_range(100..1000),
                    rng.gen_range(100..1000),
                    rng.gen_range(100..1000)
                )
            };

            // Hash the password
            let hashed_password =
                hash_password(&password).map_err(|e| AppError::Internal(e.to_string()))?;

            // Store meet info
            state
                .storage
                .store_meet_info(&meet_id, &hashed_password, &endpoints)
                .await?;

            // Create the meet actor
            let storage_clone = state.storage.clone();
            let handle = crate::meet_actor::spawn_meet_actor(&meet_id, storage_clone).await;
            state.meet_handles.insert(meet_id.clone(), handle);

            // Create a session
            let session_token = state
                .auth
                .new_session(meet_id.clone(), this_location_name, endpoints[0].priority)
                .await;

            // Send response
            let reply = ServerToClient::MeetCreated {
                meet_id,
                session_token,
            };
            let json = serde_json::to_string(&reply)?;
            tx.send(Message::Text(json.into()))
                .await
                .map_err(|_| AppError::Internal("Failed to send message".to_string()))?;

            // Update metrics
            update_session_metrics("created", None, None);
        },

        ClientToServer::JoinMeet {
            meet_id,
            password,
            location_name,
        } => {
            // Get meet info
            let meet_info = state.storage.get_meet_info(&meet_id).await?;

            // Verify password
            if !verify_password(&meet_info.password_hash, &password) {
                let err = ServerToClient::MalformedMessage {
                    err_msg: "Invalid password".to_string(),
                };
                let json = serde_json::to_string(&err)?;
                tx.send(Message::Text(json.into()))
                    .await
                    .map_err(|_| AppError::Internal("Failed to send message".to_string()))?;
                return Ok(());
            }

            // Find priority for this location
            let priority = meet_info
                .endpoints
                .iter()
                .find(|e| e.location_name == location_name)
                .map_or(0, |e| e.priority);

            // Create session
            let session_token = state
                .auth
                .new_session(meet_id.clone(), location_name, priority)
                .await;

            // Send response
            let reply = ServerToClient::MeetJoined { session_token };
            let json = serde_json::to_string(&reply)?;
            tx.send(Message::Text(json.into()))
                .await
                .map_err(|_| AppError::Internal("Failed to send message".to_string()))?;

            // Update metrics
            update_session_metrics("joined", None, None);
        },

        ClientToServer::UpdateInit {
            session_token,
            updates,
        } => {
            // Validate session
            let session = state
                .auth
                .get_session(&session_token)
                .await
                .ok_or_else(|| AppError::Auth("Invalid session".to_string()))?;

            // Get meet handle
            let handle = state
                .meet_handles
                .get(&session.meet_id)
                .ok_or(AppError::MeetNotFound)?;

            // Store updates length before moving
            let updates_len = updates.len();

            // Convert from openlifter_common::Update to backend's Update format
            let backend_updates = updates
                .into_iter()
                .map(|u| crate::messages::Update {
                    location: u.update_key,
                    value: u.update_value.to_string(),
                    timestamp: u.local_seq_num as i64,
                })
                .collect::<Vec<_>>();

            // Convert from backend's Update to openlifter_common::Update
            let ol_updates = backend_updates
                .iter()
                .map(|u| openlifter_common::Update {
                    update_key: u.location.clone(),
                    update_value: serde_json::from_str(&u.value).unwrap_or(serde_json::Value::Null),
                    local_seq_num: u.timestamp as u64,
                    after_server_seq_num: 0,
                })
                .collect();

            // Apply updates
            let results = handle
                .apply_updates(session.location_name.clone(), session.priority, ol_updates)
                .await?;

            // Send response
            let _update_ids: Vec<String> = results.iter().map(|(id, _)| id.to_string()).collect();
            let reply = ServerToClient::UpdateAck {
                update_acks: results,
            };
            let json = serde_json::to_string(&reply)?;
            tx.send(Message::Text(json.into()))
                .await
                .map_err(|_| AppError::Internal("Failed to send message".to_string()))?;

            // Update metrics
            update_session_metrics("updated", Some(updates_len), None);
        },

        ClientToServer::ClientPull {
            session_token,
            last_server_seq,
        } => {
            // Validate session
            let session = state
                .auth
                .get_session(&session_token)
                .await
                .ok_or_else(|| AppError::Auth("Invalid session".to_string()))?;

            // Get meet handle
            let handle = state
                .meet_handles
                .get(&session.meet_id)
                .ok_or(AppError::MeetNotFound)?;

            // Get updates since last seen
            let updates = handle.get_updates_since(last_server_seq).await?;
            let updates_len = updates.len();

            // Send response
            let reply = ServerToClient::ServerPull {
                last_server_seq,
                updates_relayed: updates.clone(),
            };
            let json = serde_json::to_string(&reply)?;
            tx.send(Message::Text(json.into()))
                .await
                .map_err(|_| AppError::Internal("Failed to send message".to_string()))?;

            // Update metrics
            let _ = counter!("meet.pull", &[("value", "1")]);
            let _ = histogram!("pull.updates_count", &[("value", updates_len.to_string())]);
            let _ = gauge!("live.session.active", &[("value", "1")]);
        },

        ClientToServer::PublishMeet {
            session_token,
            return_email,
            opl_csv,
        } => {
            // Validate session
            let session = state
                .auth
                .get_session(&session_token)
                .await
                .ok_or_else(|| AppError::Auth("Invalid session".to_string()))?;

            // Get meet handle
            let handle = state
                .meet_handles
                .get(&session.meet_id)
                .ok_or(AppError::MeetNotFound)?;

            // Store CSV data
            let csv_len = opl_csv.len();
            handle.store_csv_data(opl_csv, return_email).await?;

            // Send response
            let reply = ServerToClient::PublishAck;
            let json = serde_json::to_string(&reply)?;
            tx.send(Message::Text(json.into()))
                .await
                .map_err(|_| AppError::Internal("Failed to send message".to_string()))?;

            // Update metrics
            update_session_metrics("published", None, Some(csv_len));
        },
    }

    // Record handler duration
    let _duration = start.elapsed();

    Ok(())
}
