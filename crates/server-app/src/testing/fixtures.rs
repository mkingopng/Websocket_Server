// =========================================
// crates/server-app/src/testing/fixtures.rs
// =========================================
//! Comprehensive test utilities and fixtures
//! This module consolidates all test setup patterns and shared data to eliminate
//! duplication across the codebase and provide consistent test environments.

use crate::{
    auth::AuthService,
    config::Settings,
    meet_actor::{spawn_meet_actor, MeetHandle},
    messages::{Lifter, ServerMessage, Update, UpdateWithMetadata},
    storage::FlatFileStorage,
    websocket::WebSocketHandler,
    AppState,
};
use axum::extract::ws::Message as AxumMessage;
use openlifter_common::{ClientToServer, EndpointPriority, Update as CommonUpdate};
use std::{sync::Arc, time::Duration};
use tempfile::TempDir;
use tokio::sync::mpsc;

/// Standard test password that meets all validation requirements
pub const TEST_PASSWORD: &str = "TestPassword123!";
pub const WEAK_PASSWORD: &str = "weak";
pub const STRONG_PASSWORD: &str = "VeryStrongPassword123!@#";

/// Standard test location names
pub const TEST_LOCATION: &str = "Test Location";
pub const TEST_LOCATION_2: &str = "Test Location 2";
pub const HIGH_PRIORITY_LOCATION: &str = "High Priority Location";
pub const LOW_PRIORITY_LOCATION: &str = "Low Priority Location";

/// Standard test meet IDs
pub const TEST_MEET_ID: &str = "test-meet-id";
pub const TEST_MEET_ID_2: &str = "test-meet-id-2";

/// Standard test session tokens
pub const TEST_SESSION_TOKEN: &str = "test-session-token";
pub const INVALID_SESSION_TOKEN: &str = "invalid-session-token";

/// Standard test emails
pub const TEST_EMAIL: &str = "test@example.com";
pub const RETURN_EMAIL: &str = "results@example.com";

/// Standard test priorities
pub const HIGH_PRIORITY: u8 = 8;
pub const MEDIUM_PRIORITY: u8 = 5;
pub const LOW_PRIORITY: u8 = 1;

/// Standard CSV data for testing
pub const TEST_CSV_DATA: &str = "Name,Weight,Squat\nJohn Doe,93,150\nJane Smith,84,130";
pub const MINIMAL_CSV_DATA: &str = "Name,Weight\nTest,75";

/// Standard test update data
pub const TEST_UPDATE_KEY: &str = "lifter.1.name";
pub const TEST_UPDATE_VALUE: &str = "Test Lifter";

/// Comprehensive test environment
pub struct TestEnvironment {
    pub state: Arc<AppState<FlatFileStorage>>,
    pub temp_dir: TempDir,
}

/// WebSocket test environment with handler
pub struct WebSocketTestEnv {
    pub handler: WebSocketHandler<FlatFileStorage>,
    pub state: Arc<AppState<FlatFileStorage>>,
    pub temp_dir: TempDir,
}

/// Meet actor test environment
pub struct MeetActorTestEnv {
    pub handle: MeetHandle,
    pub state: Arc<AppState<FlatFileStorage>>,
    pub storage: FlatFileStorage,
    pub temp_dir: TempDir,
}

/// Session test environment
#[derive(Debug)]
pub struct SessionTestEnv<T> {
    pub manager: T,
    pub temp_dir: TempDir,
}

/// Set up a basic test environment with storage and state
pub async fn setup_test_environment() -> TestEnvironment {
    let temp_dir = TempDir::new().unwrap();
    let storage = FlatFileStorage::new(temp_dir.path()).unwrap();

    // Create settings with the temp directory path
    let mut settings = Settings::default();
    settings.storage.path = temp_dir.path().to_path_buf();

    let state = Arc::new(AppState::new(storage, &settings).await.unwrap());
    let (_tx, _rx) = mpsc::channel::<AxumMessage>(32);

    TestEnvironment { state, temp_dir }
}

/// Set up a WebSocket test environment
pub async fn setup_websocket_test() -> WebSocketTestEnv {
    let temp_dir = TempDir::new().unwrap();
    let storage = FlatFileStorage::new(temp_dir.path()).unwrap();

    // Create settings with the temp directory path
    let mut settings = Settings::default();
    settings.storage.path = temp_dir.path().to_path_buf();

    let state = Arc::new(AppState::new(storage, &settings).await.unwrap());
    let handler = WebSocketHandler::new(state.clone());
    let (_tx, _rx) = mpsc::channel::<ServerMessage>(32);

    WebSocketTestEnv {
        handler,
        state,
        temp_dir,
    }
}

