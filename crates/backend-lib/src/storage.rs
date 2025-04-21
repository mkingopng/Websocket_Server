// ============================
// openlifter-backend-lib/src/storage.rs
// ============================
//! Storage abstraction with flat-file implementation.
use std::{fs, path::{Path, PathBuf}};
use tokio::{fs as tokio_fs, io::AsyncWriteExt};
use serde_json;
use async_trait::async_trait;
use crate::error::AppError;
use openlifter_common::{MeetInfo, EndpointPriority};

/// Trait for storage backends
#[async_trait]
pub trait Storage: Send + Sync {
    /// Append a JSON line to the updates log
    async fn append_update(
        &self,
        meet_id: &str,
        json_line: &str,
    ) -> Result<(), AppError>;
    
    /// Read all updates for a meet
    async fn read_updates(
        &self,
        meet_id: &str,
    ) -> Result<Vec<String>, AppError>;
    
    /// Archive a meet (move from current to finished)
    async fn archive_meet(&self, meet_id: &str) -> Result<(), AppError>;
    
    /// Store meet information
    async fn store_meet_info(
        &self,
        meet_id: &str,
        password_hash: &str,
        endpoints: &[EndpointPriority],
    ) -> Result<(), AppError>;
    
    /// Get meet information
    async fn get_meet_info(&self, meet_id: &str) -> Result<MeetInfo, AppError>;
    
    /// Store meet CSV data
    async fn store_meet_csv(
        &self,
        meet_id: &str,
        opl_csv: &str,
        return_email: &str,
    ) -> Result<(), AppError>;
}

/// Flat-file implementation of the Storage trait
#[derive(Clone)]
pub struct FlatFileStorage {
    root: PathBuf,
}

impl FlatFileStorage {
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
    async fn append_update(
        &self,
        meet_id: &str,
        json_line: &str,
    ) -> Result<(), AppError> {
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
    async fn read_updates(
        &self,
        meet_id: &str,
    ) -> Result<Vec<String>, AppError> {
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
            .filter(|line| !line.trim().is_empty())
            .map(|s| s.to_string())
            .collect();

        Ok(updates)
    }

    /// Archive a meet (move from current to finished)
    async fn archive_meet(&self, meet_id: &str) -> Result<(), AppError> {
        let src = self.root.join("current-meets").join(meet_id);
        let dst = self.root.join("finished-meets").join(meet_id);

        if src.exists() {
            tokio_fs::rename(src, dst).await?;
        }

        Ok(())
    }

    /// Store meet information
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

    /// Get meet information
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

    /// Store meet CSV data
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