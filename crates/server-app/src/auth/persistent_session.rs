// ============================
// crates/server-app/src/auth/persistent_session.rs
// ============================
/** Persistent session storage with encryption
This module extends the SessionManager with persistent storage capabilities,
allowing sessions to survive server restarts. */
use super::{session::SessionManager, AuthService};
use crate::messages::Session;
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use std::{
    any::Any,
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};
use tokio::{fs as tokio_fs, sync::RwLock, time};

/// Security event types for logging
#[derive(Debug, Clone, Copy)]
enum SecurityEvent {
    SessionLoaded,
    SessionSaved,
    SessionEncryptionFailed,
    SessionDecryptionFailed,
}

/// Log a security event
fn log_security_event(event: SecurityEvent, details: &str) {
    let timestamp = chrono::Local::now()
        .format("%Y-%m-%d %H:%M:%S%.3f")
        .to_string();
    let event_str = format!("{:?}", event);
    println!("[SECURITY] [{timestamp}] [{event_str}] {details}");
}

/// Session entry that can be serialized
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistentSessionEntry {
    /// The session data
    session: Session,
    /// When the session was created (as UTC timestamp for serialization)
    created_at: DateTime<Utc>,
    /// When the session was last accessed (as UTC timestamp for serialization)
    last_active: DateTime<Utc>,
    /// CSRF token for protection
    csrf_token: String,
}

/// Persistent session manager with encryption
#[derive(Debug, Clone)]
pub struct PersistentSessionManager {
    /// Inner session manager
    inner: SessionManager,
    /// Path to store sessions
    storage_path: PathBuf,
    /// Encryption key
    encryption_key: [u8; 32],
    /// Auto-save interval
    save_interval: Duration,
    /// Last save timestamp
    last_save: Arc<RwLock<Instant>>,
}

impl PersistentSessionManager {
    /** Create a new persistent session manager
    # Arguments
    * `storage_path` - Path to store sessions
    * `encryption_key` - 32-byte key for AES-GCM encryption (if None, a random key will be generated and saved) */
    pub async fn new<P: AsRef<Path>>(storage_path: P) -> Result<Self, anyhow::Error> {
        // Create directory if it doesn't exist
        let storage_path = storage_path.as_ref().to_path_buf();
        fs::create_dir_all(&storage_path)?;

        // Load or generate encryption key
        let key_path = storage_path.join("session_key");
        let encryption_key = if key_path.exists() {
            // Load existing key
            let key_data = fs::read(&key_path)?;
            let mut key = [0u8; 32];
            if key_data.len() != 32 {
                return Err(anyhow::anyhow!("Invalid encryption key length"));
            }
            key.copy_from_slice(&key_data);
            key
        } else {
            // Generate new key
            let mut key = [0u8; 32];
            OsRng.fill_bytes(&mut key);
            fs::write(&key_path, &key)?;
            key
        };

        // Create session manager
        let inner = SessionManager::new();

        // Create persistent manager
        let manager = Self {
            inner,
            storage_path,
            encryption_key,
            save_interval: Duration::from_secs(60),
            last_save: Arc::new(RwLock::new(Instant::now())),
        };

        // Load sessions
        manager.load_sessions().await?;

        // Start auto-save task
        let cloned = manager.clone();
        tokio::spawn(async move {
            cloned.auto_save_task().await;
        });

        Ok(manager)
    }

    /** Create a new persistent session manager with custom timeouts
    # Arguments
    * `storage_path` - Path to store sessions
    * `absolute_ttl` - Absolute session timeout
    * `idle_ttl` - Idle session timeout */
    pub async fn new_with_timeouts<P: AsRef<Path>>(
        storage_path: P,
        absolute_ttl: Duration,
        idle_ttl: Duration,
    ) -> Result<Self, anyhow::Error> {
        // Create directory if it doesn't exist
        let storage_path = storage_path.as_ref().to_path_buf();
        fs::create_dir_all(&storage_path)?;

        // Load or generate encryption key
        let key_path = storage_path.join("session_key");
        let encryption_key = if key_path.exists() {
            // Load existing key
            let key_data = fs::read(&key_path)?;
            let mut key = [0u8; 32];
            if key_data.len() != 32 {
                return Err(anyhow::anyhow!("Invalid encryption key length"));
            }
            key.copy_from_slice(&key_data);
            key
        } else {
            // Generate new key
            let mut key = [0u8; 32];
            OsRng.fill_bytes(&mut key);
            fs::write(&key_path, &key)?;
            key
        };

        // Create session manager with custom timeouts
        let inner = SessionManager::new_with_timeouts(absolute_ttl, idle_ttl);

        // Create persistent manager
        let manager = Self {
            inner,
            storage_path,
            encryption_key,
            save_interval: Duration::from_secs(60),
            last_save: Arc::new(RwLock::new(Instant::now())),
        };

        // Load sessions
        manager.load_sessions().await?;

        // Start auto-save task
        let cloned = manager.clone();
        tokio::spawn(async move {
            cloned.auto_save_task().await;
        });

        Ok(manager)
    }

