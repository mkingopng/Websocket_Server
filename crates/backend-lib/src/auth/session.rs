// ============================
// openlifter-backend-lib/src/auth/session.rs
// ============================
//! Session token handling and management.
use uuid::Uuid;
use tokio::sync::RwLock;
use std::{collections::HashMap, sync::Arc, time::{Duration, SystemTime}};
use metrics::{counter, gauge};

/// Session TTL (time to live)
pub const SESSION_TTL: Duration = Duration::from_secs(60 * 60 * 24 * 7); // 7 days

/// Session information
#[derive(Clone)]
pub struct Session {
    pub meet_id: String,
    pub location_name: String,
    pub priority: u8,
    pub created_at: SystemTime,
    pub expires_at: SystemTime,
}

/// Session manager for handling authentication tokens
#[derive(Clone)]
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<String, Session>>>,
}

impl SessionManager {
    /// Create a new session manager
    pub fn new() -> Self {
        let manager = SessionManager {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        };
        
        // Spawn the session cleanup task
        let manager_clone = manager.clone();
        tokio::spawn(async move {
            manager_clone.cleanup_task().await;
        });
        
        manager
    }

    /// Create a new session
    pub async fn new_session(&self, meet_id: String, location_name: String, priority: u8) -> String {
        let token = uuid::Uuid::new_v4().to_string();
        let now = SystemTime::now();
        let session = Session {
            meet_id,
            location_name,
            priority,
            created_at: now,
            expires_at: now + SESSION_TTL,
        };
        
        let mut sessions = self.sessions.write().await;
        sessions.insert(token.clone(), session);
        
        // Update metrics
        counter!("session.created", 1);
        gauge!("session.active", sessions.len() as f64);
        
        token
    }

    /// Get a session by token
    pub async fn get(&self, token: &str) -> Option<Session> {
        let sessions = self.sessions.read().await;
        sessions.get(token).cloned()
    }

    /// Validate a session token
    pub async fn validate_session(&self, token: &str) -> bool {
        let sessions = self.sessions.read().await;
        if let Some(session) = sessions.get(token) {
            let now = SystemTime::now();
            if now < session.expires_at {
                return true;
            }
        }
        false
    }
    
    /// Cleanup task that runs periodically to remove expired sessions
    async fn cleanup_task(&self) {
        let cleanup_interval = Duration::from_secs(60 * 60); // 1 hour
        
        loop {
            tokio::time::sleep(cleanup_interval).await;
            
            let mut sessions = self.sessions.write().await;
            let now = SystemTime::now();
            let before_count = sessions.len();
            
            sessions.retain(|_, session| now < session.expires_at);
            
            let after_count = sessions.len();
            let removed = before_count - after_count;
            
            if removed > 0 {
                counter!("session.expired", removed as u64);
                gauge!("session.active", after_count as f64);
            }
        }
    }
} 