// ============================
// openlifter-backend-lib/src/config.rs
// ============================
//! Configuration management for the OpenLifter server.
//! 
//! This module handles loading and validating configuration from various sources:
//! 1. Environment variables
//! 2. Configuration file
//! 3. Default values
//! 
//! The configuration is loaded in that order, with later sources taking precedence.

use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use config::{Config as ConfigSource, Environment, File};
use anyhow::Result;

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Server port to listen on
    #[serde(default = "default_port")]
    pub port: u16,

    /// Data directory for storing meet data
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,

    /// Maximum number of concurrent WebSocket connections
    #[serde(default = "default_max_connections")]
    pub max_connections: usize,

    /// WebSocket message buffer size
    #[serde(default = "default_message_buffer_size")]
    pub message_buffer_size: usize,

    /// Session token expiration in days
    #[serde(default = "default_session_expiry_days")]
    pub session_expiry_days: u64,

    /// Minimum password length
    #[serde(default = "default_min_password_length")]
    pub min_password_length: usize,

    /// Log level (trace, debug, info, warn, error)
    #[serde(default = "default_log_level")]
    pub log_level: String,

    /// Enable metrics collection
    #[serde(default = "default_enable_metrics")]
    pub enable_metrics: bool,

    /// Metrics port (0 to disable)
    #[serde(default = "default_metrics_port")]
    pub metrics_port: u16,

    /// Rate limit settings
    #[serde(default = "default_rate_limit")]
    pub rate_limit: RateLimit,
}

/// Rate limit configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RateLimit {
    /// Maximum number of requests per window
    pub max_requests: u32,
    /// Window size in seconds
    pub window_secs: u64,
}

impl Settings {
    /// Load configuration from environment and file
    pub fn load() -> Result<Self> {
        let config = ConfigSource::builder()
            // Start with default values
            .add_source(File::with_name("config/default"))
            // Add environment-specific config
            .add_source(
                File::with_name(&format!(
                    "config/{}",
                    std::env::var("RUST_ENV").unwrap_or_else(|_| "development".to_string())
                ))
                .required(false),
            )
            // Add local config (gitignored)
            .add_source(File::with_name("config/local").required(false))
            // Add environment variables
            .add_source(
                Environment::with_prefix("OPENLIFTER")
                    .separator("__")
                    .try_parsing(true),
            )
            .build()?;

        Ok(config.try_deserialize()?)
    }
}

/// Settings manager for the application
pub struct SettingsManager {
    settings: Settings,
}

impl SettingsManager {
    /// Create a new settings manager
    pub fn new(settings: Settings) -> Result<Self> {
        Ok(Self { settings })
    }

    /// Get the current settings
    pub fn get(&self) -> &Settings {
        &self.settings
    }
}

/// Load settings from environment and file
pub fn load_settings() -> Result<Settings> {
    Settings::load()
}

// Default values
fn default_port() -> u16 {
    3000
}

fn default_data_dir() -> PathBuf {
    PathBuf::from("data")
}

fn default_max_connections() -> usize {
    1000
}

fn default_message_buffer_size() -> usize {
    32
}

fn default_session_expiry_days() -> u64 {
    7
}

fn default_min_password_length() -> usize {
    10
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_enable_metrics() -> bool {
    true
}

fn default_metrics_port() -> u16 {
    9090
}

fn default_rate_limit() -> RateLimit {
    RateLimit {
        max_requests: 100,
        window_secs: 60,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[test]
    fn test_default_config() {
        let config = Settings::load().unwrap();
        assert_eq!(config.port, default_port());
        assert_eq!(config.data_dir, default_data_dir());
        assert_eq!(config.max_connections, default_max_connections());
        assert_eq!(config.message_buffer_size, default_message_buffer_size());
        assert_eq!(config.session_expiry_days, default_session_expiry_days());
        assert_eq!(config.min_password_length, default_min_password_length());
        assert_eq!(config.log_level, default_log_level());
        assert_eq!(config.enable_metrics, default_enable_metrics());
        assert_eq!(config.metrics_port, default_metrics_port());
        assert_eq!(config.rate_limit, default_rate_limit());
    }

    #[test]
    fn test_custom_config() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        
        let config_content = r#"
            port = 8080
            data_dir = "custom_data"
            max_connections = 2000
            message_buffer_size = 64
            session_expiry_days = 14
            min_password_length = 12
            log_level = "debug"
            enable_metrics = false
            metrics_port = 0
            rate_limit = { max_requests = 150, window_secs = 90 }
        "#;
        
        fs::write(&config_path, config_content).unwrap();
        
        std::env::set_var("OPENLIFTER__CONFIG_PATH", config_path.to_str().unwrap());
        
        let config = Settings::load().unwrap();
        assert_eq!(config.port, 8080);
        assert_eq!(config.data_dir, PathBuf::from("custom_data"));
        assert_eq!(config.max_connections, 2000);
        assert_eq!(config.message_buffer_size, 64);
        assert_eq!(config.session_expiry_days, 14);
        assert_eq!(config.min_password_length, 12);
        assert_eq!(config.log_level, "debug");
        assert_eq!(config.enable_metrics, false);
        assert_eq!(config.metrics_port, 0);
        assert_eq!(config.rate_limit, RateLimit { max_requests: 150, window_secs: 90 });
    }

    #[test]
    fn test_environment_override() {
        std::env::set_var("OPENLIFTER__PORT", "9000");
        std::env::set_var("OPENLIFTER__LOG_LEVEL", "trace");
        
        let config = Settings::load().unwrap();
        assert_eq!(config.port, 9000);
        assert_eq!(config.log_level, "trace");
    }
} 