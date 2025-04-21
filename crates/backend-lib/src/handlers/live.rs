// ============================
// openlifter-backend-lib/src/handlers/live.rs
// ============================
//! Live WebSocket handlers.
use tokio::sync::mpsc;
use axum::extract::ws::Message;
use openlifter_common::{ClientToServer, ServerToClient};
use crate::{AppState, error::AppError, meet_actor::MeetHandle};
use crate::auth::{hash_password, verify_password, validate_password_strength, PasswordRequirements};
use rand::Rng;
use metrics::{counter, histogram};
use std::time::Instant;

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
                let err = ServerToClient::MeetCreationRejected { 
                    reason: format!("Password must be at least {} characters and contain uppercase, lowercase, digit, and special character", requirements.min_length) 
                };
                let json = serde_json::to_string(&err)?;
                tx.send(Message::Text(json)).await?;
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
            let session_token = state.auth.new_session(meet_id.clone(), this_location_name, endpoints[0].priority).await;
            
            // Send response
            let reply = ServerToClient::MeetCreated { meet_id, session_token };
            let json = serde_json::to_string(&reply)?;
            tx.send(Message::Text(json)).await?;
            
            // Update metrics
            counter!("meet.created", 1);
        }
        
        ClientToServer::JoinMeet { meet_id, password, location_name } => {
            // Get meet info
            let meet_info = state.storage.get_meet_info(&meet_id).await?;
            
            // Verify password
            if !verify_password(&meet_info.password_hash, &password) {
                let err = ServerToClient::JoinRejected { reason: "Invalid password".to_string() };
                let json = serde_json::to_string(&err)?;
                tx.send(Message::Text(json)).await?;
                return Ok(());
            }
            
            // Find priority for this location
            let priority = meet_info.endpoints.iter()
                .find(|e| e.location_name == location_name)
                .map(|e| e.priority)
                .unwrap_or(0);
            
            // Create session
            let session_token = state.auth.new_session(meet_id.clone(), location_name, priority).await;
            
            // Send response
            let reply = ServerToClient::Joined { session_token };
            let json = serde_json::to_string(&reply)?;
            tx.send(Message::Text(json)).await?;
            
            // Update metrics
            counter!("meet.joined", 1);
        }
        
        ClientToServer::Update { session_token, updates } => {
            // Validate session
            if !state.auth.validate_session(&session_token).await {
                let err = ServerToClient::UpdateRejected { reason: "Invalid session".to_string() };
                let json = serde_json::to_string(&err)?;
                tx.send(Message::Text(json)).await?;
                return Ok(());
            }
            
            // Get session info
            let session = state.auth.get(&session_token).await
                .ok_or_else(|| AppError::Auth("Session not found".to_string()))?;
            
            // Get meet handle
            let meet_handle = state.meets.get_meet(&session.meet_id)
                .ok_or_else(|| AppError::MeetNotFound)?;
            
            // Send update to actor
            let (resp_tx, mut resp_rx) = tokio::sync::mpsc::unbounded_channel();
            meet_handle.cmd_tx.send(crate::meet_actor::ActorMsg::Update {
                client_id: session.location_name,
                priority: session.priority,
                updates,
                resp_tx,
            })?;
            
            // Wait for response
            let result = resp_rx.recv().await
                .ok_or_else(|| AppError::Internal("Actor disconnected".to_string()))??;
            
            // Send response
            let reply = ServerToClient::UpdateAccepted { seqs: result };
            let json = serde_json::to_string(&reply)?;
            tx.send(Message::Text(json)).await?;
            
            // Update metrics
            counter!("update.accepted", 1);
            histogram!("update.batch_size", updates.len() as f64);
        }
        
        ClientToServer::Pull { session_token, since } => {
            // Validate session
            if !state.auth.validate_session(&session_token).await {
                let err = ServerToClient::PullRejected { reason: "Invalid session".to_string() };
                let json = serde_json::to_string(&err)?;
                tx.send(Message::Text(json)).await?;
                return Ok(());
            }
            
            // Get session info
            let session = state.auth.get(&session_token).await
                .ok_or_else(|| AppError::Auth("Session not found".to_string()))?;
            
            // Get meet handle
            let meet_handle = state.meets.get_meet(&session.meet_id)
                .ok_or_else(|| AppError::MeetNotFound)?;
            
            // Send pull request to actor
            let (resp_tx, mut resp_rx) = tokio::sync::mpsc::unbounded_channel();
            meet_handle.cmd_tx.send(crate::meet_actor::ActorMsg::Pull {
                since,
                resp_tx,
            })?;
            
            // Wait for response
            let updates = resp_rx.recv().await
                .ok_or_else(|| AppError::Internal("Actor disconnected".to_string()))??;
            
            // Send response
            let reply = ServerToClient::Updates { updates };
            let json = serde_json::to_string(&reply)?;
            tx.send(Message::Text(json)).await?;
            
            // Update metrics
            counter!("pull.accepted", 1);
            histogram!("pull.updates_count", updates.len() as f64);
        }
    };
    
    // Record handler duration
    let duration = start.elapsed();
    histogram!("handler.duration_ms", duration.as_millis() as f64);
    
    result
} 