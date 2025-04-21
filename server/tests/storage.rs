// ==========================
// server/tests/storage.rs
// ==========================
use std::fs;
use tempfile::TempDir;
use tokio;
use common::EndpointPriority;
use server::FlatFileStorage;

#[tokio::test]
async fn test_storage_meet_info() {
    // Create a temporary directory for testing
    let temp_dir = TempDir::new().unwrap();
    let storage = FlatFileStorage::new(temp_dir.path()).unwrap();
    
    // Test storing meet info
    let meet_id = "test-meet";
    let password_hash = "hashed_password";
    let endpoints = vec![
        EndpointPriority {
            location_name: "Location 1".to_string(),
            priority: 1,
        },
        EndpointPriority {
            location_name: "Location 2".to_string(),
            priority: 2,
        },
    ];
    
    // Store the meet info
    storage.store_meet_info(meet_id, password_hash, &endpoints).await.unwrap();
    
    // Retrieve the meet info
    let meet_info = storage.get_meet_info(meet_id).await.unwrap();
    
    // Verify the meet info
    assert_eq!(meet_info.password_hash, password_hash);
    assert_eq!(meet_info.endpoints.len(), 2);
    assert_eq!(meet_info.endpoints[0].location_name, "Location 1");
    assert_eq!(meet_info.endpoints[0].priority, 1);
    assert_eq!(meet_info.endpoints[1].location_name, "Location 2");
    assert_eq!(meet_info.endpoints[1].priority, 2);
}

#[tokio::test]
async fn test_storage_updates() {
    // Create a temporary directory for testing
    let temp_dir = TempDir::new().unwrap();
    let storage = FlatFileStorage::new(temp_dir.path()).unwrap();
    
    // Test storing updates
    let meet_id = "test-meet";
    let update_json = r#"{"update_key":"test-key","update_value":{"value":"test"},"local_seq_num":1,"after_server_seq_num":0}"#;
    
    // Store the update
    storage.append_update(meet_id, update_json).await.unwrap();
    
    // Retrieve the updates
    let updates = storage.read_updates(meet_id).await.unwrap();
    
    // Verify the updates
    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0], update_json);
}

#[tokio::test]
async fn test_storage_archive_meet() {
    // Create a temporary directory for testing
    let temp_dir = TempDir::new().unwrap();
    let storage = FlatFileStorage::new(temp_dir.path()).unwrap();
    
    // Create a meet
    let meet_id = "test-meet";
    let password_hash = "hashed_password";
    let endpoints = vec![
        EndpointPriority {
            location_name: "Location 1".to_string(),
            priority: 1,
        },
    ];
    
    // Store the meet info
    storage.store_meet_info(meet_id, password_hash, &endpoints).await.unwrap();
    
    // Store an update
    let update_json = r#"{"update_key":"test-key","update_value":{"value":"test"},"local_seq_num":1,"after_server_seq_num":0}"#;
    storage.append_update(meet_id, update_json).await.unwrap();
    
    // Archive the meet
    storage.archive_meet(meet_id).await.unwrap();
    
    // Verify the meet is archived
    let current_meet_path = temp_dir.path().join("current-meets").join(meet_id);
    let archived_meet_path = temp_dir.path().join("finished-meets").join(meet_id);
    
    assert!(!current_meet_path.exists());
    assert!(archived_meet_path.exists());
    
    // Verify the meet info is still accessible
    let meet_info = storage.get_meet_info(meet_id).await.unwrap();
    assert_eq!(meet_info.password_hash, password_hash);
    assert_eq!(meet_info.endpoints.len(), 1);
    
    // Verify the updates are still accessible
    let updates = storage.read_updates(meet_id).await.unwrap();
    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0], update_json);
} 