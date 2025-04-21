// ============================
// openlifter-backend-lib/src/meet_actor.rs
// ============================
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{mpsc, broadcast};
use serde::{Serialize, Deserialize};
use openlifter_common::{Seq, Update, UpdateWithServerSeq};
use crate::{storage::Storage, error::AppError, storage::FlatFileStorage};
use uuid::Uuid;
use serde_json::Value;
use metrics::{counter, histogram};

pub type ClientId = Uuid;

/// Message sent *into* the actor
#[derive(Debug)]
pub enum ActorMsg {
    Update {
        client_id: String,
        priority: u8,
        updates: Vec<Update>,
        resp_tx: mpsc::UnboundedSender<Result<Vec<(u64, u64)>, AppError>>,
    },
    Pull {
        since: u64,
        resp_tx: mpsc::UnboundedSender<Result<Vec<UpdateWithServerSeq>, AppError>>,
    },
    StoreCsv {
        opl_csv: String,
        return_email: String,
        resp_tx: mpsc::UnboundedSender<Result<(), AppError>>,
    },
}

/// Handle that other components keep: command channel + broadcast sender
#[derive(Clone)]
pub struct MeetHandle {
    pub cmd_tx: mpsc::UnboundedSender<ActorMsg>,
    pub relay_tx: broadcast::Sender<UpdateWithServerSeq>,
}

impl MeetHandle {
    pub fn new(meet_id: String) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (relay_tx, _) = broadcast::channel(100);
        
        let storage = crate::storage::FlatFileStorage::new("data").expect("Failed to initialize storage");
        let actor = MeetActor::new(meet_id, storage, relay_tx.clone());
        
        tokio::spawn(actor.run(cmd_rx));
        
        MeetHandle {
            cmd_tx,
            relay_tx,
        }
    }

    pub async fn apply_updates(
        &self,
        client_id: String,
        priority: u8,
        updates: Vec<Update>,
    ) -> Result<Vec<(u64, u64)>, AppError> {
        let (resp_tx, mut resp_rx) = mpsc::unbounded_channel();
        
        self.cmd_tx.send(ActorMsg::Update {
            client_id,
            priority,
            updates,
            resp_tx,
        })?;
        
        resp_rx.recv().await.ok_or_else(|| AppError::Internal("Failed to receive response".to_string()))?
    }

    pub async fn get_updates_since(&self, since: u64) -> Result<Vec<UpdateWithServerSeq>, AppError> {
        let (resp_tx, mut resp_rx) = mpsc::unbounded_channel();
        
        self.cmd_tx.send(ActorMsg::Pull {
            since,
            resp_tx,
        })?;
        
        resp_rx.recv().await.ok_or_else(|| AppError::Internal("Failed to receive response".to_string()))?
    }

    pub async fn store_csv_data(&self, opl_csv: String, return_email: String) -> Result<(), AppError> {
        let (resp_tx, mut resp_rx) = mpsc::unbounded_channel();
        
        self.cmd_tx.send(ActorMsg::StoreCsv {
            opl_csv,
            return_email,
            resp_tx,
        })?;
        
        resp_rx.recv().await.ok_or_else(|| AppError::Internal("Failed to receive response".to_string()))?
    }
}

struct StateUpdate {
    update: UpdateWithServerSeq,
    client_id: ClientId,
    priority: u8,
}

pub struct MeetActor {
    meet_id: String,
    storage: Box<dyn Storage + Send + Sync>,
    state: Value,
    updates: Vec<UpdateWithServerSeq>,
    server_seq: u64,
    updates_by_key: HashMap<String, UpdateWithServerSeq>,
    tx_relay: broadcast::Sender<UpdateWithServerSeq>,
}

impl MeetActor {
    pub fn new(meet_id: String, storage: impl Storage + Send + Sync + 'static, tx_relay: broadcast::Sender<UpdateWithServerSeq>) -> Self {
        MeetActor {
            meet_id,
            storage: Box::new(storage),
            state: serde_json::json!({}),
            updates: Vec::new(),
            server_seq: 0,
            updates_by_key: HashMap::new(),
            tx_relay,
        }
    }

    pub async fn handle_update(
        &mut self,
        client_id: String,
        priority: u8,
        updates: Vec<Update>,
    ) -> Result<Vec<(u64, u64)>, AppError> {
        let mut results = Vec::new();
        
        let updates_len = updates.len();
        for update in updates {
            self.server_seq += 1;
            let seq = self.server_seq;
            
            let update_with_seq = UpdateWithServerSeq {
                update: update.clone(),
                server_seq_num: seq,
            };
            
            // Apply the update to our state
            self.apply_update(&update_with_seq)?;
            
            // Store in our map of updates by key
            self.updates_by_key.insert(update.update_key.clone(), update_with_seq.clone());
            
            // Add to our list of updates
            self.updates.push(update_with_seq.clone());
            
            // Store in persistent storage
            let json = serde_json::to_string(&update_with_seq)?;
            self.storage.append_update(&self.meet_id, &json).await?;
            
            // Broadcast to all connected clients
            let _ = self.tx_relay.send(update_with_seq);
            
            results.push((seq, seq));
        }
        
        // Update metrics
        counter!("meet.updates", 1, "meet_id" => self.meet_id.clone());
        histogram!("meet.update.batch_size", updates_len as f64, "meet_id" => self.meet_id.clone());
        
        Ok(results)
    }

