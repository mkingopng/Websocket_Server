// ============================
// openlifter-backend-lib/src/handlers/live.rs
// ============================
//! Live WebSocket handlers.
use tokio::sync::mpsc;
use axum::extract::ws::Message;
use openlifter_common::{ClientToServer, ServerToClient};
use crate::{AppState, error::AppError};
use crate::auth::{hash_password, verify_password, validate_password_strength, PasswordRequirements};
use rand::Rng;
use metrics::{counter, histogram};
use std::time::Instant;
use log;

/// Handle a client message
pub async fn handle_client_message(
    msg: ClientToServer,
    state: &AppState,
    tx: mpsc::Sender<Message>,
) -> Result<(), AppError> {
    let start = Instant::now();
    
    let result = match msg {
        ClientToServer::CreateMeet { this_location_name, password, endpoints } => {
            // Validate password strength
            let requirements = PasswordRequirements::default();
            if !validate_password_strength(&password, &requirements) {
                let err = ServerToClient::MalformedMessage { 
                    err_msg: format!("Password must be at least {} characters and contain uppercase, lowercase, digit, and special character", requirements.min_length) 
                };
                let json = serde_json::to_string(&err)?;
                tx.send(Message::Text(json.into())).await.map_err(|_| AppError::Internal("Failed to send message".to_string()))?;
                return Ok(());
            }
            
            // Generate a meet ID
            let meet_id = format!(
                "{}-{}-{}",
                rand::thread_rng().gen_range(100..1000),
                rand::thread_rng().gen_range(100..1000),
                rand::thread_rng().gen_range(100..1000)
            );
            
            // Hash the password
            let hashed_password = hash_password(&password)
                .map_err(|e| AppError::Internal(e.to_string()))?;
            
            // Store meet info
            state.storage.store_meet_info(&meet_id, &hashed_password, &endpoints).await?;
            
            // Create the meet actor
            let handle = state.meets.create_meet(meet_id.clone(), state.storage.clone()).await;
            
            // Create a session
            let session_token = state.auth_srv.new_session(meet_id.clone(), this_location_name, endpoints[0].priority).await;
            
            // Send response
            let reply = ServerToClient::MeetCreated { meet_id, session_token };
            let json = serde_json::to_string(&reply)?;
            tx.send(Message::Text(json.into())).await.map_err(|_| AppError::Internal("Failed to send message".to_string()))?;
            
            // Update metrics
            counter!("meet.created", 1);
        }
        
        ClientToServer::JoinMeet { meet_id, password, location_name } => {
            // Get meet info
            let meet_info = state.storage.get_meet_info(&meet_id).await?;
            
            // Verify password
            if !verify_password(&meet_info.password_hash, &password) {
                let err = ServerToClient::MalformedMessage { 
                    err_msg: "Invalid password".to_string() 
                };
                let json = serde_json::to_string(&err)?;
                tx.send(Message::Text(json.into())).await.map_err(|_| AppError::Internal("Failed to send message".to_string()))?;
                return Ok(());
            }
            
            // Find priority for this location
            let priority = meet_info.endpoints.iter()
                .find(|e| e.location_name == location_name)
                .map(|e| e.priority)
                .unwrap_or(0);
            
            // Create session
            let session_token = state.auth_srv.new_session(meet_id.clone(), location_name, priority).await;
            
            // Send response
            let reply = ServerToClient::MeetJoined { session_token };
            let json = serde_json::to_string(&reply)?;
            tx.send(Message::Text(json.into())).await.map_err(|_| AppError::Internal("Failed to send message".to_string()))?;
            
            // Update metrics
            counter!("meet.joined", 1);
        }
        
        ClientToServer::UpdateInit { session_token, updates } => {
            // Validate session
            let session = state.auth_srv.get_session(&session_token).await.ok_or_else(|| {
                AppError::Auth("Invalid session".to_string())
            })?;

            // Get meet handle
            let meet = state.meets.get_meet(&session.meet_id).ok_or_else(|| {
                AppError::MeetNotFound
            })?;

            // Store updates length before moving
            let updates_len = updates.len();
            
            // Apply updates
            let results = meet.apply_updates(
                session.location_name.clone(),
                session.priority,
                updates,
            ).await?;

            // Send response
            let reply = ServerToClient::UpdateAck { update_acks: results };
            let json = serde_json::to_string(&reply)?;
            tx.send(Message::Text(json.into())).await.map_err(|_| AppError::Internal("Failed to send message".to_string()))?;

            // Update metrics
            counter!("meet.update", 1, "meet_id" => session.meet_id.clone());
            histogram!("update.batch_size", updates_len as f64);
        }
        
        ClientToServer::ClientPull { session_token, last_server_seq: since } => {
            // Validate session
            let session = state.auth_srv.get_session(&session_token).await.ok_or_else(|| {
                AppError::Auth("Invalid session".to_string())
            })?;

            // Get meet handle
            let meet = state.meets.get_meet(&session.meet_id).ok_or_else(|| {
                AppError::MeetNotFound
            })?;

            // Get updates since last seen
            let updates = meet.get_updates_since(since).await?;

            // Send response
            let reply = ServerToClient::ServerPull {
                last_server_seq: since,
                updates_relayed: updates.clone(),
            };
            let json = serde_json::to_string(&reply)?;
            tx.send(Message::Text(json.into())).await.map_err(|_| AppError::Internal("Failed to send message".to_string()))?;

            // Update metrics
            counter!("meet.pull", 1, "meet_id" => session.meet_id.clone());
            histogram!("pull.updates_count", updates.len() as f64);
        }
        
        ClientToServer::PublishMeet { session_token, return_email, opl_csv } => {
            // Validate session
            let session = state.auth_srv.get_session(&session_token).await.ok_or_else(|| {
                AppError::Auth("Invalid session".to_string())
            })?;

            // Get meet handle
            let meet = state.meets.get_meet(&session.meet_id).ok_or_else(|| {
                AppError::MeetNotFound
            })?;

            // Store CSV data
            let csv_len = opl_csv.len();
            meet.store_csv_data(opl_csv, return_email).await?;

            // Send response
            let reply = ServerToClient::PublishAck;
            let json = serde_json::to_string(&reply)?;
            tx.send(Message::Text(json.into())).await.map_err(|_| AppError::Internal("Failed to send message".to_string()))?;

            // Update metrics
            counter!("meet.publish", 1, "meet_id" => session.meet_id.clone());
            histogram!("publish.csv_size", csv_len as f64);
        }
    };
    
    // Record handler duration
    let duration = start.elapsed();
    histogram!("handler.duration_ms", duration.as_millis() as f64);
    
    Ok(())
} 