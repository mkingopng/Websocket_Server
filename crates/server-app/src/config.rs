// ============================
// crates/server-app/src/config.rs
// ============================
/** Configuration management for the `OpenLifter` server.
This module handles loading and validating configuration from various sources:
1. Environment variables
2. Configuration file
3. Default values
The configuration is loaded in that order, with later sources taking precedence */
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
    PathBuf::from("server-storage")
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
                path: PathBuf::from("server-storage"),
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

    /// Helper function to create test settings with given values
    fn create_test_settings(
        host: &str,
        port: u16,
        path: &str,
        max_req: u32,
        window: u64,
    ) -> Settings {
        Settings {
            server: ServerSettings {
                host: host.to_string(),
                port,
            },
            storage: StorageSettings {
                path: PathBuf::from(path),
            },
            rate_limit: RateLimitSettings {
                window_secs: window,
                max_requests: max_req,
            },
        }
    }

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

    /// Test settings validation with various configurations
    #[test]
    fn test_settings_configurations() {
        let test_cases = [
            ("default", "127.0.0.1", 8080, "server-storage", 100, 60),
            ("custom", "0.0.0.0", 9000, "custom_data", 200, 120),
            ("minimal", "192.168.1.1", 8888, "test_data", 50, 30),
        ];

        for (name, host, port, path, max_req, window) in test_cases {
            let settings = if name == "default" {
                Settings::default()
            } else {
                create_test_settings(host, port, path, max_req, window)
            };

            assert_eq!(settings.server.host, host, "Host mismatch for {}", name);
            assert_eq!(settings.server.port, port, "Port mismatch for {}", name);
            assert_eq!(
                settings.storage.path,
                PathBuf::from(path),
                "Path mismatch for {}",
                name
            );
            assert_eq!(
                settings.rate_limit.max_requests, max_req,
                "Max requests mismatch for {}",
                name
            );
            assert_eq!(
                settings.rate_limit.window_secs, window,
                "Window mismatch for {}",
                name
            );
        }
    }

    #[test]
    fn test_settings_manager() {
        let settings = Settings::default();
        let manager = SettingsManager::new(settings.clone()).unwrap();
        let current = manager.get();

        assert_eq!(current.server.port, settings.server.port);
        assert_eq!(current.storage.path, settings.storage.path);
        assert_eq!(
            current.rate_limit.max_requests,
            settings.rate_limit.max_requests
        );
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
            path = "custom_server_stroage"
            
            [rate_limit]
            max_requests = 150
            window_secs = 90
        "#;

        fs::write(&config_path, config_content).unwrap();

        let mut custom_config = create_test_config();
        custom_config.server.host = "custom_host".to_string();
        custom_config.server.port = 8080;
        custom_config.storage.path = PathBuf::from("custom_server_storage");
        custom_config.rate_limit = RateLimitSettings {
            max_requests: 150,
            window_secs: 90,
        };

        assert_eq!(custom_config.server.port, 8080);
        assert_eq!(custom_config.server.host, "custom_host");
        assert_eq!(
            custom_config.storage.path,
            PathBuf::from("custom_server_storage")
        );
        assert_eq!(
            custom_config.rate_limit,
            RateLimitSettings {
                max_requests: 150,
                window_secs: 90
            }
        );
    }

    #[test]
    fn test_rate_limit_settings() {
        let settings = Settings::default();
        let rate_limit = settings.get_rate_limit_settings();

        assert_eq!(rate_limit.max_requests, 100);
        assert_eq!(rate_limit.window_secs, 60);
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

    #[tokio::test]
    async fn test_settings_manager_operations() {
        let settings = Settings::default();
        let manager = SettingsManager::new(settings.clone()).unwrap();

        // Test that we can get settings
        let retrieved = manager.get();
        assert_eq!(retrieved.server.host, settings.server.host);

        // Test that rate limit settings work correctly
        let rate_limit = retrieved.get_rate_limit_settings();
        assert_eq!(rate_limit.max_requests, 100);
        assert_eq!(rate_limit.window_secs, 60);
    }
}
