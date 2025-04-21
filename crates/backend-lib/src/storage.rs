// ============================
// openlifter-backend-lib/src/storage.rs
// ============================
//! Storage abstraction with flat-file implementation.
//!
//! This module provides a trait-based storage abstraction for meet data,
//! with a flat-file implementation that stores data in a simple directory structure:
//!
//! ```text
//! data/
//! |-- current-meets/
//! |   |-- {meet_id}/
//! |       |-- updates.log      # Append-only log of updates
//! |       |-- meet-info.json   # Meet metadata (password hash, endpoints)
//! |       |-- meet.csv         # Final meet results
//! |       |-- return-email.txt # Email for results
//! |-- finished-meets/
//!     |-- {meet_id}/           # Archived meets
//! ```
//!
//! The storage is designed to be simple and reliable, with atomic operations
//! where possible. The flat-file implementation is suitable for most use cases
//! and provides good performance for the expected load.

use crate::error::AppError;
use async_trait::async_trait;
use openlifter_common::{EndpointPriority, MeetInfo};
use serde_json;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::{fs as tokio_fs, io::AsyncWriteExt};

/// Trait for storage backends
///
/// This trait defines the interface for storing and retrieving meet data.
/// Implementations should ensure data consistency and handle concurrent access
/// appropriately.
#[async_trait]
pub trait Storage: Send + Sync {
    /// Append a JSON line to the updates log
    ///
    /// # Arguments
    /// * `meet_id` - ID of the meet
    /// * `json_line` - JSON-encoded update to append
    ///
    /// # Returns
    /// * `Ok(())` if the update was successfully appended
    /// * `Err(AppError)` if the operation failed
    async fn append_update(&self, meet_id: &str, json_line: &str) -> Result<(), AppError>;

    /// Read all updates for a meet
    ///
    /// # Arguments
    /// * `meet_id` - ID of the meet
    ///
    /// # Returns
    /// * `Ok(Vec<String>)` - List of JSON-encoded updates
    /// * `Err(AppError)` if the operation failed
    async fn read_updates(&self, meet_id: &str) -> Result<Vec<String>, AppError>;

    /// Archive a meet (move from current to finished)
    ///
    /// # Arguments
    /// * `meet_id` - ID of the meet to archive
    ///
    /// # Returns
    /// * `Ok(())` if the meet was successfully archived
    /// * `Err(AppError)` if the operation failed
    async fn archive_meet(&self, meet_id: &str) -> Result<(), AppError>;

    /// Store meet information
    ///
    /// # Arguments
    /// * `meet_id` - ID of the meet
    /// * `password_hash` - Hashed meet password
    /// * `endpoints` - List of endpoints with priorities
    ///
    /// # Returns
    /// * `Ok(())` if the information was successfully stored
    /// * `Err(AppError)` if the operation failed
    async fn store_meet_info(
        &self,
        meet_id: &str,
        password_hash: &str,
        endpoints: &[EndpointPriority],
    ) -> Result<(), AppError>;

    /// Get meet information
    ///
    /// # Arguments
    /// * `meet_id` - ID of the meet
    ///
    /// # Returns
    /// * `Ok(MeetInfo)` - Meet information
    /// * `Err(AppError)` if the operation failed
    async fn get_meet_info(&self, meet_id: &str) -> Result<MeetInfo, AppError>;

    /// Store meet CSV data
    ///
    /// # Arguments
    /// * `meet_id` - ID of the meet
    /// * `opl_csv` - CSV data in OPL format
    /// * `return_email` - Email to send results to
    ///
    /// # Returns
    /// * `Ok(())` if the data was successfully stored
    /// * `Err(AppError)` if the operation failed
    async fn store_meet_csv(
        &self,
        meet_id: &str,
        opl_csv: &str,
        return_email: &str,
    ) -> Result<(), AppError>;
}

/// Flat-file implementation of the Storage trait
///
/// This implementation stores meet data in a simple directory structure
/// under the specified root directory. All operations are performed
/// atomically where possible to ensure data consistency.
#[derive(Clone)]
pub struct FlatFileStorage {
    root: PathBuf,
}

