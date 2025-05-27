// ====================================
// tests/integration/auth_flow_tests.rs
// ====================================
//! Integration tests for the authentication flow in the backend library.
use backend_lib::auth::{AuthService, DefaultAuth};
use tempfile::TempDir;

#[tokio::test]
async fn test_auth_service_flow() {
    // Create the auth service using the helper function
    let auth_service = setup_auth_service().await;

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

async fn setup_auth_service() -> DefaultAuth {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let session_path = temp_dir.path().join("sessions");

    // Create the file persistence adapter and session manager
    let file_persistence =
        backend_lib::auth::session::persistent::FilePersistence::new(&session_path)
            .await
            .unwrap();
    let session_manager =
        backend_lib::auth::session::memory::UnifiedSessionManager::new(file_persistence)
            .await
            .unwrap();

    DefaultAuth::new(session_manager)
}
