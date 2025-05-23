// ==========
// crates/server-app/src/middleware/middleware_tests.rs
// ==========
//! Tests for middleware functionality.
#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        routing::get,
        Router,
    };
    use std::sync::Arc;
    use tower::ServiceExt;

    use crate::config::Settings;
    use crate::storage::FlatFileStorage;
    use crate::AppState;

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
}