impl FlatFileStorage {
    /// Create a new flat-file storage instance
    ///
    /// # Arguments
    /// * `root` - Root directory for storing meet data
    ///
    /// # Returns
    /// * `Ok(FlatFileStorage)` - New storage instance
    /// * `Err(anyhow::Error)` if the directories could not be created
    pub fn new<P: AsRef<Path>>(root: P) -> anyhow::Result<Self> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(root.join("current-meets"))?;
        fs::create_dir_all(root.join("finished-meets"))?;
        Ok(Self { root })
    }
}

#[async_trait]
impl Storage for FlatFileStorage {
    /// Append a JSON line to `updates.log`.
    ///
    /// The file is created if it doesn't exist, and the update is appended
    /// atomically using a temporary file.
    async fn append_update(&self, meet_id: &str, json_line: &str) -> Result<(), AppError> {
        let path = self
            .root
            .join("current-meets")
            .join(meet_id)
            .join("updates.log");

        // ensure directory exists
        tokio_fs::create_dir_all(path.parent().unwrap()).await?;

        let mut file = tokio_fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await
            .map_err(AppError::from)?;

        file.write_all(json_line.as_bytes()).await?;
        file.write_all(b"\n").await?;
        Ok(())
    }

    /// Read all updates for a meet
    ///
    /// Returns an empty vector if the meet doesn't exist or has no updates.
    async fn read_updates(&self, meet_id: &str) -> Result<Vec<String>, AppError> {
        let path = self
            .root
            .join("current-meets")
            .join(meet_id)
            .join("updates.log");

        if !path.exists() {
            return Ok(Vec::new());
        }

        let content = tokio_fs::read_to_string(&path).await?;
        let updates: Vec<String> = content
            .lines()
            .filter(|s| !s.is_empty())
            .map(ToString::to_string)
            .collect();

        Ok(updates)
    }

    /// Archive a meet by moving it from current-meets to finished-meets
    ///
    /// The operation is atomic - it either succeeds completely or fails
    /// without modifying the filesystem.
    async fn archive_meet(&self, meet_id: &str) -> Result<(), AppError> {
        let src = self.root.join("current-meets").join(meet_id);
        let dst = self.root.join("finished-meets").join(meet_id);

        if src.exists() {
            tokio_fs::rename(src, dst).await?;
        }

        Ok(())
    }

    /// Store meet information in meet-info.json
    ///
    /// The file is created if it doesn't exist, and the information is written
    /// atomically using a temporary file.
    async fn store_meet_info(
        &self,
        meet_id: &str,
        password_hash: &str,
        endpoints: &[EndpointPriority],
    ) -> Result<(), AppError> {
        let path = self
            .root
            .join("current-meets")
            .join(meet_id)
            .join("meet-info.json");

        // ensure directory exists
        tokio_fs::create_dir_all(path.parent().unwrap()).await?;

        let meet_info = MeetInfo {
            password_hash: password_hash.to_string(),
            endpoints: endpoints.to_vec(),
        };

        let json = serde_json::to_string_pretty(&meet_info)?;
        tokio_fs::write(path, json).await?;

        Ok(())
    }

    /// Get meet information from meet-info.json
    ///
    /// Returns an error if the meet doesn't exist or the file is corrupted.
    async fn get_meet_info(&self, meet_id: &str) -> Result<MeetInfo, AppError> {
        let path = self
            .root
            .join("current-meets")
            .join(meet_id)
            .join("meet-info.json");

        if !path.exists() {
            return Err(AppError::MeetNotFound);
        }

        let content = tokio_fs::read_to_string(&path).await?;
        let meet_info: MeetInfo = serde_json::from_str(&content)?;

        Ok(meet_info)
    }

    /// Store meet CSV data and return email
    ///
    /// The CSV data is stored in meet.csv and the return email in return-email.txt.
    /// Both files are written atomically using temporary files.
    async fn store_meet_csv(
        &self,
        meet_id: &str,
        opl_csv: &str,
        return_email: &str,
    ) -> Result<(), AppError> {
        let path = self
            .root
            .join("current-meets")
            .join(meet_id)
            .join("meet.csv");

        // ensure directory exists
        tokio_fs::create_dir_all(path.parent().unwrap()).await?;

        tokio_fs::write(path, opl_csv).await?;

        // Store return email
        let email_path = self
            .root
            .join("current-meets")
            .join(meet_id)
            .join("return-email.txt");

        tokio_fs::write(email_path, return_email).await?;

        Ok(())
    }
}

