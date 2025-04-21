// ============================
// openlifter-backend-lib/src/config.rs
// ============================
//! Configuration management.
use std::net::SocketAddr;
use std::path::PathBuf;
use serde::Deserialize;
use figment::{Figment, providers::{Env, Format, Json, Toml, Yaml}};
use anyhow::Result;

/// Application settings
#[derive(Debug, Deserialize)]
pub struct Settings {
    /// Server bind address
    pub bind_addr: SocketAddr,
    /// Data directory path
    pub data_dir: PathBuf,
    /// Log level
    pub log_level: String,
    /// Session TTL in seconds
    pub session_ttl_secs: u64,
    /// Password requirements
    pub password_requirements: PasswordRequirements,
}

/// Password complexity requirements
#[derive(Debug, Deserialize)]
pub struct PasswordRequirements {
    /// Minimum password length
    pub min_length: usize,
    /// Require uppercase letters
    pub require_uppercase: bool,
    /// Require lowercase letters
    pub require_lowercase: bool,
    /// Require digits
    pub require_digit: bool,
    /// Require special characters
    pub require_special: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:3000".parse().unwrap(),
            data_dir: PathBuf::from("data"),
            log_level: "info".to_string(),
            session_ttl_secs: 60 * 60 * 24 * 7, // 7 days
            password_requirements: PasswordRequirements::default(),
        }
    }
}

impl Default for PasswordRequirements {
    fn default() -> Self {
        Self {
            min_length: 10,
            require_uppercase: true,
            require_lowercase: true,
            require_digit: true,
            require_special: true,
        }
    }
}

/// Load settings from various sources
pub fn load_settings() -> Result<Settings> {
    // Try to load from config file first, then environment variables
    let settings = Figment::new()
        .merge(Toml::file("config.toml"))
        .merge(Yaml::file("config.yaml"))
        .merge(Json::file("config.json"))
        .merge(Env::prefixed("OPENLIFTER_"))
        .extract()?;
    
    Ok(settings)
} 