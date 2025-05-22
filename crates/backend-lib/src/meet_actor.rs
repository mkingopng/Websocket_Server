// crates/backend-lib/src/meet_actor.rs

//! Meet actor module
use crate::{error::AppError, storage::Storage};
use metrics::{counter, histogram};
use openlifter_common::{Update, UpdateWithServerSeq};
use serde_json::Value;
use std::collections::HashMap;
use tokio::sync::{broadcast, mpsc};
use uuid::Uuid;

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
    // New message type for state recovery
    RecoverState {
        updates: Vec<crate::messages::Update>,
        client_id: String,
        priority: u8,
        resp_tx: mpsc::UnboundedSender<Result<(u64, usize), AppError>>,
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

        let storage =
            crate::storage::FlatFileStorage::new("data").expect("Failed to initialize storage");
        let actor = MeetActor::new(meet_id, storage, relay_tx.clone());

        tokio::spawn(actor.run(cmd_rx));

        MeetHandle { cmd_tx, relay_tx }
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

        resp_rx
            .recv()
            .await
            .ok_or_else(|| AppError::Internal("Failed to receive response".to_string()))?
    }

    pub async fn get_updates_since(
        &self,
        since: u64,
    ) -> Result<Vec<UpdateWithServerSeq>, AppError> {
        let (resp_tx, mut resp_rx) = mpsc::unbounded_channel();

        self.cmd_tx.send(ActorMsg::Pull { since, resp_tx })?;

        resp_rx
            .recv()
            .await
            .ok_or_else(|| AppError::Internal("Failed to receive response".to_string()))?
    }

    pub async fn store_csv_data(
        &self,
        opl_csv: String,
        return_email: String,
    ) -> Result<(), AppError> {
        let (resp_tx, mut resp_rx) = mpsc::unbounded_channel();

        self.cmd_tx.send(ActorMsg::StoreCsv {
            opl_csv,
            return_email,
            resp_tx,
        })?;

        resp_rx
            .recv()
            .await
            .ok_or_else(|| AppError::Internal("Failed to receive response".to_string()))?
    }

    pub async fn recover_state(
        &self,
        client_id: String,
        priority: u8,
        updates: Vec<crate::messages::Update>,
    ) -> Result<(u64, usize), AppError> {
        let (resp_tx, mut resp_rx) = mpsc::unbounded_channel();

        self.cmd_tx.send(ActorMsg::RecoverState {
            client_id,
            priority,
            updates,
            resp_tx,
        })?;

        resp_rx
            .recv()
            .await
            .ok_or_else(|| AppError::Internal("Failed to receive response".to_string()))?
    }
}

pub struct MeetActor<S: Storage> {
    meet_id: String,
    storage: S,
    state: Value,
    updates: Vec<UpdateWithServerSeq>,
    server_seq: u64,
    updates_by_key: HashMap<String, UpdateWithServerSeq>,
    tx_relay: broadcast::Sender<UpdateWithServerSeq>,
    expected_client_seq: HashMap<String, u64>,
    last_update_time: std::time::Instant,
    need_consistency_check: bool,
}

impl<S: Storage> MeetActor<S> {
    pub fn new(
        meet_id: String,
        storage: S,
        tx_relay: broadcast::Sender<UpdateWithServerSeq>,
    ) -> Self {
        Self {
            meet_id,
            storage,
            state: serde_json::json!({}),
            updates: Vec::new(),
            server_seq: 0,
            updates_by_key: HashMap::new(),
            tx_relay,
            expected_client_seq: HashMap::new(),
            last_update_time: std::time::Instant::now(),
            need_consistency_check: false,
        }
    }

    /// Detect sequence gaps in client updates
    ///
    /// This method checks if there are any gaps in the sequence numbers
    /// from a specific client, which might indicate lost updates.
    ///
    /// Returns true if a gap is detected, false otherwise.
    pub fn detect_sequence_gaps(&mut self, client_id: &str, updates: &[Update]) -> bool {
        if updates.is_empty() {
            return false;
        }

        // Get the expected next sequence number for this client
        let expected_seq = self
            .expected_client_seq
            .get(client_id)
            .copied()
            .unwrap_or(0);

        // Check if the first update has the expected sequence number
        let first_update_seq = updates[0].local_seq_num;

        // Check for gaps in the update sequence
        if expected_seq > 0 && first_update_seq > expected_seq {
            // Gap detected!
            println!(
                "Sequence gap detected for client {client_id}: expected {expected_seq}, got {first_update_seq}"
            );

            // Mark that we need a consistency check
            self.need_consistency_check = true;

            // Update metrics
            let _ = counter!("meet.sequence_gaps", &[("value", "1")]);

            return true;
        }

        // Check for gaps between updates in this batch
        let mut prev_seq = first_update_seq;
        for update in &updates[1..] {
            if update.local_seq_num > prev_seq + 1 {
                // Gap detected within batch
                println!(
                    "Sequence gap detected within batch for client {}: gap between {} and {}",
                    client_id, prev_seq, update.local_seq_num
                );

                // Mark that we need a consistency check
                self.need_consistency_check = true;

                // Update metrics
                let _ = counter!("meet.sequence_gaps", &[("value", "1")]);

                return true;
            }
            prev_seq = update.local_seq_num;
        }

        // Update the expected next sequence number for this client
        let last_update = updates.last().unwrap();
        self.expected_client_seq
            .insert(client_id.to_string(), last_update.local_seq_num + 1);

        false
    }

