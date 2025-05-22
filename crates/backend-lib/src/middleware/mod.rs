// crates/backend-lib/src/middleware/mod.rs

//! Middleware for the `OpenLifter` WebSocket server.

pub mod rate_limit;

pub use rate_limit::rate_limit;

#[cfg(test)]
mod middleware_tests;
