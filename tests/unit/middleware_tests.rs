// ==============================
// tests/unit/middleware_tests.rs
// ==============================
//! Unit tests for the middleware module
use backend_lib::config::Settings;
use backend_lib::storage::FlatFileStorage;
use backend_lib::AppState;

use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::get,
    Router,
};
use std::sync::Arc;
use tempfile::TempDir;
use tower::ServiceExt;

async fn test_handler() -> &'static str {
    "Hello, World!"
}

#[tokio::test]
async fn test_basic_router() {
    // Create a temporary directory for storage
    let temp_dir = TempDir::new().unwrap();
    let storage = FlatFileStorage::new(temp_dir.path()).unwrap();

    // Create settings with a specific sessions path in the temp directory
    let mut settings = Settings::default();
    settings.storage.path = temp_dir.path().to_path_buf();

    // Ensure the sessions directory exists
    let sessions_dir = temp_dir.path().join("sessions");
    std::fs::create_dir_all(&sessions_dir).expect("Failed to create sessions directory");

    // Create test state with proper error handling
    let state = AppState::new(storage, &settings)
        .await
        .expect("Failed to create AppState for test");

    // Create test router without middleware for now
    let app = Router::new()
        .route("/", get(test_handler))
        .with_state(Arc::new(state));

    // Test successful request
    let response = app
        .clone()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
