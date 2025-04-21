#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        routing::get,
        Router,
        body::Body,
        http::{Request, StatusCode},
    };
    use std::time::Duration;
    use tower::ServiceExt;

    async fn test_handler() -> &'static str {
        "Hello, World!"
    }

    #[tokio::test]
    async fn test_rate_limit() {
        // Create test settings
        let settings = Settings::default();
        let settings = SettingsManager::new(settings).unwrap();

        // Create test state
        let mut state = AppState::new(
            FlatFileStorage::new("test_data").unwrap(),
            settings.get().await,
        )
        .unwrap();

        // Create test router
        let app = Router::new()
            .route("/", get(test_handler))
            .layer(axum::middleware::from_fn(rate_limit))
            .with_state(Arc::new(state));

        // Test successful request
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header("x-real-ip", "127.0.0.1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Test rate limit exceeded
        for _ in 0..settings.get().await.rate_limit.max_requests {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/")
                        .header("x-real-ip", "127.0.0.1")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
        }

        // Next request should be rate limited
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header("x-real-ip", "127.0.0.1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);

        // Wait for rate limit window to expire
        tokio::time::sleep(Duration::from_secs(
            settings.get().await.rate_limit.window_secs + 1,
        ))
        .await;

        // Request should succeed again
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header("x-real-ip", "127.0.0.1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
} 