use backend_lib::auth::{AuthService, DefaultAuth, PersistentSessionManager};
use tempfile::tempdir;

#[tokio::test]
async fn test_auth_service_flow() {
    // Create a temporary directory for session storage
    let temp_dir = tempdir().unwrap();
    let session_path = temp_dir.path().join("sessions");

    // Create a session manager
    let session_manager = PersistentSessionManager::new(&session_path).await.unwrap();

    // Create the auth service
    let auth_service = DefaultAuth::new(session_manager);

    // Test session creation
    let meet_id = "test-meet-123".to_string();
    let location = "Table 1".to_string();
    let priority = 5;

    let session_token = auth_service
        .new_session(meet_id.clone(), location.clone(), priority)
        .await;

    // Test session validation
    let is_valid = auth_service.validate_session(&session_token).await;
    assert!(is_valid, "Session should be valid");

    // Test retrieving session
    let session = auth_service.get_session(&session_token).await;
    assert!(session.is_some(), "Session should exist");

    if let Some(session) = session {
        assert_eq!(session.meet_id, meet_id);
        assert_eq!(session.location_name, location);
        assert_eq!(session.priority, priority);
    }

    // Test invalid session
    let is_valid = auth_service.validate_session("invalid-token").await;
    assert!(!is_valid, "Invalid session should not be valid");
}
