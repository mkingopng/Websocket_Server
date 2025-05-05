//! Test utilities for WebSocket Server tests
//!
//! This module provides common test setup logic for initializing test environments
//! with proper session directories and configuration.

use backend_lib::{
    config::Settings,
    storage::FlatFileStorage,
    AppState,
};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::mpsc;
use axum::extract::ws::Message;

/// Sets up a test environment with a temporary directory and properly configured session directory
///
/// This helper function creates:
/// 1. A temporary directory for test data
/// 2. The necessary session directory that many components require to work properly
/// 3. A properly configured Settings object pointing to the temp directory
/// 4. An AppState instance properly initialized
/// 5. WebSocket message channels for testing message-based components
///
/// # Returns
///
/// A tuple with:
/// - AppState with FlatFileStorage
/// - Message sender for WebSocket tests
/// - Message receiver for WebSocket tests
/// - The temporary directory (keep this in scope to prevent cleanup during the test)
///
/// # Example
///
/// ```
/// #[tokio::test]
/// async fn test_something() {
///     let (state, tx, mut rx, _temp_dir) = setup_test_env().await;
///     
///     // Test logic here...
/// }
/// ```
pub async fn setup_test_env() -> (
    Arc<AppState<FlatFileStorage>>,
    mpsc::Sender<Message>,
    mpsc::Receiver<Message>,
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
            .expect("Failed to create AppState for test")
    );

    // Create a channel for sending messages back to the client
    let (tx, rx) = mpsc::channel::<Message>(32);

    (state, tx, rx, temp_dir)
}

/// Create a properly structured meet directory in the test environment
///
/// This helper ensures meet directories exist with the proper structure
/// for tests that need to access meet data.
///
/// # Arguments
///
/// * `temp_dir` - The temporary directory for the test
/// * `meet_id` - The ID of the meet to create
///
/// # Returns
///
/// The path to the meet directory
pub fn create_meet_directory(temp_dir: &TempDir, meet_id: &str) -> std::path::PathBuf {
    let meet_dir = temp_dir.path().join("current-meets").join(meet_id);
    std::fs::create_dir_all(&meet_dir).expect("Failed to create meet directory");
    meet_dir
}

/// Wait for a short period to ensure async operations complete
///
/// Use this to avoid race conditions in tests when checking for file existence
/// or other side effects of async operations.
///
/// # Arguments
///
/// * `milliseconds` - Number of milliseconds to wait
pub async fn wait_briefly(milliseconds: u64) {
    tokio::time::sleep(tokio::time::Duration::from_millis(milliseconds)).await;
} 