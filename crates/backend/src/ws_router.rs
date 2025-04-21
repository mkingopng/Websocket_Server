// ============================
// openlifter-backend/src/ws_router.rs
// ============================
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade, Utf8Bytes},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use openlifter_common::{ClientToServer, ServerToClient};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, broadcast};
use crate::{AppState, error::AppError, meet_actor::{ActorMsg, MeetHandle}};
use rand::Rng;
use std::sync::Arc;

pub fn router(app_state: AppState) -> Router<AppState> {
    Router::new().route("/ws", get(ws_handler)).with_state(app_state)
}

fn text(s: String) -> Message {
    Message::Text(Utf8Bytes::from(s))
}

#[axum::debug_handler]
async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = mpsc::channel(32);

    // Handle incoming messages
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let Message::Text(msg_text) = msg {
                match serde_json::from_str::<ClientToServer>(&msg_text.to_string()) {
                    Ok(client_msg) => {
                        match handle_client_message(client_msg, &state, tx.clone()).await {
                            Ok(_) => (),
                            Err(e) => {
                                let err = ServerToClient::MalformedMessage { err_msg: e.to_string() };
                                if let Ok(json) = serde_json::to_string(&err) {
                                    let _ = tx.send(text(json)).await;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let err = ServerToClient::MalformedMessage { err_msg: format!("Invalid message format: {}", e) };
                        if let Ok(json) = serde_json::to_string(&err) {
                            let _ = tx.send(text(json)).await;
                        }
                    }
                }
            }
        }
    });

    // Handle outgoing messages
    let mut send_task = tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            if sender.send(message).await.is_err() {
                break;
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = (&mut recv_task) => send_task.abort(),
        _ = (&mut send_task) => recv_task.abort(),
    };
}

async fn handle_client_message(
    msg: ClientToServer,
    state: &AppState,
    tx: mpsc::Sender<Message>,
) -> Result<(), AppError> {
    match msg {
        ClientToServer::CreateMeet { this_location_name, password, endpoints } => {
            let meet_id = format!(
                "{}-{}-{}",
                rand::thread_rng().gen_range(100..1000),
                rand::thread_rng().gen_range(100..1000),
                rand::thread_rng().gen_range(100..1000)
            );
            
            let hashed_password = crate::auth::SessionManager::hash_password(&password)
                .map_err(|e| AppError::Internal(e.to_string()))?;
            
            state.storage.store_meet_info(&meet_id, &hashed_password, &endpoints).await?;
            
            // Create the meet actor
            let _handle = MeetHandle::new(meet_id.clone());
            state.meets.create_meet(meet_id.clone(), this_location_name.clone(), "owner".to_string());
            
            let session_token = state.auth.new_session(meet_id.clone(), this_location_name, endpoints[0].priority).await;
            
            let reply = ServerToClient::MeetCreated { meet_id, session_token };
            let json = serde_json::to_string(&reply)?;
            tx.send(text(json))
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?;
            Ok(())
        }
        
        ClientToServer::JoinMeet { meet_id, password, location_name } => {
            let meet_info = state.storage.get_meet_info(&meet_id).await?;
            
            if !crate::auth::SessionManager::verify_password(&meet_info.password_hash, &password) {
                let err = ServerToClient::JoinRejected { reason: "Invalid password".to_string() };
                let json = serde_json::to_string(&err)?;
                tx.send(text(json))
                    .await
                    .map_err(|e| AppError::Internal(e.to_string()))?;
                return Ok(());
            }
            
            let priority = meet_info.endpoints.iter()
                .find(|e| e.location_name == location_name)
                .map(|e| e.priority)
                .unwrap_or(0);
            
            let session_token = state.auth.new_session(meet_id.clone(), location_name, priority).await;
            
            let reply = ServerToClient::MeetJoined { session_token };
            let json = serde_json::to_string(&reply)?;
            tx.send(text(json))
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?;
                
            // Start relay task
            if let Some(meet_handle) = state.meets.get_meet(&meet_id) {
                let mut relay_rx = meet_handle.relay_tx.subscribe();
                let mut tx_clone = tx.clone();
                tokio::spawn(async move {
                    while let Ok(update) = relay_rx.recv().await {
                        let msg = ServerToClient::UpdateRelay { updates_relayed: vec![update] };
                        if let Ok(json) = serde_json::to_string(&msg) {
                            let _ = tx_clone.send(text(json)).await;
                        }
                    }
                });
            }
            
            Ok(())
        }
        
        ClientToServer::UpdateInit { session_token, updates } => {
            let sess = state.auth.get(&session_token).await.ok_or_else(|| AppError::Auth("Invalid session".into()))?;
            let handle = state.meets.get_meet(&sess.meet_id).ok_or_else(|| AppError::Internal("Meet not found".into()))?;
            
            let (resp_tx, mut resp_rx) = mpsc::unbounded_channel();
            handle.cmd_tx.send(ActorMsg::Update { 
                client_id: sess.meet_id.clone(), 
                priority: sess.priority, 
                updates, 
                resp_tx 
            }).map_err(|_| AppError::Internal("Actor gone".into()))?;
            
            if let Some(Ok(acks)) = resp_rx.recv().await {
                let reply = ServerToClient::UpdateAck { update_acks: acks };
                let json = serde_json::to_string(&reply)?;
                tx.send(text(json))
                    .await
                    .map_err(|e| AppError::Internal(e.to_string()))?;
            }
            Ok(())
        }
        
        ClientToServer::ClientPull { session_token, last_server_seq } => {
            let sess = state.auth.get(&session_token).await.ok_or_else(|| AppError::Auth("Invalid session".into()))?;
            let handle = state.meets.get_meet(&sess.meet_id).ok_or_else(|| AppError::Internal("Meet not found".into()))?;
            
            let (resp_tx, mut resp_rx) = mpsc::unbounded_channel();
            handle.cmd_tx.send(ActorMsg::Pull { 
                since: last_server_seq, 
                resp_tx 
            }).map_err(|_| AppError::Internal("Actor gone".into()))?;
            
            if let Some(Ok(updates)) = resp_rx.recv().await {
                let reply = ServerToClient::ServerPull { 
                    last_server_seq, 
                    updates_relayed: updates 
                };
                let json = serde_json::to_string(&reply)?;
                tx.send(text(json))
                    .await
                    .map_err(|e| AppError::Internal(e.to_string()))?;
            }
            Ok(())
        }
        
        ClientToServer::PublishMeet { session_token, return_email, opl_csv } => {
            let sess = state.auth.get(&session_token).await.ok_or_else(|| AppError::Auth("Invalid session".into()))?;
            
            state.storage.store_meet_csv(&sess.meet_id, &opl_csv, &return_email).await?;
            state.storage.archive_meet(&sess.meet_id).await?;
            state.meets.delete_meet(&sess.meet_id);
            
            let reply = ServerToClient::PublishAck;
            let json = serde_json::to_string(&reply)?;
            tx.send(text(json))
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?;
            Ok(())
        }
    }
} 