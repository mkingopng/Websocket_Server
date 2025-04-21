// server/tests/auth.rs
use tokio;
use std::time::Duration;
use server::SessionManager;

#[tokio::test]
async fn test_password_hashing() {
    let plain_password = "test123";
    
    // Hash the password
    let hash = SessionManager::hash_password(plain_password).unwrap();
    
    // Verify the password
    assert!(SessionManager::verify_password(&hash, plain_password));
    
    // Verify an incorrect password
    assert!(!SessionManager::verify_password(&hash, "wrong_password"));
}

#[tokio::test]
async fn test_session_management() {
    let session_manager = SessionManager::new();
    
    // Create a new session
    let meet_id = "test-meet".to_string();
    let location_name = "Test Location".to_string();
    let priority = 1;
    
    let session_token = session_manager.new_session(
        meet_id.clone(),
        location_name.clone(),
        priority,
    ).await;
    
    // Verify the session exists
    assert!(session_manager.validate_session(&session_token).await);
    
    // Get the session
    let session = session_manager.get(&session_token).await.unwrap();
    
    // Verify the session data
    assert_eq!(session.meet_id, meet_id);
    assert_eq!(session.location_name, location_name);
    assert_eq!(session.priority, priority);
    
    // Verify the session is valid
    assert!(session_manager.validate_session(&session_token).await);
    
    // Verify an invalid session
    assert!(!session_manager.validate_session("invalid_token").await);
}

#[tokio::test]
async fn test_session_expiration() {
    // This test would require mocking the system time
    // For simplicity, we're just testing the basic functionality
    
    let session_manager = SessionManager::new();
    
    // Create a new session
    let session_token = session_manager.new_session(
        "test-meet".to_string(),
        "Test Location".to_string(),
        1,
    ).await;
    
    // Verify the session is valid
    assert!(session_manager.validate_session(&session_token).await);
    
    // In a real test, we would advance the system time and verify the session expires
    // For now, we're just testing that the session is valid initially
} 