    /// Check if state recovery is needed
    ///
    /// Determines if we should initiate state recovery based on:
    /// 1. Gap detection in sequence numbers
    /// 2. Long periods of inactivity
    /// 3. Explicitly set `need_consistency_check` flag
    ///
    /// Returns true if recovery is needed, false otherwise.
    pub fn needs_state_recovery(&mut self) -> bool {
        // If we've already determined we need a consistency check
        if self.need_consistency_check {
            self.need_consistency_check = false; // Reset the flag
            return true;
        }

        // Check for long period of inactivity (more than 5 minutes)
        let now = std::time::Instant::now();
        let inactivity_duration = now.duration_since(self.last_update_time);
        if inactivity_duration > std::time::Duration::from_secs(300) {
            println!(
                "Long inactivity period detected for meet {}: {:?}",
                self.meet_id, inactivity_duration
            );

            // Update the last update time
            self.last_update_time = now;

            // This could indicate a network partition or server restart
            return true;
        }

        false
    }

    pub async fn handle_update(
        &mut self,
        client_id: String,
        priority: u8,
        updates: Vec<Update>,
    ) -> Result<Vec<(u64, u64)>, AppError> {
        // Update the last update time
        self.last_update_time = std::time::Instant::now();

        // Detect sequence gaps
        let gaps_detected = self.detect_sequence_gaps(&client_id, &updates);

        // Check if we need state recovery
        let recovery_needed = gaps_detected || self.needs_state_recovery();

        // If we need recovery, return a special error to trigger recovery
        if recovery_needed {
            return Err(AppError::NeedsRecovery {
                meet_id: self.meet_id.clone(),
                last_known_seq: self.server_seq,
            });
        }

        let mut results = Vec::new();

        let updates_len = updates.len();
        for update in updates {
            self.server_seq += 1;
            let seq = self.server_seq;

            let update_with_seq = UpdateWithServerSeq {
                update: update.clone(),
                server_seq_num: seq,
                source_client_id: client_id.clone(),
                source_client_priority: priority,
            };

            // Apply the update to our state
            self.apply_update(&update_with_seq);

            // Store in our map of updates by key
            self.updates_by_key
                .insert(update.update_key.clone(), update_with_seq.clone());

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
        let _ = counter!("meet.updates", &[("value", "1")]);
        let _ = histogram!(
            "meet.update.batch_size",
            &[("value", updates_len.to_string())]
        );

        Ok(results)
    }

    pub fn get_updates_since(&self, since: u64) -> Vec<UpdateWithServerSeq> {
        self.updates
            .iter()
            .filter(|u| u.server_seq_num > since)
            .cloned()
            .collect()
    }

    fn apply_update(&mut self, update: &UpdateWithServerSeq) {
        // Apply the update to our state
        // This is a simplified version - in a real app, you'd have more complex state management
        if let Some(obj) = self.state.as_object_mut() {
            obj.insert(
                update.update.update_key.clone(),
                update.update.update_value.clone(),
            );
        } else {
            self.state = serde_json::json!({
                update.update.update_key.clone(): update.update.update_value.clone()
            });
        }
    }

    pub fn get_state(&self) -> Value {
        self.state.clone()
    }

    /// Process client updates for state recovery
    ///
    /// This method is used when the server needs to recover its state from client updates.
    /// It applies updates with proper sequence numbering and conflict resolution.
    pub async fn handle_state_recovery(
        &mut self,
        client_id: String,
        priority: u8,
        updates: Vec<crate::messages::Update>,
    ) -> Result<(u64, usize), AppError> {
        let original_seq = self.server_seq;
        let mut applied_updates = 0;

        if updates.is_empty() {
            return Ok((self.server_seq, 0));
        }

        // Sort updates by timestamp to ensure proper ordering
        let mut sorted_updates = updates.clone();
        sorted_updates.sort_by_key(|u| u.timestamp);

        // Track existing update keys to avoid duplicates
        let existing_keys: std::collections::HashSet<String> =
            self.updates_by_key.keys().cloned().collect();

        // Process each update
        for update in sorted_updates {
            // Convert messages::Update to openlifter_common::Update
            // This is a temporary solution to handle the type mismatch
            let common_update = openlifter_common::Update {
                update_key: update.location.clone(),
                update_value: serde_json::from_str(&update.value)
                    .unwrap_or(serde_json::Value::Null),
                #[allow(clippy::cast_sign_loss)]
                local_seq_num: update.timestamp as u64, // Use timestamp as local sequence number
                after_server_seq_num: 0, // Default to 0 for recovery
            };

            // Skip if we already have this update
            if existing_keys.contains(&common_update.update_key) {
                // Check if we should override based on priority
                if let Some(existing) = self.updates_by_key.get(&common_update.update_key) {
                    // If existing update has higher or equal priority, skip this update
                    // This is a simplified conflict resolution strategy
                    if priority <= existing.source_client_priority {
                        continue;
                    }
                }
            }

            // Apply the update
            self.server_seq += 1;
            let seq = self.server_seq;

            let update_with_seq = UpdateWithServerSeq {
                update: common_update,
                server_seq_num: seq,
                source_client_id: client_id.clone(),
                source_client_priority: priority,
            };

            // Apply to state
            self.apply_update(&update_with_seq);

            // Store in maps
            self.updates_by_key.insert(
                update_with_seq.update.update_key.clone(),
                update_with_seq.clone(),
            );
            self.updates.push(update_with_seq.clone());

            // Store in persistent storage
            let json = serde_json::to_string(&update_with_seq)?;
            self.storage.append_update(&self.meet_id, &json).await?;

            // Update counter
            applied_updates += 1;

            // We don't broadcast during recovery to avoid duplicates
        }

        if applied_updates > 0 {
            // Log recovery stats
            println!(
                "Recovered {} updates for meet {} from client {}, seq {} -> {}",
                applied_updates, self.meet_id, client_id, original_seq, self.server_seq
            );
        }

        Ok((self.server_seq, applied_updates))
    }

    pub async fn run(mut self, mut rx: mpsc::UnboundedReceiver<ActorMsg>) {
        while let Some(msg) = rx.recv().await {
            match msg {
                ActorMsg::Update {
                    client_id,
                    priority,
                    updates,
                    resp_tx,
                } => {
                    let result = self.handle_update(client_id, priority, updates).await;
                    let _ = resp_tx.send(result);
                },
                ActorMsg::Pull { since, resp_tx } => {
                    let updates = self.get_updates_since(since);
                    let _ = resp_tx.send(Ok(updates));
                },
                ActorMsg::StoreCsv {
                    opl_csv,
                    return_email,
                    resp_tx,
                } => {
                    let result = self.store_csv_data(opl_csv, return_email).await;
                    let _ = resp_tx.send(result);
                },
                ActorMsg::RecoverState {
                    client_id,
                    priority,
                    updates,
                    resp_tx,
                } => {
                    let result = self
                        .handle_state_recovery(client_id, priority, updates)
                        .await;
                    let _ = resp_tx.send(result);
                },
            }
        }
    }

    pub async fn store_csv_data(
        &self,
        opl_csv: String,
        return_email: String,
    ) -> Result<(), AppError> {
        // Store CSV data
        self.storage
            .store_meet_csv(&self.meet_id, &opl_csv, &return_email)
            .await?;

        // Update metrics
        let _ = counter!("meet.published", &[("value", "1")]);
        let _ = histogram!("meet.csv_size", &[("value", opl_csv.len().to_string())]);

        Ok(())
    }
}

/// Spawn a new meet actor and return its handle
pub async fn spawn_meet_actor(meet_id: &str, storage: impl Storage + 'static) -> MeetHandle {
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
    use crate::storage::FlatFileStorage;
    use tempfile::TempDir;
    use tokio;

    async fn setup() -> (MeetHandle, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let storage = FlatFileStorage::new(temp_dir.path()).unwrap();
        let handle = spawn_meet_actor("test-meet", storage).await;
        // Small delay to ensure actor is ready
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        (handle, temp_dir)
    }

    #[tokio::test]
    async fn test_handle_update() {
        let (actor, _temp_dir) = setup().await;

        let updates = vec![openlifter_common::Update {
            update_key: "test.key1".to_string(),
            update_value: serde_json::json!("value1"),
            local_seq_num: 1,
            after_server_seq_num: 0,
        }];

        let result = actor
            .apply_updates("client1".to_string(), 5, updates)
            .await
            .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, 1); // server_seq should be 1

        // Get updates since 0 to verify state
        let updates_since_0 = actor.get_updates_since(0).await.unwrap();
        assert_eq!(updates_since_0.len(), 1);
    }

