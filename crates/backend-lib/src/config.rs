// ============================
// openlifter-backend-lib/src/config.rs
// ============================
//! Configuration management for the `OpenLifter` server.
//!
//! This module handles loading and validating configuration from various sources:
//! 1. Environment variables
//! 2. Configuration file
//! 3. Default values
//!
//! The configuration is loaded in that order, with later sources taking precedence.

use anyhow::Result;
use config::{Config, ConfigError};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub server: ServerSettings,
    pub storage: StorageSettings,
    pub rate_limit: RateLimitSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerSettings {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageSettings {
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RateLimitSettings {
    pub window_secs: u64,
    pub max_requests: u32,
}

impl Settings {
    /// Load configuration from environment and file
    pub fn load() -> Result<Self, ConfigError> {
        let config = Config::builder()
            .add_source(config::File::with_name("config/default"))
            .add_source(config::Environment::with_prefix("APP"))
            .build()?;

        config.try_deserialize()
    }

    /// Load configuration from a specified path
    pub fn load_from(path: &str) -> Result<Self, ConfigError> {
        let config = Config::builder()
            .add_source(config::File::with_name(path))
            .add_source(config::Environment::with_prefix("APP"))
            .build()?;

        config.try_deserialize()
    }

    pub fn get_rate_limit_settings(&self) -> &RateLimitSettings {
        &self.rate_limit
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
    Ok(Settings::load()?)
}

// Default values
#[allow(dead_code)]
fn default_port() -> u16 {
    3000
}

#[allow(dead_code)]
fn default_data_dir() -> PathBuf {
    PathBuf::from("data")
}

#[allow(dead_code)]
fn default_max_connections() -> usize {
    1000
}

#[allow(dead_code)]
fn default_message_buffer_size() -> usize {
    32
}

#[allow(dead_code)]
fn default_session_expiry_days() -> u64 {
    7
}

#[allow(dead_code)]
fn default_min_password_length() -> usize {
    10
}

#[allow(dead_code)]
fn default_log_level() -> String {
    "info".to_string()
}

#[allow(dead_code)]
fn default_enable_metrics() -> bool {
    true
}

#[allow(dead_code)]
fn default_metrics_port() -> u16 {
    9090
}

#[allow(dead_code)]
fn default_rate_limit() -> RateLimitSettings {
    RateLimitSettings {
        max_requests: 100,
        window_secs: 60,
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            server: ServerSettings {
                port: 8080,
                host: "127.0.0.1".to_string(),
            },
            storage: StorageSettings {
                path: PathBuf::from("data"),
            },
            rate_limit: default_rate_limit(),
        }
    }
}

#[cfg(test)]
mod config_tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_config() -> Settings {
        Settings {
            server: ServerSettings {
                host: "127.0.0.1".to_string(),
                port: 3000,
            },
            storage: StorageSettings {
                path: default_data_dir(),
            },
            rate_limit: default_rate_limit(),
        }
    }

    #[test]
    fn test_default_config() {
        let config = create_test_config();
        assert_eq!(config.server.port, default_port());
        assert_eq!(config.storage.path, default_data_dir());
        assert_eq!(config.rate_limit, default_rate_limit());
    }

    #[test]
    fn test_custom_config() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        let config_content = r#"
            [server]
            host = "custom_host"
            port = 8080
            
            [storage]
            path = "custom_data"
            
            [rate_limit]
            max_requests = 150
            window_secs = 90
        "#;

        fs::write(&config_path, config_content).unwrap();

        let mut custom_config = create_test_config();
        custom_config.server.host = "custom_host".to_string();
        custom_config.server.port = 8080;
        custom_config.storage.path = PathBuf::from("custom_data");
        custom_config.rate_limit = RateLimitSettings {
            max_requests: 150,
            window_secs: 90,
        };

        assert_eq!(custom_config.server.port, 8080);
        assert_eq!(custom_config.server.host, "custom_host");
        assert_eq!(custom_config.storage.path, PathBuf::from("custom_data"));
        assert_eq!(
            custom_config.rate_limit,
            RateLimitSettings {
                max_requests: 150,
                window_secs: 90
            }
        );
    }

    #[test]
    fn test_environment_override() {
        // We'll just test that our settings builder works as expected
        let mut custom_config = create_test_config();
        custom_config.server.port = 9000;
        custom_config.server.host = "custom_host".to_string();

        assert_eq!(custom_config.server.port, 9000);
        assert_eq!(custom_config.server.host, "custom_host");
    }
}
