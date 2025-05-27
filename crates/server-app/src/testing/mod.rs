pub mod fixtures;

// Re-export commonly used items for convenience
pub use fixtures::*;

/// Integration test utilities
pub mod integration {
    use super::*;
    use crate::{websocket::WebSocketHandler, AppState};
    use std::sync::Arc;

    /// Complete integration test environment with WebSocket handler
    pub struct IntegrationTestEnv {
        pub state: Arc<AppState<crate::storage::FlatFileStorage>>,
        pub handler: WebSocketHandler<crate::storage::FlatFileStorage>,
        pub temp_dir: tempfile::TempDir,
    }

    /// Setup complete integration test environment
    pub async fn setup_integration_test() -> IntegrationTestEnv {
        let env = setup_websocket_test().await;
        IntegrationTestEnv {
            state: env.state,
            handler: env.handler,
            temp_dir: env.temp_dir,
        }
    }

    /// Create and join meet workflow for integration tests
    pub async fn create_and_join_meet(_env: &mut IntegrationTestEnv) -> (String, String, String) {
        // This would contain the full workflow logic
        // For now, return mock data
        (
            TEST_MEET_ID.to_string(),
            "creator_token".to_string(),
            "joiner_token".to_string(),
        )
    }
}

/// Performance testing utilities
pub mod performance {
    use crate::messages::Update;
    use std::future::Future;
    use std::time::{Duration, Instant};

    /// Generate test data for performance testing
    pub fn generate_performance_test_data(count: usize) -> Vec<Update> {
        (0..count)
            .map(|i| Update {
                location: format!("test.location.{}", i),
                value: format!("test_value_{}", i),
                timestamp: chrono::Utc::now().timestamp(),
            })
            .collect()
    }

    /// Measure execution time of async operations
    pub async fn measure_test_time<F, Fut, T>(f: F) -> (T, Duration)
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = T>,
    {
        let start = Instant::now();
        let result = f().await;
        let duration = start.elapsed();
        (result, duration)
    }
}

/// Cleanup utilities for tests
pub mod cleanup {
    use tempfile::TempDir;

    /// Clean up test files and directories
    pub fn cleanup_test_files(_temp_dir: &TempDir) {
        // TempDir automatically cleans up on drop
        // This function exists for explicit cleanup if needed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_integration_utilities() {
        let mut env = integration::setup_integration_test().await;

        // Test create and join workflow
        let (meet_id, creator_token, joiner_token) =
            integration::create_and_join_meet(&mut env).await;

        assert!(!meet_id.is_empty());
        assert!(!creator_token.is_empty());
        assert!(!joiner_token.is_empty());
        assert_ne!(creator_token, joiner_token);
    }

    #[tokio::test]
    async fn test_performance_utilities() {
        let updates = performance::generate_performance_test_data(100);
        assert_eq!(updates.len(), 100);

        // Test timing measurement
        let (result, duration) = performance::measure_test_time(|| async {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            42
        })
        .await;

        assert_eq!(result, 42);
        assert!(duration >= std::time::Duration::from_millis(10));
    }

    #[tokio::test]
    async fn test_cleanup_utilities() {
        let env = setup_test_environment().await;

        // The temp directory should exist
        assert!(env.temp_dir.path().exists());

        // Test cleanup (TempDir will auto-cleanup on drop)
        cleanup::cleanup_test_files(&env.temp_dir);
    }
}
