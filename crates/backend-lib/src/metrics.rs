// ==============
// crates/backend-lib/src/metrics.rs

//! Central place for Prometheus metric keys
pub const WS_CONNECTION: &str = "ws.connection";
pub const WS_ACTIVE: &str = "ws.active";
pub const MEET_CREATED: &str = "meet.created";
pub const MEET_JOINED: &str = "meet.joined";
pub const UPDATE_ACCEPTED: &str = "update.accepted";
pub const UPDATE_BATCH_SIZE: &str = "update.batch_size";
