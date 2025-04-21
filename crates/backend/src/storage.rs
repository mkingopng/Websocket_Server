// ============================
// openlifter-backend/src/storage.rs
// ============================
//! Flat‑file persistence with an async façade.
use std::{fs, path::{Path, PathBuf}};
use tokio::{fs as tokio_fs, io::AsyncWriteExt};
use serde_json;
use crate::error::AppError;
use openlifter_common::MeetInfo;

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

    /// Append a JSON line to `updates.log`.
    pub async fn append_update(
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

    /// Read every line from `updates.log` (for resync/bootstrap).
    pub async fn read_updates(
        &self,
        meet_id: &str,
    ) -> Result<Vec<String>, AppError> {
        let path = self
            .root
            .join("current-meets")
            .join(meet_id)
            .join("updates.log");

        if !path.exists() {
            return Ok(vec![]);
        }

        let content = tokio_fs::read_to_string(path).await?;
        Ok(content.lines().map(|s| s.to_owned()).collect())
    }

    /// Move a finished meet to the archive dir.
    pub async fn archive_meet(&self, meet_id: &str) -> Result<(), AppError> {
        let cur = self.root.join("current-meets").join(meet_id);
        let dst = self.root.join("finished-meets").join(meet_id);
        tokio_fs::rename(cur, dst).await?;
        Ok(())
    }
    
    /// Store meet information (password hash and endpoints)
    pub async fn store_meet_info(
        &self,
        meet_id: &str,
        password_hash: &str,
        endpoints: &[openlifter_common::EndpointPriority],
    ) -> Result<(), AppError> {
        let path = self
            .root
            .join("current-meets")
            .join(meet_id)
            .join("auth.json");
            
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
    pub async fn get_meet_info(&self, meet_id: &str) -> Result<MeetInfo, AppError> {
        let path = self
            .root
            .join("current-meets")
            .join(meet_id)
            .join("auth.json");
            
        if !path.exists() {
            return Err(AppError::Internal("Meet info not found".into()));
        }
        
        let json = tokio_fs::read_to_string(path).await?;
        let meet_info: MeetInfo = serde_json::from_str(&json)?;
        
        Ok(meet_info)
    }
    
    /// Store CSV data for a meet
    pub async fn store_meet_csv(
        &self,
        meet_id: &str,
        opl_csv: &str,
        return_email: &str,
    ) -> Result<(), AppError> {
        let path = self
            .root
            .join("finished-meets")
            .join(meet_id)
            .join("opl.csv");
            
        // ensure directory exists
        tokio_fs::create_dir_all(path.parent().unwrap()).await?;
        
        // Write CSV data
        tokio_fs::write(path, opl_csv).await?;
        
        // Write email info
        let email_path = self
            .root
            .join("finished-meets")
            .join(meet_id)
            .join("email.txt");
            
        tokio_fs::write(email_path, return_email).await?;
        
        Ok(())
    }
}
