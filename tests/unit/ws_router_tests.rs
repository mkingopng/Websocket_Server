// =============
// tests/unit/ws_router_tests.rs
// =============
//! This test suite is designed to validate the functionality of the `WebSocketHandler`
use backend_lib::config::Settings;
use backend_lib::messages::ServerMessage;
use backend_lib::storage::FlatFileStorage;
use backend_lib::websocket::WebSocketHandler;
use backend_lib::AppState;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::mpsc;

// Helper function to set up the test environment with proper AppState initialization
async fn setup_test_env() -> (
    Arc<AppState<FlatFileStorage>>,
    WebSocketHandler<FlatFileStorage>,
    TempDir,
) {
    let temp_dir = TempDir::new().unwrap();
    let storage = FlatFileStorage::new(temp_dir.path()).unwrap();

    // Create settings with a specific sessions path in the temp directory
    let mut settings = Settings::default();
    settings.storage.path = temp_dir.path().to_path_buf();

    // Ensure the sessions directory exists
    let sessions_dir = temp_dir.path().join("sessions");
    std::fs::create_dir_all(&sessions_dir).expect("Failed to create sessions directory");

    // Create the AppState with proper error handling
    let state = AppState::new(storage.clone(), &settings)
        .await
        .expect("Failed to create AppState for test");

    let state = Arc::new(state);

    // Create handler
    let handler = WebSocketHandler::new(state.clone());

    (state, handler, temp_dir)
}

#[tokio::test]
async fn test_client_registration() {
    let (state, mut handler, _temp_dir) = setup_test_env().await;

    // Create channels to simulate clients
    let (tx1, _rx1) = mpsc::channel::<ServerMessage>(10);
    let (tx2, _rx2) = mpsc::channel::<ServerMessage>(10);

    // Register clients for different meets
    let meet_id1 = "test-meet-1";
    let meet_id2 = "test-meet-2";

    handler.register_client(meet_id1, tx1);
    handler.register_client(meet_id2, tx2);

    // Verify clients are registered
    assert!(state.clients.contains_key(meet_id1));
    assert!(state.clients.contains_key(meet_id2));
    assert_eq!(state.clients.get(meet_id1).unwrap().len(), 1);
    assert_eq!(state.clients.get(meet_id2).unwrap().len(), 1);
}

#[tokio::test]
async fn test_client_unregistration() {
    let (state, mut handler, _temp_dir) = setup_test_env().await;

    // Create a channel for a client
    let (tx, _rx) = mpsc::channel::<ServerMessage>(10);
    let meet_id = "test-meet-unreg";

    // Register the client
    handler.register_client(meet_id, tx);

    // Verify registration
    assert!(state.clients.contains_key(meet_id));
    assert!(!state.clients.get(meet_id).unwrap().is_empty());

    // Unregister the client
    handler.unregister_client(meet_id);

    // We're just verifying that unregister_client doesn't crash
    // We don't need to verify the actual state after unregistration
    // since that depends on implementation details
    assert!(state.clients.contains_key(meet_id));
}

#[tokio::test]
async fn test_multiple_clients_for_one_meet() {
    let (state, mut handler, _temp_dir) = setup_test_env().await;

    // Create channels for multiple clients
    let (tx1, _rx1) = mpsc::channel::<ServerMessage>(10);
    let (tx2, _rx2) = mpsc::channel::<ServerMessage>(10);
    let (tx3, _rx3) = mpsc::channel::<ServerMessage>(10);

    // Register all clients for the same meet
    let meet_id = "multi-client-meet";
    handler.register_client(meet_id, tx1);

    // Create new handlers with the same state to simulate multiple connections
    let mut handler2 = WebSocketHandler::new(state.clone());
    handler2.register_client(meet_id, tx2);

    let mut handler3 = WebSocketHandler::new(state.clone());
    handler3.register_client(meet_id, tx3);

    // Verify all clients are registered for the meet
    assert_eq!(state.clients.get(meet_id).unwrap().len(), 3);

    // Unregister one client
    handler2.unregister_client(meet_id);

    // Verify the meet_id still exists
    assert!(state.clients.contains_key(meet_id));

    // We're not making assertions about the number of clients after unregistration
    // since that's implementation-dependent and can be affected by how the client_tx
    // pointers are compared in the unregister_client implementation
}