/// Set up a meet actor test environment  
pub async fn setup_meet_actor_test() -> MeetActorTestEnv {
    let temp_dir = TempDir::new().unwrap();
    let storage = FlatFileStorage::new(temp_dir.path()).unwrap();
    let settings = Settings::default();
    let state = Arc::new(AppState::new(storage.clone(), &settings).await.unwrap());
    let handle = spawn_meet_actor(TEST_MEET_ID, storage.clone()).await;

    MeetActorTestEnv {
        handle,
        state,
        storage,
        temp_dir,
    }
}

/// Creates a session manager test environment
pub async fn setup_session_test<T>() -> SessionTestEnv<T>
where
    T: Default,
{
    let temp_dir = TempDir::new().unwrap();
    let manager = T::default();

    SessionTestEnv { manager, temp_dir }
}

/// Standard test endpoint priority configurations
pub fn test_endpoint_priority() -> EndpointPriority {
    EndpointPriority {
        location_name: TEST_LOCATION.to_string(),
        priority: MEDIUM_PRIORITY,
    }
}

pub fn high_priority_endpoint() -> EndpointPriority {
    EndpointPriority {
        location_name: HIGH_PRIORITY_LOCATION.to_string(),
        priority: HIGH_PRIORITY,
    }
}

pub fn low_priority_endpoint() -> EndpointPriority {
    EndpointPriority {
        location_name: LOW_PRIORITY_LOCATION.to_string(),
        priority: LOW_PRIORITY,
    }
}

pub fn multiple_endpoints() -> Vec<EndpointPriority> {
    vec![
        high_priority_endpoint(),
        test_endpoint_priority(),
        low_priority_endpoint(),
    ]
}

/// Standard message factories for testing
pub fn test_create_meet_message() -> ClientToServer {
    ClientToServer::CreateMeet {
        this_location_name: TEST_LOCATION.to_string(),
        password: TEST_PASSWORD.to_string(),
        endpoints: vec![test_endpoint_priority()],
    }
}

pub fn test_create_meet_with_multiple_endpoints() -> ClientToServer {
    ClientToServer::CreateMeet {
        this_location_name: TEST_LOCATION.to_string(),
        password: TEST_PASSWORD.to_string(),
        endpoints: vec![
            EndpointPriority {
                location_name: TEST_LOCATION.to_string(),
                priority: MEDIUM_PRIORITY,
            },
            EndpointPriority {
                location_name: TEST_LOCATION_2.to_string(),
                priority: HIGH_PRIORITY,
            },
        ],
    }
}

pub fn test_create_meet_weak_password() -> ClientToServer {
    ClientToServer::CreateMeet {
        this_location_name: TEST_LOCATION.to_string(),
        password: WEAK_PASSWORD.to_string(),
        endpoints: vec![],
    }
}

pub fn test_join_meet_message() -> ClientToServer {
    ClientToServer::JoinMeet {
        meet_id: TEST_MEET_ID.to_string(),
        password: TEST_PASSWORD.to_string(),
        location_name: TEST_LOCATION.to_string(),
    }
}

pub fn test_join_meet_custom(meet_id: &str, location: &str) -> ClientToServer {
    ClientToServer::JoinMeet {
        meet_id: meet_id.to_string(),
        password: TEST_PASSWORD.to_string(),
        location_name: location.to_string(),
    }
}

pub fn test_update_init_message(session_token: &str) -> ClientToServer {
    ClientToServer::UpdateInit {
        session_token: session_token.to_string(),
        updates: vec![test_update()],
    }
}

pub fn test_update_init_empty(session_token: &str) -> ClientToServer {
    ClientToServer::UpdateInit {
        session_token: session_token.to_string(),
        updates: vec![],
    }
}

pub fn test_client_pull_message(session_token: &str, last_seq: u64) -> ClientToServer {
    ClientToServer::ClientPull {
        session_token: session_token.to_string(),
        last_server_seq: last_seq,
    }
}

pub fn test_publish_meet_message(session_token: &str) -> ClientToServer {
    ClientToServer::PublishMeet {
        session_token: session_token.to_string(),
        return_email: TEST_EMAIL.to_string(),
        opl_csv: TEST_CSV_DATA.to_string(),
    }
}