    pub fn get_updates_since(&self, since: u64) -> Vec<UpdateWithServerSeq> {
        self.updates.iter()
            .filter(|u| u.server_seq_num > since)
            .cloned()
            .collect()
    }

    fn apply_update(&mut self, update: &UpdateWithServerSeq) -> Result<(), AppError> {
        // Apply the update to our state
        // This is a simplified version - in a real app, you'd have more complex state management
        if let Some(obj) = self.state.as_object_mut() {
            obj.insert(update.update.update_key.clone(), update.update.update_value.clone());
        } else {
            self.state = serde_json::json!({
                update.update.update_key.clone(): update.update.update_value.clone()
            });
        }
        
        Ok(())
    }

    pub fn get_state(&self) -> Value {
        self.state.clone()
    }

    pub async fn run(mut self, mut rx: mpsc::UnboundedReceiver<ActorMsg>) {
        while let Some(msg) = rx.recv().await {
            match msg {
                ActorMsg::Update { client_id, priority, updates, resp_tx } => {
                    let result = self.handle_update(client_id, priority, updates).await;
                    let _ = resp_tx.send(result);
                }
                ActorMsg::Pull { since, resp_tx } => {
                    let updates = self.get_updates_since(since);
                    let _ = resp_tx.send(Ok(updates));
                }
                ActorMsg::StoreCsv { opl_csv, return_email, resp_tx } => {
                    let result = self.store_csv_data(opl_csv, return_email).await;
                    let _ = resp_tx.send(result);
                }
            }
        }
    }

    pub async fn store_csv_data(&self, opl_csv: String, return_email: String) -> Result<(), AppError> {
        // Store CSV data
        self.storage.store_meet_csv(&self.meet_id, &opl_csv, &return_email).await?;
        
        // Update metrics
        counter!("meet.published", 1, "meet_id" => self.meet_id.clone());
        histogram!("meet.csv_size", opl_csv.len() as f64);
        
        Ok(())
    }
}

/// Spawn a new meet actor and return its handle
pub async fn spawn_meet_actor(meet_id: &str, storage: impl Storage + Send + Sync + 'static) -> MeetHandle {
    let (cmd_tx, rx_cmd) = mpsc::unbounded_channel();
    let (relay_tx, _) = broadcast::channel(32);
    let actor = MeetActor::new(meet_id.to_string(), storage, relay_tx.clone());
    
    tokio::spawn(async move { 
        actor.run(rx_cmd).await; 
    });
    
    MeetHandle { cmd_tx, relay_tx }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;
    use tempfile::TempDir;

    async fn setup() -> (MeetActor, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let storage = FlatFileStorage::new(temp_dir.path()).unwrap();
        let (relay_tx, _) = broadcast::channel(100);
        let actor = MeetActor::new("test-meet".to_string(), storage, relay_tx);
        (actor, temp_dir)
    }

    #[tokio::test]
    async fn test_handle_update() {
        let (mut actor, _temp_dir) = setup().await;
        let client_id = "client1".to_string();
        let priority = 1;
        let updates = vec![Update {
            update_key: "test.key".to_string(),
            update_value: serde_json::json!("value"),
            local_seq_num: 1,
            after_server_seq_num: 0,
        }];

        let result = actor.handle_update(client_id, priority, updates).await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, 1); // server_seq
        assert_eq!(result[0].1, 1); // local_seq
    }

    #[tokio::test]
    async fn test_get_updates_since() {
        let (mut actor, _temp_dir) = setup().await;
        
        // Add some updates
        let updates = vec![
            Update {
                update_key: "key1".to_string(),
                update_value: serde_json::json!("value1"),
                local_seq_num: 1,
                after_server_seq_num: 0,
            },
            Update {
                update_key: "key2".to_string(),
                update_value: serde_json::json!("value2"),
                local_seq_num: 2,
                after_server_seq_num: 1,
            },
        ];

        actor.handle_update("client1".to_string(), 1, updates).await.unwrap();

        // Get updates since seq 1
        let updates = actor.get_updates_since(1);
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].server_seq_num, 2);
    }

    #[tokio::test]
    async fn test_apply_update() {
        let (mut actor, _temp_dir) = setup().await;
        
        let update = UpdateWithServerSeq {
            update: Update {
                update_key: "test.key".to_string(),
                update_value: serde_json::json!("value"),
                local_seq_num: 1,
                after_server_seq_num: 0,
            },
            server_seq_num: 1,
        };

        actor.apply_update(&update).unwrap();
        
        let state = actor.get_state();
        assert_eq!(state["test.key"], "value");
    }

    #[tokio::test]
    async fn test_meet_handle() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FlatFileStorage::new(temp_dir.path()).unwrap();
        let handle = MeetHandle::new("test-meet".to_string());

        // Verify channels are created
        assert!(handle.cmd_tx.send(ActorMsg::Pull {
            since: 0,
            resp_tx: mpsc::unbounded_channel().0,
        }).is_ok());
    }
} 