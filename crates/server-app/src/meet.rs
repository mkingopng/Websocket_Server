// ============================
// crates/server-app/src/error.rs
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Storage;
    use anyhow::Result;
    use async_trait::async_trait;

    // Mock storage for testing
    #[derive(Clone)]
    struct MockStorage;

    #[async_trait]
    impl Storage for MockStorage {
        async fn append_update(
            &self,
            _meet_id: &str,
            _value: &str,
        ) -> Result<(), crate::error::AppError> {
            Ok(())
        }

        async fn read_updates(
            &self,
            _meet_id: &str,
        ) -> Result<Vec<String>, crate::error::AppError> {
            Ok(vec![])
        }

        async fn archive_meet(&self, _meet_id: &str) -> Result<(), crate::error::AppError> {
            Ok(())
        }

        async fn store_meet_info(
            &self,
            _meet_id: &str,
            _password_hash: &str,
            _endpoints: &[openlifter_common::EndpointPriority],
        ) -> Result<(), crate::error::AppError> {
            Ok(())
        }

        async fn get_meet_info(
            &self,
            _meet_id: &str,
        ) -> Result<openlifter_common::MeetInfo, crate::error::AppError> {
            Ok(openlifter_common::MeetInfo {
                password_hash: "hashed_password".to_string(),
                endpoints: vec![],
            })
        }

        async fn store_meet_csv(
            &self,
            _meet_id: &str,
            _csv_data: &str,
            _email: &str,
        ) -> Result<(), crate::error::AppError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_meet_manager_create() {
        let manager = MeetManager::new();
        let storage = MockStorage;
        let meet_id = "test-meet-id".to_string();
        let _handle = manager.create_meet(meet_id.clone(), storage).await;

        assert!(manager.get_meet(&meet_id).is_some());
    }

    #[tokio::test]
    async fn test_meet_manager_get() {
        let manager = MeetManager::new();
        let storage = MockStorage;
        let meet_id = "test-meet-id".to_string();
        let _original_handle = manager.create_meet(meet_id.clone(), storage).await;

        let retrieved_handle = manager.get_meet(&meet_id);
        assert!(retrieved_handle.is_some());
    }

    #[tokio::test]
    async fn test_meet_manager_delete() {
        let manager = MeetManager::new();
        let storage = MockStorage;
        let meet_id = "test-meet-id".to_string();
        let _handle = manager.create_meet(meet_id.clone(), storage).await;

        assert!(manager.get_meet(&meet_id).is_some());

        let result = manager.delete_meet(&meet_id);
        assert!(result);
        assert!(manager.get_meet(&meet_id).is_none());
    }

    #[tokio::test]
    async fn test_meet_manager_delete_nonexistent() {
        let manager = MeetManager::new();
        let result = manager.delete_meet("nonexistent-meet");
        assert!(!result);
    }

    #[tokio::test]
    async fn test_meet_manager_get_all_meet_ids() {
        let manager = MeetManager::new();
        let storage = MockStorage;

        assert!(manager.get_all_meet_ids().is_empty());

        let meet_id1 = "test-meet-1".to_string();
        let meet_id2 = "test-meet-2".to_string();
        let meet_id3 = "test-meet-3".to_string();

        manager.create_meet(meet_id1.clone(), storage.clone()).await;
        manager.create_meet(meet_id2.clone(), storage.clone()).await;
        manager.create_meet(meet_id3.clone(), storage.clone()).await;

        let all_meets = manager.get_all_meet_ids();
        assert_eq!(all_meets.len(), 3);
        assert!(all_meets.contains(&meet_id1));
        assert!(all_meets.contains(&meet_id2));
        assert!(all_meets.contains(&meet_id3));
    }
}
