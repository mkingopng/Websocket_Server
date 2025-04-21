use async_trait::async_trait;
use crate::messages::Session;

#[async_trait]
pub trait AuthService: Send + Sync {
    async fn new_session(&self, meet_id: String, location_name: String, priority: u8) -> String;
    async fn get_session(&self, token: &str) -> Option<Session>;
    async fn validate_session(&self, token: &str) -> bool;
} 