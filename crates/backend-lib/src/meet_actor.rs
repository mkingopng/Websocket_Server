// ============================
// openlifter-backend-lib/src/meet_actor.rs
// ============================
use std::collections::{HashMap, BTreeMap};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{mpsc, broadcast};
use serde::{Serialize, Deserialize};
use openlifter_common::{Seq, Update, UpdateWithServerSeq};
use crate::{storage::Storage, error::AppError};
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
        
        for update in updates {
            self.server_seq += 1;
            let seq = self.server_seq;
            
            let update_with_seq = UpdateWithServerSeq {
                update: update.clone(),
                seq,
            };
            
            // Apply the update to our state
            self.apply_update(&update_with_seq)?;
            
            // Store in our map of updates by key
            self.updates_by_key.insert(update.key.clone(), update_with_seq.clone());
            
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
        histogram!("meet.update.batch_size", updates.len() as f64, "meet_id" => self.meet_id.clone());
        
        Ok(results)
    }

    pub fn get_updates_since(&self, since: u64) -> Vec<UpdateWithServerSeq> {
        self.updates.iter()
            .filter(|u| u.seq > since)
            .cloned()
            .collect()
    }

    fn apply_update(&mut self, update: &UpdateWithServerSeq) -> Result<(), AppError> {
        // Apply the update to our state
        // This is a simplified version - in a real app, you'd have more complex state management
        if let Some(obj) = self.state.as_object_mut() {
            obj.insert(update.update.key.clone(), serde_json::Value::String(update.update.value.clone()));
        } else {
            self.state = serde_json::json!({
                update.update.key: update.update.value
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
            }
        }
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