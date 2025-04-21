// ============================
// openlifter-backend-lib/src/meet.rs
// ============================
//! Meet management and actor coordination.
use crate::{
    meet_actor::{spawn_meet_actor, MeetHandle},
    storage::Storage,
};
use dashmap::DashMap;
use metrics::{counter, gauge};
use std::sync::Arc;

/// Manager for live meets
#[derive(Clone)]
pub struct MeetManager {
    meets: Arc<DashMap<String, MeetHandle>>,
}

impl Default for MeetManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MeetManager {
    /// Create a new meet manager
    pub fn new() -> Self {
        MeetManager {
            meets: Arc::new(DashMap::new()),
        }
    }

    /// Create a new meet and store its handle
    pub async fn create_meet(
        &self,
        meet_id: String,
        storage: impl Storage + 'static,
    ) -> MeetHandle {
        let handle = spawn_meet_actor(&meet_id, storage).await;
        self.meets.insert(meet_id.clone(), handle.clone());

        // Update metrics
        let _ = counter!("meet.created", &[("value", "1")]);
        let _ = gauge!("meet.active", &[("value", "1")]);

        handle
    }

    /// Get a meet handle by ID
    pub fn get_meet(&self, meet_id: &str) -> Option<MeetHandle> {
        self.meets.get(meet_id).map(|h| h.clone())
    }

    /// Delete a meet
    pub fn delete_meet(&self, meet_id: &str) -> bool {
        if self.meets.remove(meet_id).is_some() {
            // Update metrics
            let _ = counter!("meet.deleted", &[("value", "1")]);
            let _ = gauge!("meet.active", &[("value", "-1")]);
            true
        } else {
            false
        }
    }

    /// Get all active meet IDs
    pub fn get_all_meet_ids(&self) -> Vec<String> {
        self.meets.iter().map(|entry| entry.key().clone()).collect()
    }
}
