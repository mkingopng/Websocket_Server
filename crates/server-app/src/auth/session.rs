// ============================
// crates/server-app/src/auth/session.rs
// ============================
//! Session token handling and management.
use super::{token_generator::generate_secure_token, AuthService};
use crate::messages::Session;
use async_trait::async_trait;
use chrono;
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

/// Absolute maximum session lifetime
pub const SESSION_ABSOLUTE_TTL: std::time::Duration = std::time::Duration::from_secs(24 * 60 * 60); // 24 hours

/// Idle timeout for sessions
pub const SESSION_IDLE_TTL: std::time::Duration = std::time::Duration::from_secs(60 * 60); // 1 hour

/// Security event types for logging
#[derive(Debug, Clone, Copy)]
enum SecurityEvent {
    SessionCreated,
    SessionValidated,
    SessionExpired,
    SessionRemoved,
    SessionRotated,
    InvalidSessionAccess,
    CsrfValidationFailed,
    CsrfValidationSuccess,
}

/// Log a security event
fn log_security_event(event: SecurityEvent, details: &str) {
    let timestamp = chrono::Local::now()
        .format("%Y-%m-%d %H:%M:%S%.3f")
        .to_string();
    let event_str = format!("{:?}", event);
    tracing::info!("[SECURITY] [{timestamp}] [{event_str}] {details}");
}

/// Session entry with enhanced security features
#[derive(Debug, Clone)]
pub struct SessionEntry {
    /// The session data
    pub session: Session,
    /// When the session was created
    pub created_at: Instant,
    /// When the session was last accessed
    pub last_active: Instant,
    /// CSRF token for protection
    pub csrf_token: String,
    /// Duration since creation (for persistence)
    pub created_at_duration: std::time::Duration,
    /// Duration since last activity (for persistence)
    pub last_active_duration: std::time::Duration,
}

