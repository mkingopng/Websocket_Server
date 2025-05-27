// =========================================
// crates/server-app/src/testing/fixtures.rs
// =========================================
//! Common test data fixtures
//! This module provides standardized test data that can be reused across
//! different test modules to ensure consistency and reduce duplication.

use openlifter_common::{ClientToServer, EndpointPriority};

/// Standard test password that meets all validation requirements
pub const TEST_PASSWORD: &str = "TestPassword123!";

/// Standard test location name
pub const TEST_LOCATION: &str = "Test Location";

/// Standard test meet ID
pub const TEST_MEET_ID: &str = "test-meet-id";

/// Standard test session token
pub const TEST_SESSION_TOKEN: &str = "test-session-token";

/// Creates a standard test endpoint priority
pub fn test_endpoint_priority() -> EndpointPriority {
    EndpointPriority {
        location_name: TEST_LOCATION.to_string(),
        priority: 5,
    }
}

/// Creates a standard CreateMeet message for testing
pub fn test_create_meet_message() -> ClientToServer {
    ClientToServer::CreateMeet {
        this_location_name: TEST_LOCATION.to_string(),
        password: TEST_PASSWORD.to_string(),
        endpoints: vec![test_endpoint_priority()],
    }
}

/// Creates a standard JoinMeet message for testing
pub fn test_join_meet_message() -> ClientToServer {
    ClientToServer::JoinMeet {
        meet_id: TEST_MEET_ID.to_string(),
        password: TEST_PASSWORD.to_string(),
        location_name: TEST_LOCATION.to_string(),
    }
} 