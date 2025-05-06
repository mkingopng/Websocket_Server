// ========================
// tests/unit/meet_tests.rs
// ========================
//! Unit tests for the `MeetManager` module
use anyhow::Result;
use async_trait::async_trait;
use backend_lib::meet::MeetManager;
use backend_lib::storage::Storage;

// Mock storage for testing
#[derive(Clone)]
struct MockStorage;

#[async_trait]
impl Storage for MockStorage {
    async fn append_update(
        &self,
        _meet_id: &str,
        _value: &str,
    ) -> Result<(), backend_lib::error::AppError> {
        Ok(())
    }

    async fn read_updates(
        &self,
        _meet_id: &str,
    ) -> Result<Vec<String>, backend_lib::error::AppError> {
        Ok(vec![])
    }

    async fn archive_meet(&self, _meet_id: &str) -> Result<(), backend_lib::error::AppError> {
        Ok(())
    }

    async fn store_meet_info(
        &self,
        _meet_id: &str,
        _password_hash: &str,
        _endpoints: &[openlifter_common::EndpointPriority],
    ) -> Result<(), backend_lib::error::AppError> {
        Ok(())
    }

    async fn get_meet_info(
        &self,
        _meet_id: &str,
    ) -> Result<openlifter_common::MeetInfo, backend_lib::error::AppError> {
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
    ) -> Result<(), backend_lib::error::AppError> {
        Ok(())
    }
}

#[tokio::test]
async fn test_meet_manager_create() {
    // Create a MeetManager
    let manager = MeetManager::new();

    // Create a storage instance
    let storage = MockStorage;

    // Create a meet
    let meet_id = "test-meet-id".to_string();
    let _handle = manager.create_meet(meet_id.clone(), storage).await;

    // Verify the meet was created
    assert!(manager.get_meet(&meet_id).is_some());
}

#[tokio::test]
async fn test_meet_manager_get() {
    // Create a MeetManager
    let manager = MeetManager::new();

    // Create a storage instance
    let storage = MockStorage;

    // Create a meet
    let meet_id = "test-meet-id".to_string();
    let _original_handle = manager.create_meet(meet_id.clone(), storage).await;

    // Get the meet handle
    let retrieved_handle = manager.get_meet(&meet_id);

    // Verify we got a handle
    assert!(retrieved_handle.is_some());
}

#[tokio::test]
async fn test_meet_manager_delete() {
    // Create a MeetManager
    let manager = MeetManager::new();

    // Create a storage instance
    let storage = MockStorage;

    // Create a meet
    let meet_id = "test-meet-id".to_string();
    let _handle = manager.create_meet(meet_id.clone(), storage).await;

    // Verify the meet exists
    assert!(manager.get_meet(&meet_id).is_some());

    // Delete the meet
    let result = manager.delete_meet(&meet_id);

    // Verify the meet was deleted
    assert!(result);
    assert!(manager.get_meet(&meet_id).is_none());
}

#[tokio::test]
async fn test_meet_manager_delete_nonexistent() {
    // Create a MeetManager
    let manager = MeetManager::new();

    // Try to delete a meet that doesn't exist
    let result = manager.delete_meet("nonexistent-meet");

    // Verify the result indicates failure
    assert!(!result);
}

#[tokio::test]
async fn test_meet_manager_get_all_meet_ids() {
    // Create a MeetManager
    let manager = MeetManager::new();

    // Create a storage instance
    let storage = MockStorage;

    // No meets initially
    assert!(manager.get_all_meet_ids().is_empty());

    // Create a few meets
    let meet_id1 = "test-meet-1".to_string();
    let meet_id2 = "test-meet-2".to_string();
    let meet_id3 = "test-meet-3".to_string();

    manager.create_meet(meet_id1.clone(), storage.clone()).await;
    manager.create_meet(meet_id2.clone(), storage.clone()).await;
    manager.create_meet(meet_id3.clone(), storage.clone()).await;

    // Get all meet IDs
    let all_meets = manager.get_all_meet_ids();

    // Verify we have the expected number of meets
    assert_eq!(all_meets.len(), 3);

    // Verify all expected meet IDs are present
    assert!(all_meets.contains(&meet_id1));
    assert!(all_meets.contains(&meet_id2));
    assert!(all_meets.contains(&meet_id3));
}
