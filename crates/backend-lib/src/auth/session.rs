// ============================
// openlifter-backend-lib/src/auth/session.rs
// ============================
//! Session token handling and management.
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use parking_lot::RwLock;
use uuid::Uuid;

/// Session TTL (time to live)
pub const SESSION_TTL: Duration = Duration::from_secs(60 * 60 * 24 * 7); // 7 days

/// Session information
#[derive(Clone)]
pub struct Session {
    pub meet_id: String,
    pub location_name: String,
    pub priority: u8,
}

/// Session manager for handling authentication tokens
#[derive(Clone)]
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
        SessionManager {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new session
    pub fn new_session(&self, meet_id: String, location_name: String, priority: u8) -> String {
        let token = Uuid::new_v4().to_string();
        let session = Session {
            meet_id,
            location_name,
            priority,
        };
        let mut sessions = self.sessions.write();
        sessions.insert(token.clone(), session);
        token
    }

    /// Get a session by token
    pub fn get_session(&self, token: &str) -> Option<Session> {
        let sessions = self.sessions.read();
        sessions.get(token).cloned()
    }

    /// Validate a session token
    pub fn validate_session(&self, token: &str) -> bool {
        let sessions = self.sessions.read();
        sessions.contains_key(token)
    }
    
    /// Cleanup task that runs periodically to remove expired sessions
    pub fn cleanup_expired_sessions(&self) {
        let mut sessions = self.sessions.write();
        sessions.retain(|_, _| true); // TODO: Implement actual expiration logic
    }
} 