    /// Save sessions to disk
    pub async fn save_sessions(&self) -> Result<(), anyhow::Error> {
        // Get sessions from inner manager
        let sessions = self.inner.get_all_sessions().await?;
        if sessions.is_empty() {
            return Ok(());
        }

        // Convert to serializable format
        let mut persistent_entries = HashMap::new();
        let session_count = sessions.len();

        for (token, entry) in &sessions {
            // Convert Instant to DateTime
            let created_at = SystemTime::now() - entry.created_at_duration;
            let last_active = SystemTime::now() - entry.last_active_duration;

            // Create persistent entry
            let persistent_entry = PersistentSessionEntry {
                session: entry.session.clone(),
                created_at: DateTime::from(created_at),
                last_active: DateTime::from(last_active),
                csrf_token: entry.csrf_token.clone(),
            };

            persistent_entries.insert(token.clone(), persistent_entry);
        }

        // Serialize
        let json = serde_json::to_string(&persistent_entries)?;

        // Encrypt
        let cipher = Aes256Gcm::new_from_slice(&self.encryption_key)?;
        let nonce_bytes = generate_nonce();
        let nonce = Nonce::from_slice(&nonce_bytes);

        let encrypted_data = match cipher.encrypt(nonce, json.as_bytes()) {
            Ok(data) => data,
            Err(err) => {
                log_security_event(
                    SecurityEvent::SessionEncryptionFailed,
                    &format!("Failed to encrypt sessions: {}", err),
                );
                return Err(anyhow::anyhow!("Encryption failed"));
            },
        };

        // Combine nonce and encrypted data
        let mut combined = Vec::with_capacity(nonce_bytes.len() + encrypted_data.len());
        combined.extend_from_slice(&nonce_bytes);
        combined.extend_from_slice(&encrypted_data);

        // Save to file
        let sessions_file = self.storage_path.join("sessions.dat");
        tokio_fs::write(&sessions_file, &combined).await?;

        // Update last save timestamp
        *self.last_save.write().await = Instant::now();

        // Log session save
        log_security_event(
            SecurityEvent::SessionSaved,
            &format!("Saved {} sessions to disk", session_count),
        );

        Ok(())
    }

    /// Load sessions from disk
    async fn load_sessions(&self) -> Result<(), anyhow::Error> {
        let sessions_file = self.storage_path.join("sessions.dat");
        if !sessions_file.exists() {
            return Ok(());
        }

        // Read file
        let combined = tokio_fs::read(&sessions_file).await?;
        if combined.len() < 12 {
            // Nonce is 12 bytes
            return Err(anyhow::anyhow!("Invalid session file"));
        }

        // Split nonce and encrypted data
        let nonce_bytes = &combined[..12];
        let encrypted_data = &combined[12..];

        // Decrypt
        let cipher = Aes256Gcm::new_from_slice(&self.encryption_key)?;
        let nonce = Nonce::from_slice(nonce_bytes);

        let decrypted_data = match cipher.decrypt(nonce, encrypted_data) {
            Ok(data) => data,
            Err(err) => {
                log_security_event(
                    SecurityEvent::SessionDecryptionFailed,
                    &format!("Failed to decrypt sessions: {}", err),
                );
                return Err(anyhow::anyhow!("Decryption failed"));
            },
        };

        // Deserialize
        let persistent_entries: HashMap<String, PersistentSessionEntry> =
            serde_json::from_slice(&decrypted_data)?;

        let entry_count = persistent_entries.len();

        // Import sessions to inner manager
        for (token, entry) in persistent_entries {
            // Skip expired sessions
            let now = Utc::now();
            let created_duration = now.signed_duration_since(entry.created_at);
            let last_active_duration = now.signed_duration_since(entry.last_active);

            if created_duration.to_std().is_ok() && last_active_duration.to_std().is_ok() {
                let created_at_duration = created_duration.to_std()?;
                let last_active_duration = last_active_duration.to_std()?;

                // Add to inner manager
                self.inner
                    .add_session(
                        token,
                        entry.session,
                        created_at_duration,
                        last_active_duration,
                        entry.csrf_token,
                    )
                    .await?;
            }
        }

        // Log session load
        log_security_event(
            SecurityEvent::SessionLoaded,
            &format!("Loaded {} sessions from disk", entry_count),
        );

        Ok(())
    }