/// Session manager for handling authentication tokens
#[derive(Debug, Clone)]
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<String, SessionEntry>>>,
    absolute_ttl: std::time::Duration,
    idle_ttl: std::time::Duration,
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
            absolute_ttl: SESSION_ABSOLUTE_TTL,
            idle_ttl: SESSION_IDLE_TTL,
        }
    }

    /// Create a new session manager with custom timeouts
    pub fn new_with_timeouts(
        absolute_ttl: std::time::Duration,
        idle_ttl: std::time::Duration,
    ) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            absolute_ttl,
            idle_ttl,
        }
    }

    /// Create a new session
    pub async fn create_session(
        &self,
        meet_id: String,
        location_name: String,
        priority: u8,
    ) -> Session {
        // Create a new session with a secure token instead of UUID
        let token = generate_secure_token();
        let csrf_token = generate_secure_token();

        let session = Session {
            token: token.clone(),
            meet_id: meet_id.clone(),
            location_name: location_name.clone(),
            priority,
        };

        let now = Instant::now();

        // Store the session with additional security information
        self.sessions.write().await.insert(
            token.clone(),
            SessionEntry {
                session: session.clone(),
                created_at: now,
                last_active: now,
                csrf_token,
                created_at_duration: std::time::Duration::from_secs(0),
                last_active_duration: std::time::Duration::from_secs(0),
            },
        );

        // Log session creation
        log_security_event(
            SecurityEvent::SessionCreated,
            &format!(
                "Created new session for meet: {}, location: {}",
                meet_id, location_name
            ),
        );

        session
    }

    /// Add an existing session (for persistent storage)
    pub async fn add_session(
        &self,
        token: String,
        session: Session,
        created_at_duration: std::time::Duration,
        last_active_duration: std::time::Duration,
        csrf_token: String,
    ) -> Result<(), anyhow::Error> {
        // Recreate Instant values from durations
        let now = Instant::now();
        let created_at = now - created_at_duration;
        let last_active = now - last_active_duration;

        // Store the session
        self.sessions.write().await.insert(
            token.clone(),
            SessionEntry {
                session: session.clone(),
                created_at,
                last_active,
                csrf_token,
                created_at_duration,
                last_active_duration,
            },
        );

        // Log session creation
        log_security_event(
            SecurityEvent::SessionCreated,
            &format!(
                "Restored session for meet: {}, location: {}",
                session.meet_id, session.location_name
            ),
        );

        Ok(())
    }

    /// Get all sessions (for persistent storage)
    pub async fn get_all_sessions(&self) -> Result<HashMap<String, SessionEntry>, anyhow::Error> {
        let sessions = self.sessions.read().await;
        let mut result = HashMap::new();

        let now = Instant::now();

        // Clone all sessions with updated durations
        for (token, entry) in sessions.iter() {
            let created_at_duration = now.duration_since(entry.created_at);
            let last_active_duration = now.duration_since(entry.last_active);

            let updated_entry = SessionEntry {
                session: entry.session.clone(),
                created_at: entry.created_at,
                last_active: entry.last_active,
                csrf_token: entry.csrf_token.clone(),
                created_at_duration,
                last_active_duration,
            };

            result.insert(token.clone(), updated_entry);
        }

        Ok(result)
    }

    /// Get CSRF token for a session
    pub async fn get_csrf_token(&self, token: &str) -> Option<String> {
        // Acquire a write lock immediately to avoid read->write deadlock
        let mut sessions = self.sessions.write().await;

        if let Some(entry) = sessions.get_mut(token) {
            // Update last active time
            entry.last_active = Instant::now();
            return Some(entry.csrf_token.clone());
        }

        // Log invalid session access
        log_security_event(
            SecurityEvent::InvalidSessionAccess,
            &format!("Attempted to get CSRF token for invalid session: {}", token),
        );

        None
    }

    /// Get a session by token
    pub async fn get_session(&self, token: &str) -> Option<Session> {
        let sessions = self.sessions.read().await;
        let entry = sessions.get(token)?;

        // Check if session has expired
        let now = Instant::now();
        let age = now.duration_since(entry.created_at);
        let idle = now.duration_since(entry.last_active);

        if age > self.absolute_ttl || idle > self.idle_ttl {
            // Session has expired
            log_security_event(
                SecurityEvent::SessionExpired,
                &format!(
                    "Session expired for meet: {}, location: {}",
                    entry.session.meet_id, entry.session.location_name
                ),
            );
            return None;
        }

        // Clone session before dropping read lock
        let session = entry.session.clone();

        drop(sessions); // Release read lock
        let mut sessions = self.sessions.write().await;
        if let Some(entry) = sessions.get_mut(token) {
            entry.last_active = now;
            entry.last_active_duration = std::time::Duration::from_secs(0);
        }

        Some(session)
    }

    /// Validate a session token
    pub async fn validate_session(&self, token: &str) -> bool {
        let sessions = self.sessions.read().await;
        let entry = match sessions.get(token) {
            Some(entry) => entry,
            None => {
                log_security_event(
                    SecurityEvent::InvalidSessionAccess,
                    &format!("Invalid session token: {}", token),
                );
                return false;
            },
        };

        // Check if session has expired
        let now = Instant::now();
        let age = now.duration_since(entry.created_at);
        let idle = now.duration_since(entry.last_active);

        if age > self.absolute_ttl || idle > self.idle_ttl {
            // Session has expired
            log_security_event(
                SecurityEvent::SessionExpired,
                &format!(
                    "Session expired for meet: {}, location: {}",
                    entry.session.meet_id, entry.session.location_name
                ),
            );
            return false;
        }

        // Clone session info before dropping read lock
        let meet_id = entry.session.meet_id.clone();
        let location_name = entry.session.location_name.clone();

        drop(sessions); // Release read lock
        let mut sessions = self.sessions.write().await;
        if let Some(entry) = sessions.get_mut(token) {
            entry.last_active = now;
            entry.last_active_duration = std::time::Duration::from_secs(0);
        }

        log_security_event(
            SecurityEvent::SessionValidated,
            &format!(
                "Session validated for meet: {}, location: {}",
                meet_id, location_name
            ),
        );

        true
    }

    /// Remove a session
    pub async fn remove_session(&self, token: &str) {
        let mut sessions = self.sessions.write().await;
        if let Some(entry) = sessions.remove(token) {
            log_security_event(
                SecurityEvent::SessionRemoved,
                &format!(
                    "Removed session for meet: {}, location: {}",
                    entry.session.meet_id, entry.session.location_name
                ),
            );
        }
    }

    /// Rotate a session token
    pub async fn rotate_session(&self, old_token: &str) -> Option<String> {
        let mut sessions = self.sessions.write().await;
        let entry = sessions.remove(old_token)?;

        // Generate new token
        let new_token = generate_secure_token();
        let new_csrf_token = generate_secure_token();

        // Clone meet_id and location_name before moving
        let meet_id = entry.session.meet_id.clone();
        let location_name = entry.session.location_name.clone();

        // Create new session entry
        let now = Instant::now();
        let new_entry = SessionEntry {
            session: Session {
                token: new_token.clone(),
                meet_id,
                location_name,
                priority: entry.session.priority,
            },
            created_at: now,
            last_active: now,
            csrf_token: new_csrf_token,
            created_at_duration: std::time::Duration::from_secs(0),
            last_active_duration: std::time::Duration::from_secs(0),
        };

        // Store new session
        sessions.insert(new_token.clone(), new_entry);

        log_security_event(
            SecurityEvent::SessionRotated,
            &format!(
                "Rotated session for meet: {}, location: {}",
                entry.session.meet_id, entry.session.location_name
            ),
        );

        Some(new_token)
    }

    /// Clean up expired sessions
    pub async fn cleanup_expired_sessions(&self) {
        let mut sessions = self.sessions.write().await;
        let now = Instant::now();
        let mut expired = Vec::new();

        // Find expired sessions
        for (token, entry) in sessions.iter() {
            let age = now.duration_since(entry.created_at);
            let idle = now.duration_since(entry.last_active);

            if age > self.absolute_ttl || idle > self.idle_ttl {
                expired.push(token.clone());
            }
        }

        // Remove expired sessions
        for token in expired {
            if let Some(entry) = sessions.remove(&token) {
                log_security_event(
                    SecurityEvent::SessionExpired,
                    &format!(
                        "Cleaned up expired session for meet: {}, location: {}",
                        entry.session.meet_id, entry.session.location_name
                    ),
                );
            }
        }
    }

    /// Verify CSRF token
    pub async fn verify_csrf_token(&self, session_token: &str, csrf_token: &str) -> bool {
        let sessions = self.sessions.read().await;
        let entry = match sessions.get(session_token) {
            Some(entry) => entry,
            None => {
                log_security_event(
                    SecurityEvent::CsrfValidationFailed,
                    &format!("Invalid session token: {}", session_token),
                );
                return false;
            },
        };

        // Use constant-time comparison
        let valid = constant_time_compare(&entry.csrf_token, csrf_token);

        if valid {
            log_security_event(
                SecurityEvent::CsrfValidationSuccess,
                &format!(
                    "CSRF validation successful for meet: {}, location: {}",
                    entry.session.meet_id, entry.session.location_name
                ),
            );
        } else {
            log_security_event(
                SecurityEvent::CsrfValidationFailed,
                &format!(
                    "CSRF validation failed for meet: {}, location: {}",
                    entry.session.meet_id, entry.session.location_name
                ),
            );
        }

        valid
    }

    /// Return count of active sessions
    pub async fn active_session_count(&self) -> usize {
        let sessions = self.sessions.read().await;
        sessions.len()
    }
}

