// ============================
// openlifter-backend-lib/src/auth/session.rs
// ============================
//! Session token handling and management.
use super::AuthService;
use crate::messages::Session;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Session TTL (time to live)
pub const SESSION_TTL: std::time::Duration = std::time::Duration::from_secs(3600); // 1 hour

/// Session manager for handling authentication tokens
#[derive(Debug, Clone)]
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<String, Session>>>,
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionManager {
    /// Create a new session manager
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new session
    pub async fn create_session(
        &self,
        meet_id: String,
        location_name: String,
        priority: u8,
    ) -> Session {
        let session = Session::new(meet_id, location_name, priority);
        let token = session.token.clone();
        self.sessions.write().await.insert(token, session.clone());
        session
    }

    /// Get a session by token
    pub async fn get_session(&self, token: &str) -> Option<Session> {
        self.sessions.read().await.get(token).cloned()
    }

    /// Validate a session by token
    pub async fn validate_session(&self, token: &str) -> bool {
        self.sessions.read().await.contains_key(token)
    }

    /// Remove a session by token
    pub async fn remove_session(&self, token: &str) {
        self.sessions.write().await.remove(token);
    }

    /// Cleanup task that runs periodically to remove expired sessions
    pub async fn cleanup_expired_sessions(&self) {
        // TODO: Implement session expiration
        let mut sessions = self.sessions.write().await;
        sessions.retain(|_, _| true);
    }
}

#[async_trait]
impl AuthService for SessionManager {
    async fn new_session(&self, meet_id: String, location_name: String, priority: u8) -> String {
        let session = self.create_session(meet_id, location_name, priority).await;
        session.token
    }

    async fn get_session(&self, token: &str) -> Option<Session> {
        self.get_session(token).await
    }

    async fn validate_session(&self, token: &str) -> bool {
        self.validate_session(token).await
    }
}
