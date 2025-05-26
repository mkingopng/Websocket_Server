// ===================
// tests/test_utils.rs
// ===================
//! Test utilities for WebSocket Server tests
//! This module provides server-protocols test setup logic for initializing test environments
//! with proper session directories and configuration.

use axum::extract::ws::Message as AxumMessage;
use backend_lib::{config::Settings, storage::FlatFileStorage, AppState};
use futures_util::{SinkExt, StreamExt};
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