/// Constant-time comparison to prevent timing attacks
fn constant_time_compare(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }

    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();

    let mut result = 0;
    for i in 0..a.len() {
        result |= a_bytes[i] ^ b_bytes[i];
    }

    result == 0
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::{self, timeout};

    #[tokio::test]
    async fn test_session_create_validate() {
        // Add timeout to prevent test from hanging indefinitely
        timeout(Duration::from_secs(5), async {
            let sm = SessionManager::new();
            let session = sm
                .create_session("test-meet".to_string(), "Test Location".to_string(), 5)
                .await;

            // Validate the session
            assert!(sm.validate_session(&session.token).await);

            // Get session
            let retrieved = sm.get_session(&session.token).await.unwrap();
            assert_eq!(retrieved.meet_id, "test-meet");
            assert_eq!(retrieved.location_name, "Test Location");
            assert_eq!(retrieved.priority, 5);
        })
        .await
        .expect("Test timed out");
    }

    #[tokio::test]
    async fn test_csrf_token() {
        timeout(Duration::from_secs(5), async {
            let sm = SessionManager::new();
            let session = sm
                .create_session("test-meet".to_string(), "Test Location".to_string(), 5)
                .await;

            // Get CSRF token
            let csrf_token = sm.get_csrf_token(&session.token).await.unwrap();

            // Verify correct CSRF token
            assert!(sm.verify_csrf_token(&session.token, &csrf_token).await);

            // Verify incorrect CSRF token
            assert!(!sm.verify_csrf_token(&session.token, "invalid-token").await);

            // Verify CSRF with invalid session
            assert!(!sm.verify_csrf_token("invalid-session", &csrf_token).await);
        })
        .await
        .expect("Test timed out");
    }

    #[tokio::test]
    async fn test_session_expiry() {
        timeout(Duration::from_secs(5), async {
            println!("Starting test_session_expiry");
            // Create session manager with short timeouts for testing
            let sm = SessionManager::new_with_timeouts(
                Duration::from_millis(300), // 300ms absolute timeout (reduced from 500ms)
                Duration::from_millis(200), // 200ms idle timeout (reduced from 300ms)
            );
            println!("Created SessionManager");

            let session = sm
                .create_session("test-meet".to_string(), "Test Location".to_string(), 5)
                .await;
            println!("Created session");

            // Initially valid
            println!("Validating session for the first time");
            assert!(sm.validate_session(&session.token).await);
            println!("Session validated successfully");

            // Wait for idle timeout (slightly more than timeout)
            println!("Sleeping for idle timeout (220ms)");
            time::sleep(Duration::from_millis(220)).await;
            println!("Woke up from sleep");

            // Should be expired due to idle timeout
            println!("Validating session after idle timeout");
            assert!(
                !sm.validate_session(&session.token).await,
                "Session should expire after idle timeout"
            );
            println!("Session expired correctly after idle timeout");

            // Create a new session to test absolute timeout
            println!("Creating a second session");
            let session2 = sm
                .create_session("test-meet2".to_string(), "Test Location".to_string(), 5)
                .await;
            println!("Created second session");

            // Keep it active by validating
            println!("Validating second session");
            assert!(sm.validate_session(&session2.token).await);
            println!("Second session validated");

            // Sleep for less than idle timeout
            println!("Sleeping for 100ms (less than idle timeout)");
            time::sleep(Duration::from_millis(100)).await;
            println!("Woke up from short sleep");

            // Should still be valid
            println!("Validating session after short sleep");
            assert!(
                sm.validate_session(&session2.token).await,
                "Session should be valid before idle timeout"
            );
            println!("Session still valid after short sleep");

            // Wait for absolute timeout (slightly more than timeout)
            println!("Sleeping for absolute timeout (220ms)");
            time::sleep(Duration::from_millis(220)).await;
            println!("Woke up from absolute timeout sleep");

            // Should be expired due to absolute timeout
            println!("Validating session after absolute timeout");
            assert!(
                !sm.validate_session(&session2.token).await,
                "Session should expire after absolute timeout"
            );
            println!("Session expired correctly after absolute timeout");
            println!("Test completed successfully");
        })
        .await
        .expect("Test timed out");
    }

    #[tokio::test]
    async fn test_session_rotation() {
        timeout(Duration::from_secs(5), async {
            let sm = SessionManager::new();
            let session = sm
                .create_session("test-meet".to_string(), "Test Location".to_string(), 5)
                .await;
            let old_token = session.token.clone();

            // Rotate session
            let new_token = sm.rotate_session(&old_token).await.unwrap();

            // Old token should be invalid
            assert!(!sm.validate_session(&old_token).await);

            // New token should be valid
            assert!(sm.validate_session(&new_token).await);

            // Get session with new token
            let rotated_session = sm.get_session(&new_token).await.unwrap();
            assert_eq!(rotated_session.meet_id, "test-meet");
            assert_eq!(rotated_session.location_name, "Test Location");
            assert_eq!(rotated_session.priority, 5);
        })
        .await
        .expect("Test timed out");
    }

    #[tokio::test]
    async fn test_constant_time_compare() {
        // Test equal strings
        assert!(constant_time_compare("abc123", "abc123"));

        // Test different lengths
        assert!(!constant_time_compare("abc", "abcd"));

        // Test same length but different content
        assert!(!constant_time_compare("abc123", "abc124"));
    }
}
