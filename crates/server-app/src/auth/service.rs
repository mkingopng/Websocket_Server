// =============
// crates/server-app/src/auth/service.rs
// =============
//! This module defines the `AuthService` trait, which is used for authentication
use crate::messages::Session;
use async_trait::async_trait;
use std::any::Any;

#[async_trait]
pub trait AuthService: Send + Sync {
    async fn new_session(&self, meet_id: String, location_name: String, priority: u8) -> String;
    async fn get_session(&self, token: &str) -> Option<Session>;
    async fn validate_session(&self, token: &str) -> bool;

    /// Convert self to Any for downcasting
    fn as_any(&self) -> &dyn Any;
}
