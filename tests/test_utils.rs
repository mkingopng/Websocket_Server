// ===================
// tests/test_utils.rs
// ===================
//! Test utilities for WebSocket Server tests
//! This module provides server-protocols test setup logic for initializing test environments
//! with proper session directories and configuration.

use axum::extract::ws::Message as AxumMessage;
use backend_lib::auth::AuthService;
use backend_lib::meet_actor::{spawn_meet_actor, MeetHandle};
use backend_lib::{
    auth::{DefaultAuth, PersistentSessionManager},
    config::Settings,
    messages::{Decision, Lifter},
    storage::FlatFileStorage,
    AppState,
};
use futures_util::{SinkExt, StreamExt};
use openlifter_common::{Update, UpdateWithServerSeq};
use std::fmt::Debug;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message as TungsteniteMessage;

/// Sets up a test environment with a temporary directory and properly configured session directory
/// This helper function creates:
/// 1. A temporary directory for test data
/// 2. The necessary session directory that many components require to work properly
/// 3. A properly configured Settings object pointing to the temp directory
/// 4. An `AppState` instance properly initialized
/// 5. WebSocket message channels for testing message-based components
/// # Returns
/// A tuple with:
/// - `AppState` with `FlatFileStorage`
/// - Message sender for WebSocket tests
/// - Message receiver for WebSocket tests
/// - The temporary directory (keep this in scope to prevent cleanup during the test)
pub async fn setup_test_env() -> (
    Arc<AppState<FlatFileStorage>>,
    mpsc::Sender<AxumMessage>,
    mpsc::Receiver<AxumMessage>,
    TempDir,
) {
    // Create a temporary directory for test data
    let temp_dir = TempDir::new().unwrap();
    let storage = FlatFileStorage::new(temp_dir.path()).unwrap();

    // Create settings with proper temp directory
    let mut settings = Settings::default();
    settings.storage.path = temp_dir.path().to_path_buf();

    // Ensure the sessions directory exists - critical for tests to pass
    let sessions_dir = temp_dir.path().join("sessions");
    std::fs::create_dir_all(&sessions_dir).expect("Failed to create sessions directory");

    // Create app state
    let state = Arc::new(
        AppState::new(storage.clone(), &settings)
            .await
            .expect("Failed to create AppState for test"),
    );

    // Create a channel for sending messages back to the client
    let (tx, rx) = mpsc::channel::<AxumMessage>(32);

    (state, tx, rx, temp_dir)
}

/// Create a properly structured meet directory in the test environment
/// This helper ensures meet directories exist with the proper structure
/// for tests that need to access meet data.
/// # Arguments
/// * `temp_dir` - The temporary directory for the test
/// * `meet_id` - The ID of the meet to create
/// # Returns
/// The path to the meet directory
pub fn create_meet_directory(temp_dir: &TempDir, meet_id: &str) -> std::path::PathBuf {
    let meet_dir = temp_dir.path().join("current-meets").join(meet_id);
    std::fs::create_dir_all(&meet_dir).expect("Failed to create meet directory");
    meet_dir
}

/// Wait for a short period to ensure async operations complete
/// Use this to avoid race conditions in tests when checking for file existence
/// or other side effects of async operations.
/// # Arguments
/// * `milliseconds` - Number of milliseconds to wait
pub async fn wait_briefly(milliseconds: u64) {
    tokio::time::sleep(tokio::time::Duration::from_millis(milliseconds)).await;
}

/// Generate a unique meet ID for testing
pub fn unique_meet_id(prefix: &str) -> String {
    format!("{}-{}", prefix, uuid::Uuid::new_v4())
}

/// Wait for a message with timeout
pub async fn next_message_with_timeout<S>(
    stream: &mut S,
    timeout_secs: u64,
    operation_name: &str,
) -> TungsteniteMessage
where
    S: StreamExt<Item = Result<TungsteniteMessage, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    match tokio::time::timeout(
        tokio::time::Duration::from_secs(timeout_secs),
        stream.next(),
    )
    .await
    {
        Ok(Some(Ok(msg))) => msg,
        Ok(Some(Err(e))) => panic!("Error during {operation_name}: {e}"),
        Ok(None) => panic!("Stream closed during {operation_name}"),
        Err(e) => panic!("Timeout during {operation_name}: {e}"),
    }
}

/// Safely close a WebSocket connection
pub async fn safe_close_connection<S>(stream: &mut S)
where
    S: SinkExt<TungsteniteMessage> + Unpin,
    <S as futures_util::Sink<TungsteniteMessage>>::Error: Debug,
{
    if let Err(e) = stream.close().await {
        eprintln!("Error closing connection: {e:?}");
    }
}