    /// Auto-save task
    async fn auto_save_task(&self) {
        loop {
            // Sleep for a while
            time::sleep(Duration::from_secs(10)).await;

            // Check if we should save
            let last_save = *self.last_save.read().await;
            let now = Instant::now();
            if now.duration_since(last_save) > self.save_interval {
                // Save sessions
                if let Err(err) = self.save_sessions().await {
                    eprintln!("Error saving sessions: {}", err);
                }
            }
        }
    }

    // Delegate methods to inner session manager

    /// Create a new session
    pub async fn create_session(
        &self,
        meet_id: String,
        location_name: String,
        priority: u8,
    ) -> Session {
        let session = self
            .inner
            .create_session(meet_id, location_name, priority)
            .await;

        // Save sessions after creation
        if let Err(err) = self.save_sessions().await {
            eprintln!("Error saving sessions after creation: {}", err);
        }

        session
    }

    /// Get CSRF token for a session
    pub async fn get_csrf_token(&self, token: &str) -> Option<String> {
        self.inner.get_csrf_token(token).await
    }

    /// Get a session by token
    pub async fn get_session(&self, token: &str) -> Option<Session> {
        self.inner.get_session(token).await
    }

    /// Validate a session by token
    pub async fn validate_session(&self, token: &str) -> bool {
        self.inner.validate_session(token).await
    }

    /// Remove a session by token
    pub async fn remove_session(&self, token: &str) {
        self.inner.remove_session(token).await;

        // Save sessions after removal
        if let Err(err) = self.save_sessions().await {
            eprintln!("Error saving sessions after removal: {}", err);
        }
    }

    /// Rotate the session token for enhanced security
    pub async fn rotate_session(&self, old_token: &str) -> Option<String> {
        let result = self.inner.rotate_session(old_token).await;

        // Save sessions after rotation
        if result.is_some() {
            if let Err(err) = self.save_sessions().await {
                eprintln!("Error saving sessions after rotation: {}", err);
            }
        }

        result
    }

    /// Cleanup task that runs periodically to remove expired sessions
    pub async fn cleanup_expired_sessions(&self) {
        self.inner.cleanup_expired_sessions().await;

        // Save sessions after cleanup
        if let Err(err) = self.save_sessions().await {
            eprintln!("Error saving sessions after cleanup: {}", err);
        }
    }

    /// Return count of active sessions
    pub async fn active_session_count(&self) -> usize {
        self.inner.active_session_count().await
    }

    /// Verify a CSRF token for a session
    pub async fn verify_csrf_token(&self, session_token: &str, csrf_token: &str) -> bool {
        self.inner
            .verify_csrf_token(session_token, csrf_token)
            .await
    }
}

/// Generate a random nonce for AES-GCM
fn generate_nonce() -> [u8; 12] {
    let mut nonce = [0u8; 12];
    OsRng.fill_bytes(&mut nonce);
    nonce
}

#[async_trait]
impl AuthService for PersistentSessionManager {
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

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::time::{timeout, Duration};

    /// Create a test manager with auto-save disabled
    async fn setup_test_manager(
        temp_dir: &TempDir,
    ) -> Result<PersistentSessionManager, anyhow::Error> {
        // Create directory if it doesn't exist
        let storage_path = temp_dir.path().to_path_buf();
        fs::create_dir_all(&storage_path)?;

        // Load or generate encryption key
        let key_path = storage_path.join("session_key");
        let encryption_key = if key_path.exists() {
            // Load existing key
            let key_data = fs::read(&key_path)?;
            let mut key = [0u8; 32];
            if key_data.len() != 32 {
                return Err(anyhow::anyhow!("Invalid encryption key length"));
            }
            key.copy_from_slice(&key_data);
            key
        } else {
            // Generate new key
            let mut key = [0u8; 32];
            OsRng.fill_bytes(&mut key);
            fs::write(&key_path, key)?;
            key
        };

        // Create session manager
        let inner = SessionManager::new_with_timeouts(
            Duration::from_millis(500), // Short timeout for tests
            Duration::from_millis(300),
        );

        // Create persistent manager with a long save interval to prevent auto-saving during tests
        let manager = PersistentSessionManager {
            inner,
            storage_path,
            encryption_key,
            save_interval: Duration::from_secs(600), // 10 minutes - effectively disable auto-save
            last_save: Arc::new(RwLock::new(Instant::now())),
        };

        // Load sessions
        manager.load_sessions().await?;

        Ok(manager)
    }