pub fn test_publish_meet_custom(
    session_token: &str,
    csv_data: &str,
    email: &str,
) -> ClientToServer {
    ClientToServer::PublishMeet {
        session_token: session_token.to_string(),
        return_email: email.to_string(),
        opl_csv: csv_data.to_string(),
    }
}

/// Standard update data factories
pub fn test_update() -> CommonUpdate {
    CommonUpdate {
        update_key: TEST_UPDATE_KEY.to_string(),
        update_value: serde_json::json!(TEST_UPDATE_VALUE),
        local_seq_num: 1,
        after_server_seq_num: 0,
    }
}

pub fn test_update_custom(
    key: &str,
    value: serde_json::Value,
    local_seq: u64,
    after_seq: u64,
) -> CommonUpdate {
    CommonUpdate {
        update_key: key.to_string(),
        update_value: value,
        local_seq_num: local_seq,
        after_server_seq_num: after_seq,
    }
}

pub fn test_lifter_update(name: &str, weight: f32) -> CommonUpdate {
    CommonUpdate {
        update_key: format!("lifter.{}.name", name.replace(' ', "_")),
        update_value: serde_json::json!({
            "name": name,
            "bodyweight": weight
        }),
        local_seq_num: 1,
        after_server_seq_num: 0,
    }
}

pub fn test_batch_updates() -> Vec<CommonUpdate> {
    vec![
        test_lifter_update("John Doe", 93.0),
        test_lifter_update("Jane Smith", 84.0),
        test_update_custom("meet.status", serde_json::json!("active"), 3, 2),
    ]
}

/// Standard internal update factories
pub fn test_internal_update() -> Update {
    Update {
        location: TEST_UPDATE_KEY.to_string(),
        value: TEST_UPDATE_VALUE.to_string(),
        timestamp: chrono::Utc::now().timestamp(),
    }
}

pub fn test_update_with_metadata() -> UpdateWithMetadata {
    UpdateWithMetadata {
        update: test_internal_update(),
        source_client: "test-client".to_string(),
        server_seq: 1,
        priority: MEDIUM_PRIORITY,
    }
}

/// Standard lifter data
pub fn test_lifter() -> Lifter {
    Lifter {
        name: "Test Lifter".to_string(),
        weight_class: "93".to_string(),
        gender: "M".to_string(),
        age: 25,
    }
}

pub fn test_lifters() -> Vec<Lifter> {
    vec![
        Lifter {
            name: "John Doe".to_string(),
            weight_class: "93".to_string(),
            gender: "M".to_string(),
            age: 28,
        },
        Lifter {
            name: "Jane Smith".to_string(),
            weight_class: "84".to_string(),
            gender: "F".to_string(),
            age: 26,
        },
    ]
}

/// Test workflow helpers - these replace common test patterns
pub async fn create_test_session(auth: &dyn AuthService) -> String {
    auth.new_session(
        TEST_MEET_ID.to_string(),
        TEST_LOCATION.to_string(),
        MEDIUM_PRIORITY,
    )
    .await
}

pub async fn create_test_meet_and_get_session(env: &mut WebSocketTestEnv) -> (String, String) {
    let create_msg = test_create_meet_message();
    let response = env.handler.handle_message(create_msg).await.unwrap();

    match response {
        ServerMessage::MeetCreated {
            meet_id,
            session_token,
        } => (meet_id, session_token),
        _ => panic!("Expected MeetCreated response"),
    }
}

pub async fn join_test_meet_and_get_session(env: &mut WebSocketTestEnv, meet_id: &str) -> String {
    let join_msg = test_join_meet_custom(meet_id, TEST_LOCATION_2);
    let response = env.handler.handle_message(join_msg).await.unwrap();

    match response {
        ServerMessage::MeetJoined { session_token, .. } => session_token,
        _ => panic!("Expected MeetJoined response"),
    }
}

/// Assertion helpers for common test patterns
pub fn assert_meet_created_response(response: &ServerMessage) -> (&str, &str) {
    match response {
        ServerMessage::MeetCreated {
            meet_id,
            session_token,
        } => {
            assert!(!meet_id.is_empty(), "Meet ID should not be empty");
            assert!(
                !session_token.is_empty(),
                "Session token should not be empty"
            );
            (meet_id, session_token)
        },
        other => panic!("Expected MeetCreated response, got {:?}", other),
    }
}

pub fn assert_meet_joined_response(response: &ServerMessage) -> &str {
    match response {
        ServerMessage::MeetJoined { session_token, .. } => {
            assert!(
                !session_token.is_empty(),
                "Session token should not be empty"
            );
            session_token
        },
        other => panic!("Expected MeetJoined response, got {:?}", other),
    }
}

