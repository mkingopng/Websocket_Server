// ============================
// crates/backend-lib/src/auth/session.rs
// ============================
//! Session token handling and management.
use super::AuthService;
use crate::messages::Session;
use async_trait::async_trait;
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

/// Session TTL (time to live)
pub const SESSION_TTL: std::time::Duration = std::time::Duration::from_secs(3600); // 1 hour

/// Session manager for handling authentication tokens
#[derive(Debug, Clone)]
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<String, (Session, Instant)>>>,
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
        self.sessions
            .write()
            .await
            .insert(token, (session.clone(), Instant::now()));
        session
    }

    /// Get a session by token
    pub async fn get_session(&self, token: &str) -> Option<Session> {
        if let Some((session, creation_time)) = self.sessions.read().await.get(token) {
            // Check if session is expired
            if creation_time.elapsed() > SESSION_TTL {
                return None;
            }
            return Some(session.clone());
        }
        None
    }

    /// Validate a session by token
    pub async fn validate_session(&self, token: &str) -> bool {
        if let Some((_, creation_time)) = self.sessions.read().await.get(token) {
            // Check if session is expired
            if creation_time.elapsed() > SESSION_TTL {
                return false;
            }
            return true;
        }
        false
    }

    /// Remove a session by token
    pub async fn remove_session(&self, token: &str) {
        self.sessions.write().await.remove(token);
    }

    /// Cleanup task that runs periodically to remove expired sessions
    pub async fn cleanup_expired_sessions(&self) {
        let mut sessions = self.sessions.write().await;
        let _now = Instant::now();

        // Remove all sessions that have expired
        sessions.retain(|_, (_, creation_time)| {
            // Keep session if not expired (creation_time + TTL > now)
            creation_time.elapsed() < SESSION_TTL
        });

        // Log the number of active sessions after cleanup
        println!(
            "Session cleanup complete: {} active sessions",
            sessions.len()
        );
    }

    /// Return count of active sessions
    pub async fn active_session_count(&self) -> usize {
        let sessions = self.sessions.read().await;
        sessions.len()
    }
}

#[async_trait]
impl AuthService for SessionManager {
    async fn new_session(&self, meet_id: String, location_name: String, priority: u8) -> String {
        let session = self.create_session(meet_id, location_name, priority).await;
        session.token
    }

    async fn get_session(&self, token: &str) -> Option<Session> {
        // Call the method from SessionManager, not recursively
        SessionManager::get_session(self, token).await
    }

    async fn validate_session(&self, token: &str) -> bool {
        // Call the method from SessionManager, not recursively
        SessionManager::validate_session(self, token).await
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
