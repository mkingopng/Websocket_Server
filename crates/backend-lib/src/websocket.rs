use std::sync::Arc;
use tokio::sync::mpsc;
use anyhow::Result;
use uuid::Uuid;
use crate::AppState;
use crate::messages::{ClientMessage, ServerMessage, Update, UpdateWithMetadata};
use crate::storage::Storage;

pub struct WebSocketHandler<S: Storage + Send + Sync + 'static> {
    state: Arc<AppState<S>>,
    client_id: String,
    client_tx: Option<mpsc::Sender<ServerMessage>>,
}

impl<S: Storage + Send + Sync + 'static> WebSocketHandler<S> {
    pub fn new(state: Arc<AppState<S>>) -> Self {
        Self { 
            state, 
            client_id: Uuid::new_v4().to_string(),
            client_tx: None,
        }
    }
    
    // Register this client for a specific meet
    pub fn register_client(&mut self, meet_id: &str, tx: mpsc::Sender<ServerMessage>) {
        // Store the client's transmission channel
        self.client_tx = Some(tx.clone());
        
        // Add client to the clients map for the meet
        let mut meet_clients = self.state.clients
            .entry(meet_id.to_string())
            .or_insert_with(Vec::new);
            
        meet_clients.push(tx);
        
        println!("Client {} registered for meet {}", self.client_id, meet_id);
    }
    
    // Unregister this client when disconnecting
    pub fn unregister_client(&self, meet_id: &str) {
        if let Some(client_tx) = &self.client_tx {
            if let Some(mut clients) = self.state.clients.get_mut(meet_id) {
                // Remove this client from the list
                clients.retain(|tx| !std::ptr::eq(tx, client_tx));
                println!("Client {} unregistered from meet {}", self.client_id, meet_id);
            }
        }
    }
    
    // Broadcast updates to all connected clients for a meet
    async fn broadcast_update(&self, meet_id: &str, updates: Vec<Update>) -> Result<()> {
        // Check if we have clients for this meet
        if let Some(clients) = self.state.clients.get(meet_id) {
            if clients.len() <= 1 {
                // No other clients to broadcast to
                return Ok(());
            }
            
            // Create metadata for each update
            let updates_with_metadata: Vec<UpdateWithMetadata> = updates.into_iter()
                .enumerate()
                .map(|(idx, update)| {
                    UpdateWithMetadata {
                        update,
                        source_client: self.client_id.clone(),
                        server_seq: idx as u64 + 1, // Simple sequential numbering for now
                    }
                })
                .collect();
            
            // Create the relay message
            let relay_msg = ServerMessage::UpdateRelay {
                meet_id: meet_id.to_string(),
                updates: updates_with_metadata,
            };
            
            // Send to all connected clients except ourselves
            for client in clients.iter() {
                if self.client_tx.as_ref().map_or(true, |tx| !std::ptr::eq(tx, client)) {
                    if let Err(e) = client.send(relay_msg.clone()).await {
                        println!("Failed to relay update to client: {}", e);
                    }
                }
            }
        }
        
        Ok(())
    }

    pub async fn handle_message(&self, msg: ClientMessage) -> Result<ServerMessage> {
        match msg {
            ClientMessage::CreateMeet { meet_id, password: _ } => {
                // Handle meet creation
                let session = self.state.auth.new_session(meet_id.clone(), "default".to_string(), 0).await;
                
                // Return create response
                Ok(ServerMessage::MeetCreated {
                    meet_id,
                    session_token: session,
                })
            }
            ClientMessage::JoinMeet { meet_id, password: _ } => {
                // Handle meet joining
                let session = self.state.auth.new_session(meet_id.clone(), "default".to_string(), 0).await;
                
                // Return join response
                Ok(ServerMessage::MeetJoined {
                    meet_id,
                    session_token: session,
                })
            }
            ClientMessage::UpdateInit { meet_id, session_token, updates } => {
                if self.state.auth.validate_session(&session_token).await {
                    // Generate update IDs
                    let update_ids: Vec<String> = updates
                        .iter()
                        .map(|_| Uuid::new_v4().to_string())
                        .collect();
                        
                    // Broadcast updates to other clients
                    if !updates.is_empty() {
                        self.broadcast_update(&meet_id, updates.clone()).await?;
                    }
                    
                    // Return ack response
                    Ok(ServerMessage::UpdateAck {
                        meet_id,
                        update_ids,
                    })
                } else {
                    // Return error if session is invalid
                    Ok(ServerMessage::Error {
                        code: "INVALID_SESSION".to_string(),
                        message: "Invalid or expired session token".to_string(),
                    })
                }
            }
        }
    }
} 