pub fn assert_update_ack_response(response: &ServerMessage) -> (&str, &[String]) {
    match response {
        ServerMessage::UpdateAck {
            meet_id,
            update_ids,
        } => {
            assert!(!meet_id.is_empty(), "Meet ID should not be empty");
            (meet_id, update_ids)
        },
        other => panic!("Expected UpdateAck response, got {:?}", other),
    }
}

pub fn assert_error_response(response: &ServerMessage, expected_code: &str) {
    match response {
        ServerMessage::Error { code, message } => {
            assert_eq!(code, expected_code, "Error code mismatch");
            assert!(!message.is_empty(), "Error message should not be empty");
        },
        other => panic!(
            "Expected Error response with code {}, got {:?}",
            expected_code, other
        ),
    }
}

pub fn assert_invalid_session_response(response: &ServerMessage, expected_token: &str) {
    match response {
        ServerMessage::InvalidSession { session_token } => {
            assert_eq!(session_token, expected_token, "Session token mismatch");
        },
        other => panic!("Expected InvalidSession response, got {:?}", other),
    }
}

/// Timeout wrapper for async tests
pub async fn with_timeout<F, T>(future: F) -> T
where
    F: std::future::Future<Output = T>,
{
    tokio::time::timeout(std::time::Duration::from_secs(5), future)
        .await
        .expect("Test timed out")
}

/// Test utilities for file operations
pub fn create_test_meet_directory(temp_dir: &TempDir, meet_id: &str) -> std::path::PathBuf {
    let meet_dir = temp_dir.path().join("current-meets").join(meet_id);
    std::fs::create_dir_all(&meet_dir).unwrap();
    meet_dir
}

pub async fn wait_briefly_ms(milliseconds: u64) {
    tokio::time::sleep(Duration::from_millis(milliseconds)).await;
}

/// Random data generators for unique test scenarios
pub fn unique_meet_id(prefix: &str) -> String {
    format!("{}-{}", prefix, uuid::Uuid::new_v4())
}

pub fn unique_location_name(prefix: &str) -> String {
    format!("{} {}", prefix, uuid::Uuid::new_v4())
}

pub fn unique_session_token() -> String {
    format!("session-{}", uuid::Uuid::new_v4())
}

/// Performance test helpers
pub fn generate_large_update_batch(count: usize) -> Vec<CommonUpdate> {
    (0..count)
        .map(|i| {
            test_update_custom(
                &format!("lifter.{}.weight", i),
                serde_json::json!(80.0 + i as f64),
                i as u64 + 1,
                i as u64,
            )
        })
        .collect()
}

pub fn generate_test_csv(rows: usize) -> String {
    let mut csv = "Name,Weight,Squat\n".to_string();
    for i in 0..rows {
        csv.push_str(&format!("Lifter{},{},{}\n", i, 80 + i, 100 + i * 5));
    }
    csv
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_setup_test_environment() {
        let env = setup_test_environment().await;
        assert!(env.temp_dir.path().exists());
        assert!(env.temp_dir.path().join("sessions").exists());
        assert!(env.temp_dir.path().join("current-meets").exists());
    }

    #[tokio::test]
    async fn test_websocket_test_environment() {
        let env = setup_websocket_test().await;
        assert!(env.temp_dir.path().exists());
        assert!(env.temp_dir.path().join("sessions").exists());
    }

    #[test]
    fn test_message_factories() {
        let create_msg = test_create_meet_message();
        assert!(matches!(create_msg, ClientToServer::CreateMeet { .. }));

        let join_msg = test_join_meet_message();
        assert!(matches!(join_msg, ClientToServer::JoinMeet { .. }));

        let update_msg = test_update_init_message("test-token");
        assert!(matches!(update_msg, ClientToServer::UpdateInit { .. }));
    }

    #[test]
    fn test_update_factories() {
        let update = test_update();
        assert_eq!(update.update_key, TEST_UPDATE_KEY);
        assert_eq!(update.local_seq_num, 1);

        let batch = test_batch_updates();
        assert_eq!(batch.len(), 3);
    }

    #[test]
    fn test_unique_generators() {
        let id1 = unique_meet_id("test");
        let id2 = unique_meet_id("test");
        assert_ne!(id1, id2);
        assert!(id1.starts_with("test-"));
        assert!(id2.starts_with("test-"));
    }
}
