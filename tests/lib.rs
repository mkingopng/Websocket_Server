// ============
// tests/lib.rs
// ============
//! WebSocket Server Test Suite
//! This crate contains tests for the WebSocket Server.
//!
//! Unit tests have been moved to their respective source files as #[cfg(test)] modules.
//! This test crate now only contains integration and performance tests.

// Common test utilities - export for use in all test modules
#[path = "test_utils.rs"]
pub mod test_utils;

#[cfg(test)]
mod integration {
    // Integration tests
    mod auth_flow_tests;
    mod meet_simulation_test;
    mod websocket_flow_tests;
}

#[cfg(test)]
mod performance {
    // Performance tests
    mod websocket_throughput;
}