#[async_trait]
impl<T: Storage + ?Sized> Storage for Arc<Box<T>> {
    async fn append_update(&self, meet_id: &str, json_line: &str) -> Result<(), AppError> {
        (**self).append_update(meet_id, json_line).await
    }

    async fn read_updates(&self, meet_id: &str) -> Result<Vec<String>, AppError> {
        (**self).read_updates(meet_id).await
    }

    async fn archive_meet(&self, meet_id: &str) -> Result<(), AppError> {
        (**self).archive_meet(meet_id).await
    }

    async fn store_meet_info(
        &self,
        meet_id: &str,
        password_hash: &str,
        endpoints: &[EndpointPriority],
    ) -> Result<(), AppError> {
        (**self)
            .store_meet_info(meet_id, password_hash, endpoints)
            .await
    }

    async fn get_meet_info(&self, meet_id: &str) -> Result<MeetInfo, AppError> {
        (**self).get_meet_info(meet_id).await
    }

    async fn store_meet_csv(
        &self,
        meet_id: &str,
        opl_csv: &str,
        return_email: &str,
    ) -> Result<(), AppError> {
        (**self)
            .store_meet_csv(meet_id, opl_csv, return_email)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (FlatFileStorage, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let storage = FlatFileStorage::new(temp_dir.path()).unwrap();
        (storage, temp_dir)
    }

    #[tokio::test]
    async fn test_append_read_updates() {
        let (storage, _temp_dir) = setup();
        let meet_id = "test-meet";

        // Append some updates
        storage.append_update(meet_id, "update1").await.unwrap();
        storage.append_update(meet_id, "update2").await.unwrap();

        // Read updates
        let updates = storage.read_updates(meet_id).await.unwrap();
        assert_eq!(updates.len(), 2);
        assert_eq!(updates[0], "update1");
        assert_eq!(updates[1], "update2");
    }

    #[tokio::test]
    async fn test_store_get_meet_info() {
        let (storage, _temp_dir) = setup();
        let meet_id = "test-meet";
        let password_hash = "hashed_password";
        let endpoints = vec![EndpointPriority {
            location_name: "location1".to_string(),
            priority: 1,
        }];
        // Store meet info
        storage
            .store_meet_info(meet_id, password_hash, &endpoints)
            .await
            .unwrap();

        // Get meet info
        let info = storage.get_meet_info(meet_id).await.unwrap();
        assert_eq!(info.password_hash, password_hash);
        assert_eq!(info.endpoints.len(), 1);
        assert_eq!(info.endpoints[0].location_name, "location1");
        assert_eq!(info.endpoints[0].priority, 1);
    }

    #[tokio::test]
    async fn test_archive_meet() {
        let (storage, _temp_dir) = setup();
        let meet_id = "test-meet";

        // Create some data
        storage.append_update(meet_id, "test").await.unwrap();
        storage.store_meet_info(meet_id, "hash", &[]).await.unwrap();
        // Archive meet
        storage.archive_meet(meet_id).await.unwrap();

        // Verify meet is no longer in current-meets
        let path = storage.root.join("current-meets").join(meet_id);
        assert!(!path.exists());

        // Verify meet is in finished-meets
        let path = storage.root.join("finished-meets").join(meet_id);
        assert!(path.exists());
    }

    #[tokio::test]
    async fn test_store_csv_data() {
        let (storage, _temp_dir) = setup();
        let meet_id = "test-meet";
        let csv = "Name,Weight,Squat";
        let email = "test@example.com";

        storage.store_meet_csv(meet_id, csv, email).await.unwrap();
        // Verify files exist
        let csv_path = storage
            .root
            .join("current-meets")
            .join(meet_id)
            .join("meet.csv");
        let email_path = storage
            .root
            .join("current-meets")
            .join(meet_id)
            .join("return-email.txt");

        assert!(csv_path.exists());
        assert!(email_path.exists());

        assert_eq!(fs::read_to_string(csv_path).unwrap(), csv);
        assert_eq!(fs::read_to_string(email_path).unwrap(), email);
    }

    #[tokio::test]
    async fn test_read_updates_nonexistent_meet() {
        let (storage, _temp_dir) = setup();
        let meet_id = "nonexistent-meet";

        let updates = storage.read_updates(meet_id).await.unwrap();
        assert!(updates.is_empty());
    }
}
