// 
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn test_settings_validation() {
        // Test valid settings
        let settings = Settings {
            bind_addr: "127.0.0.1:3000".parse().unwrap(),
            data_dir: PathBuf::from("data"),
            log_level: "info".to_string(),
            session_ttl_secs: 3600,
            password_requirements: PasswordRequirements::default(),
            rate_limit: RateLimitSettings::default(),
        };
        assert!(settings.validate().is_ok());

        // Test invalid log level
        let mut invalid_settings = settings.clone();
        invalid_settings.log_level = "invalid".to_string();
        assert!(invalid_settings.validate().is_err());

        // Test invalid session TTL
        let mut invalid_settings = settings.clone();
        invalid_settings.session_ttl_secs = 0;
        assert!(invalid_settings.validate().is_err());

        // Test invalid password requirements
        let mut invalid_settings = settings.clone();
        invalid_settings.password_requirements.min_length = 4;
        assert!(invalid_settings.validate().is_err());

        // Test invalid rate limit settings
        let mut invalid_settings = settings.clone();
        invalid_settings.rate_limit.max_requests = 0;
        assert!(invalid_settings.validate().is_err());
    }

    #[test]
    fn test_settings_builder() {
        let settings = Settings::builder()
            .bind_addr("127.0.0.1:3000".parse().unwrap())
            .data_dir(PathBuf::from("data"))
            .log_level("info".to_string())
            .session_ttl(3600)
            .build()
            .unwrap();

        assert_eq!(settings.bind_addr.to_string(), "127.0.0.1:3000");
        assert_eq!(settings.data_dir, PathBuf::from("data"));
        assert_eq!(settings.log_level, "info");
        assert_eq!(settings.session_ttl_secs, 3600);
    }

    #[tokio::test]
    async fn test_settings_manager() {
        let settings = Settings::default();
        let manager = SettingsManager::new(settings.clone()).unwrap();

        // Test get
        let current = manager.get().await;
        assert_eq!(current.bind_addr, settings.bind_addr);
        assert_eq!(current.data_dir, settings.data_dir);

        // Test update
        let mut new_settings = settings.clone();
        new_settings.log_level = "debug".to_string();
        manager.update(new_settings.clone()).await.unwrap();

        let updated = manager.get().await;
        assert_eq!(updated.log_level, "debug");
    }

    #[test]
    fn test_load_settings() {
        // Create a temporary directory for config files
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        
        // Write a test config file
        std::fs::write(
            &config_path,
            r#"
            bind_addr = "127.0.0.1:3000"
            data_dir = "test_data"
            log_level = "debug"
            session_ttl_secs = 3600
            "#,
        )
        .unwrap();

        // Set environment variable
        std::env::set_var("OPENLIFTER_LOG_LEVEL", "info");

        // Load settings
        let settings = load_settings().unwrap();
        assert_eq!(settings.bind_addr.to_string(), "127.0.0.1:3000");
        assert_eq!(settings.data_dir, PathBuf::from("test_data"));
        assert_eq!(settings.log_level, "info"); // Environment variable takes precedence
        assert_eq!(settings.session_ttl_secs, 3600);
    }
} 