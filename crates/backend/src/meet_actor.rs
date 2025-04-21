// ============================
// openlifter-backend/src/meet_actor.rs
// ============================
use std::collections::{HashMap, BTreeMap};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{mpsc, broadcast};
use serde::{Serialize, Deserialize};
use openlifter_common::{Seq, Update, UpdateWithServerSeq};
use crate::{storage::FlatFileStorage, error::AppError};
use uuid::Uuid;
use serde_json::Value;

use crate::AppState;

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
        
        let storage = FlatFileStorage::new("data").expect("Failed to initialize storage");
        let actor = MeetActor::new(meet_id, storage, relay_tx.clone());
        
        tokio::spawn(actor.run(cmd_rx));
        
        MeetHandle {
            cmd_tx,
            relay_tx,
        }
    }
}

/// Represents a state update with metadata
#[derive(Clone, Debug)]
struct StateUpdate {
    update: UpdateWithServerSeq,
    client_id: ClientId,
    priority: u8,
}

pub struct MeetActor {
    meet_id: String,
    storage: FlatFileStorage,
    state: Value,
    updates: Vec<UpdateWithServerSeq>,
    server_seq: u64,
    updates_by_key: HashMap<String, UpdateWithServerSeq>,
    tx_relay: broadcast::Sender<UpdateWithServerSeq>,
}

impl MeetActor {
    pub fn new(meet_id: String, storage: FlatFileStorage, tx_relay: broadcast::Sender<UpdateWithServerSeq>) -> Self {
        MeetActor {
            meet_id,
            storage,
            state: Value::Object(serde_json::Map::new()),
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
        let mut acks = Vec::new();
        
        for update in updates {
            let server_seq = self.server_seq;
            self.server_seq += 1;
            
            let update_with_seq = UpdateWithServerSeq {
                update: update.clone(),
                server_seq_num: server_seq,
            };
            
            // Check for conflicts
            if let Some(existing) = self.updates_by_key.get(&update.update_key) {
                if update.after_server_seq_num < existing.server_seq_num {
                    // Conflict detected - use the priority directly for comparison
                    if priority > 0 {  // Higher priority client wins
                        self.apply_update(&update_with_seq)?;
                        self.updates_by_key.insert(update.update_key.clone(), update_with_seq.clone());
                        acks.push((update.local_seq_num, server_seq));
                        let _ = self.tx_relay.send(update_with_seq);
                    }
                } else {
                    // No conflict, apply the update
                    self.apply_update(&update_with_seq)?;
                    self.updates_by_key.insert(update.update_key.clone(), update_with_seq.clone());
                    acks.push((update.local_seq_num, server_seq));
                    let _ = self.tx_relay.send(update_with_seq);
                }
            } else {
                // No existing update for this key
                self.apply_update(&update_with_seq)?;
                self.updates_by_key.insert(update.update_key.clone(), update_with_seq.clone());
                acks.push((update.local_seq_num, server_seq));
                let _ = self.tx_relay.send(update_with_seq);
            }
        }
        
        Ok(acks)
    }

    pub fn get_updates_since(&self, since: u64) -> Vec<UpdateWithServerSeq> {
        self.updates.iter()
            .filter(|u| u.server_seq_num > since)
            .cloned()
            .collect()
    }

    fn apply_update(&mut self, update: &UpdateWithServerSeq) -> Result<(), AppError> {
        if let Value::Object(state_map) = &mut self.state {
            if let Some(update_map) = update.update.update_value.as_object() {
                for (key, value) in update_map {
                    state_map.insert(key.clone(), value.clone());
                }
            }
        }
        
        self.updates.push(update.clone());
        Ok(())
    }

    pub fn get_state(&self) -> Value {
        self.state.clone()
    }

    pub async fn run(mut self, mut rx: mpsc::UnboundedReceiver<ActorMsg>) {
        while let Some(msg) = rx.recv().await {
            match msg {
                ActorMsg::Update { client_id, priority, updates, resp_tx } => {
                    let _ = resp_tx.send(self.handle_update(client_id, priority, updates).await);
                }
                ActorMsg::Pull { since, resp_tx } => {
                    let _ = resp_tx.send(Ok(self.get_updates_since(since)));
                }
            }
        }
    }
}

pub async fn spawn_meet_actor(meet_id: &str, state: &AppState) -> MeetHandle {
    let (cmd_tx, rx_cmd) = mpsc::unbounded_channel();
    let (relay_tx, _) = broadcast::channel(32);
    let storage = (*state.storage).clone();
    let actor = MeetActor::new(meet_id.to_string(), storage, relay_tx.clone());
    
    tokio::spawn(async move { 
        actor.run(rx_cmd).await; 
    });
    
    MeetHandle { cmd_tx, relay_tx }
}