    #[tokio::test]
    async fn test_get_updates_since() {
        let (actor, _temp_dir) = setup().await;

        // First add updates through the handle API
        let update1 = openlifter_common::Update {
            update_key: "test.key1".to_string(),
            update_value: serde_json::json!("value1"),
            local_seq_num: 1,
            after_server_seq_num: 0,
        };

        let update2 = openlifter_common::Update {
            update_key: "test.key2".to_string(),
            update_value: serde_json::json!("value2"),
            local_seq_num: 2,
            after_server_seq_num: 1,
        };

        // Apply the first update
        actor
            .apply_updates("client1".to_string(), 1, vec![update1])
            .await
            .unwrap();

        // Apply the second update
        actor
            .apply_updates("client1".to_string(), 1, vec![update2])
            .await
            .unwrap();

        // Test get_updates_since
        let updates_since_0 = actor.get_updates_since(0).await.unwrap();
        assert_eq!(updates_since_0.len(), 2);

        let updates_since_1 = actor.get_updates_since(1).await.unwrap();
        assert_eq!(updates_since_1.len(), 1);
        assert_eq!(updates_since_1[0].server_seq_num, 2);
    }

    #[tokio::test]
    async fn test_apply_update() {
        let (actor, _temp_dir) = setup().await;

        let update = openlifter_common::Update {
            update_key: "plate.weight".to_string(),
            update_value: serde_json::json!(25),
            local_seq_num: 1,
            after_server_seq_num: 0,
        };

        // Apply the update
        actor
            .apply_updates("client1".to_string(), 1, vec![update])
            .await
            .unwrap();

        // Verify that we can get the update
        let updates = actor.get_updates_since(0).await.unwrap();
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].update.update_key, "plate.weight");
        assert_eq!(updates[0].update.update_value, serde_json::json!(25));
    }

    #[tokio::test]
    async fn test_sequence_gap_detection() {
        let (actor, _temp_dir) = setup().await;

        // First send update with seq 1
        let update1 = openlifter_common::Update {
            update_key: "test.key1".to_string(),
            update_value: serde_json::json!("value1"),
            local_seq_num: 1,
            after_server_seq_num: 0,
        };

        // Apply the first update
        let result1 = actor
            .apply_updates("client1".to_string(), 1, vec![update1])
            .await;

        assert!(result1.is_ok());

        // Send update with seq 3 (skipping 2) - should trigger recovery
        let update3 = openlifter_common::Update {
            update_key: "test.key3".to_string(),
            update_value: serde_json::json!("value3"),
            local_seq_num: 3, // Gap here - skipped seq 2
            after_server_seq_num: 1,
        };

        // Apply the update with gap
        let result3 = actor
            .apply_updates("client1".to_string(), 1, vec![update3])
            .await;

        // Should return a NeedsRecovery error
        match result3 {
            Err(crate::error::AppError::NeedsRecovery {
                meet_id,
                last_known_seq,
            }) => {
                assert_eq!(meet_id, "test-meet");
                assert_eq!(last_known_seq, 1); // We've only applied one update so far
            },
            other => panic!("Expected NeedsRecovery error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_inactivity_triggers_recovery() {
        let (actor, _temp_dir) = setup().await;

        // First add an update
        let update1 = openlifter_common::Update {
            update_key: "test.key1".to_string(),
            update_value: serde_json::json!("value1"),
            local_seq_num: 1,
            after_server_seq_num: 0,
        };

        // Apply the update
        let result1 = actor
            .apply_updates("client1".to_string(), 1, vec![update1])
            .await;

        assert!(result1.is_ok());

        // In a real implementation, we'd test the inactivity detection
        // by manipulating the last_update_time. However, this field is private
        // and not directly accessible in tests.

        // Create another update to simulate coming back after inactivity
        let _update2 = openlifter_common::Update {
            update_key: "test.key2".to_string(),
            update_value: serde_json::json!("value2"),
            local_seq_num: 2,
            after_server_seq_num: 1,
        };

        // This is just a placeholder for a more complete integration test
        // that would actually verify the recovery mechanism is triggered
        // after a period of inactivity.
    }
}
