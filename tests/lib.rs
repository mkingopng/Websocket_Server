//! WebSocket Server Test Suite
//!
//! This crate contains tests for the WebSocket Server.

// Common test utilities - export for use in all test modules
#[path = "test_utils.rs"]
pub mod test_utils;

#[cfg(test)]
mod unit {
    // Unit tests
    mod config_tests;
    mod error_tests;
    mod live_handler_tests;
    mod meet_tests;
    mod middleware_tests;
    mod password_tests;
    mod rate_limit_tests;
    mod ws_router_tests;
}

#[cfg(test)]
mod integration {
    // Integration tests
    mod auth_flow_tests;
}

#[cfg(test)]
mod performance {
    // Performance tests
    mod websocket_throughput;
}
