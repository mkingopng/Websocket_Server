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
use tower::ServiceExt;

async fn test_handler() -> &'static str {
    "Hello, World!"
}

#[tokio::test]
async fn test_basic_router() {
    // Create test settings
    let settings = Settings::default();

    // Create test state
    let state = AppState::new(FlatFileStorage::new("test_data").unwrap(), &settings)
        .await
        .unwrap();

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