/// Set up a test server
pub async fn setup_server() -> (
    String,                         // Server address
    Arc<AppState<FlatFileStorage>>, // App state
    TempDir,                        // Temp directory
) {
    let (state, _tx, _rx, temp_dir) = setup_test_env().await;
    let addr = "127.0.0.1:0".to_string();
    (addr, state, temp_dir)
}

pub struct TestMeet {
    pub meet_id: String,
    pub auth_service: DefaultAuth,
    pub storage: FlatFileStorage,
    pub meet_handle: MeetHandle,
    _temp_dir: TempDir,
}

impl TestMeet {
    pub async fn new() -> Self {
        let temp_dir = TempDir::new().unwrap();
        let storage = FlatFileStorage::new(temp_dir.path()).unwrap();
        let session_path = temp_dir.path().join("sessions");
        let session_manager = PersistentSessionManager::new(&session_path).await.unwrap();
        let auth_service = DefaultAuth::new(session_manager);
        let meet_id = uuid::Uuid::new_v4().to_string();
        let meet_handle = spawn_meet_actor(&meet_id, storage.clone()).await;
        Self {
            meet_id,
            auth_service,
            storage,
            meet_handle,
            _temp_dir: temp_dir,
        }
    }

    pub async fn create_meet(&self, _password: &str) -> Result<String, String> {
        Ok(self
            .auth_service
            .new_session(self.meet_id.clone(), "Test Location".to_string(), 1)
            .await)
    }

    pub async fn register_lifters(&self, lifters: &[Lifter]) -> Result<(), String> {
        // Store lifters as updates
        for lifter in lifters {
            let update = Update {
                update_key: format!("lifters.{}", lifter.name),
                update_value: serde_json::to_value(lifter).unwrap(),
                local_seq_num: 1,
                after_server_seq_num: 0,
            };
            self.meet_handle
                .apply_updates("test".to_string(), 1, vec![update])
                .await
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub async fn apply_attempt_update(
        &self,
        client_id: &str,
        priority: u8,
        update: Update,
    ) -> Result<(), String> {
        self.meet_handle
            .apply_updates(client_id.to_string(), priority, vec![update])
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub async fn get_updates_since(&self, since: u64) -> Result<Vec<UpdateWithServerSeq>, String> {
        self.meet_handle
            .get_updates_since(since)
            .await
            .map_err(|e| e.to_string())
    }
}

pub fn attempt_to_update(
    lifter: &Lifter,
    lift: &str,
    attempt_number: u8,
    weight: f32,
    decision: &Decision,
) -> Update {
    let update_key = format!("{}.{}.attempt{}", lifter.name, lift, attempt_number);
    let update_value = serde_json::json!({
        "weight": weight,
        "decision": format!("{:?}", decision),
    });
    Update {
        update_key,
        update_value,
        local_seq_num: u64::from(attempt_number),
        after_server_seq_num: 0,
    }
}

pub fn create_test_lifters() -> Vec<Lifter> {
    vec![
        Lifter {
            name: "John Smith".to_string(),
            weight_class: "93kg".to_string(),
            gender: "M".to_string(),
            age: 25,
        },
        Lifter {
            name: "Jane Doe".to_string(),
            weight_class: "84kg".to_string(),
            gender: "F".to_string(),
            age: 28,
        },
        // Add more test lifters as needed
    ]
}

pub async fn run_meet_simulation() -> Result<(), String> {
    let test_meet = TestMeet::new().await;
    let lifters = create_test_lifters();

    // Create meet
    let _session_token = test_meet.create_meet("TestPassword123!").await?;

    test_meet.register_lifters(&lifters).await?;

    // Start meet (commented out, method does not exist)
    // test_meet.meet_handle.start_meet(&test_meet.meet_id).await?;

    // Simulate meet flow
    for lift in &["squat", "bench", "deadlift"] {
        for attempt in 1..=3 {
            for lifter in &lifters {
                // Record attempt
                let decision = if rand::random() {
                    Decision::GoodLift
                } else {
                    Decision::NoLift
                };

                test_meet
                    .apply_attempt_update(
                        &lifter.name,
                        0,
                        attempt_to_update(
                            lifter,
                            lift,
                            attempt,
                            100.0 + (f32::from(attempt) * 5.0),
                            &decision,
                        ),
                    )
                    .await?;

                // Submit next attempt if not the last attempt
                if attempt < 3 {
                    test_meet
                        .apply_attempt_update(
                            &lifter.name,
                            0,
                            attempt_to_update(
                                lifter,
                                lift,
                                attempt + 1,
                                100.0 + (f32::from(attempt + 1) * 5.0),
                                &Decision::NoLift,
                            ),
                        )
                        .await?;
                }
            }
        }
    }

    // Verify final state
    let final_state = test_meet.get_updates_since(0).await?;
    assert_eq!(final_state.len(), 9); // 3 lifts * 3 attempts

    Ok(())
}
