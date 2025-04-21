// ============================
// server/src/auth.rs
// ============================
//! Password hashing + session token handling.
use scrypt::{password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng}, Scrypt};
use uuid::Uuid;
use tokio::sync::RwLock;
use std::{collections::HashMap, sync::Arc, time::{Duration, SystemTime}};

const SESSION_TTL: Duration = Duration::from_secs(60 * 60 * 24 * 7);

#[derive(Clone)]
pub struct Session {
    pub meet_id: String,
    pub location_name: String,
    pub priority: u8,
    pub created_at: SystemTime,
    pub expires_at: SystemTime,
}

#[derive(Clone)]
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<String, Session>>>,
}

impl SessionManager {
    pub fn new() -> Self {
        SessionManager {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn hash_password(plain: &str) -> anyhow::Result<String> {
        let salt = SaltString::generate(&mut OsRng);
        let hash = Scrypt
            .hash_password(plain.as_bytes(), &salt)?
            .to_string();
        Ok(hash)
    }

    pub fn verify_password(hash: &str, plain: &str) -> bool {
        let parsed_hash = match PasswordHash::new(hash) {
            Ok(h) => h,
            Err(_) => return false,
        };
        Scrypt.verify_password(plain.as_bytes(), &parsed_hash).is_ok()
    }

    pub async fn new_session(&self, meet_id: String, location_name: String, priority: u8) -> String {
        let token = uuid::Uuid::new_v4().to_string();
        let now = SystemTime::now();
        let session = Session {
            meet_id,
            location_name,
            priority,
            created_at: now,
            expires_at: now + Duration::from_secs(3600 * 24), // 24 hours
        };
        let mut sessions = self.sessions.write().await;
        sessions.insert(token.clone(), session);
        token
    }

    pub async fn get(&self, token: &str) -> Option<Session> {
        let sessions = self.sessions.read().await;
        sessions.get(token).cloned()
    }

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
}