    async fn setup() -> (PersistentSessionManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let manager = setup_test_manager(&temp_dir).await.unwrap();
        (manager, temp_dir)
    }

    #[tokio::test]
    async fn test_create_and_validate_session() {
        // Add timeout to prevent test from hanging indefinitely
        timeout(Duration::from_secs(5), async {
            let (manager, _temp_dir) = setup().await;

            // Create a session
            let session = manager
                .create_session("test-meet".to_string(), "Test Location".to_string(), 5)
                .await;

            // Validate the session
            assert!(manager.validate_session(&session.token).await);

            // Get the session
            let retrieved = manager.get_session(&session.token).await.unwrap();
            assert_eq!(retrieved.meet_id, "test-meet");
            assert_eq!(retrieved.location_name, "Test Location");
            assert_eq!(retrieved.priority, 5);

            // Explicitly save sessions and check
            manager.save_sessions().await.unwrap();
        })
        .await
        .expect("Test timed out");
    }

    #[tokio::test]
    async fn test_save_and_load_sessions() {
        timeout(Duration::from_secs(5), async {
            let temp_dir = TempDir::new().unwrap();

            // Scope for first manager to ensure it's dropped before creating second manager
            {
                // Create a session with the first manager
                let manager1 = setup_test_manager(&temp_dir).await.unwrap();
                let _session = manager1
                    .create_session("test-meet".to_string(), "Test Location".to_string(), 5)
                    .await;

                // Force save
                manager1.save_sessions().await.unwrap();
            }

            // Create a new manager that should load the session
            let manager2 = setup_test_manager(&temp_dir).await.unwrap();

            // Check sessions were loaded
            let sessions = manager2.inner.get_all_sessions().await.unwrap();
            assert!(!sessions.is_empty(), "Sessions should have been loaded");

            // Session token might be different, so we check by meet ID
            let found_session = sessions.values().any(|entry| {
                entry.session.meet_id == "test-meet"
                    && entry.session.location_name == "Test Location"
                    && entry.session.priority == 5
            });
            assert!(found_session, "Expected session not found");
        })
        .await
        .expect("Test timed out");
    }

    #[tokio::test]
    async fn test_session_removal() {
        timeout(Duration::from_secs(5), async {
            let (manager, _temp_dir) = setup().await;

            // Create a session
            let session = manager
                .create_session("test-meet".to_string(), "Test Location".to_string(), 5)
                .await;

            // Validate the session
            assert!(manager.validate_session(&session.token).await);

            // Remove the session
            manager.remove_session(&session.token).await;

            // Session should no longer be valid
            assert!(!manager.validate_session(&session.token).await);
        })
        .await
        .expect("Test timed out");
    }

    #[tokio::test]
    async fn test_session_rotation() {
        timeout(Duration::from_secs(5), async {
            let (manager, _temp_dir) = setup().await;

            // Create a session
            let session = manager
                .create_session("test-meet".to_string(), "Test Location".to_string(), 5)
                .await;

            // Rotate the session
            let new_token = manager.rotate_session(&session.token).await.unwrap();

            // Old token should be invalid
            assert!(!manager.validate_session(&session.token).await);

            // New token should be valid
            assert!(manager.validate_session(&new_token).await);

            // Get the session with the new token
            let retrieved = manager.get_session(&new_token).await.unwrap();
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
            let (manager, _temp_dir) = setup().await;

            // Create a session
            let session = manager
                .create_session("test-meet".to_string(), "Test Location".to_string(), 5)
                .await;

            // Get CSRF token
            let csrf_token = manager.get_csrf_token(&session.token).await.unwrap();

            // Verify correct CSRF token
            assert!(manager.verify_csrf_token(&session.token, &csrf_token).await);

            // Verify incorrect CSRF token
            assert!(
                !manager
                    .verify_csrf_token(&session.token, "invalid-token")
                    .await
            );
        })
        .await
        .expect("Test timed out");
    }
}
