// ============================
// openlifter-backend-lib/src/meet.rs
// ============================
//! Meet management and actor coordination.
use dashmap::DashMap;
use std::sync::Arc;
use crate::meet_actor::{MeetHandle, spawn_meet_actor};
use crate::storage::Storage;
use metrics::{counter, gauge};

pub type MeetId = String;

/// Manager for all active meets
pub struct MeetManager {
    meets: Arc<DashMap<MeetId, MeetHandle>>,
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
        storage: impl Storage + Send + Sync + 'static
    ) -> MeetHandle {
        // Spawn the actor
        let handle = spawn_meet_actor(&meet_id, storage).await;
        
        // Store the handle in our map
        self.meets.insert(meet_id.clone(), handle.clone());
        
        // Update metrics
        counter!("meet.created", 1);
        gauge!("meet.active", self.meets.len() as f64);
        
        handle
    }
    
    /// Get a meet handle by ID
    pub fn get_meet(&self, meet_id: &str) -> Option<MeetHandle> {
        self.meets.get(meet_id).map(|entry| entry.value().clone())
    }
    
    /// Delete a meet
    pub fn delete_meet(&self, meet_id: &str) {
        if self.meets.remove(meet_id).is_some() {
            counter!("meet.deleted", 1);
            gauge!("meet.active", self.meets.len() as f64);
        }
    }
    
    /// Get all active meet IDs
    pub fn get_all_meet_ids(&self) -> Vec<String> {
        self.meets.iter().map(|entry| entry.key().clone()).collect()
    }
} 