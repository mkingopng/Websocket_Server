// =========================
// tests/unit/error_tests.rs
// =========================
//! Unit tests for the error module
use axum::http::StatusCode;
use backend_lib::error::AppError;
use std::io::{Error as IoError, ErrorKind};

#[test]
fn test_app_error_display() {
    // Test error display formatting for different error types
    let auth_error = AppError::Auth("Invalid token".to_string());
    assert_eq!(
        auth_error.to_string(),
        "Authentication error: Invalid token"
    );

    let io_error = AppError::Io(IoError::new(ErrorKind::NotFound, "File not found"));
    assert!(io_error.to_string().contains("IO error"));

    let rate_limit_error = AppError::RateLimitExceeded;
    assert_eq!(rate_limit_error.to_string(), "Rate limit exceeded");
}

#[test]
fn test_app_error_status_codes() {
    assert_eq!(
        AppError::Auth("Invalid credentials".to_string()).status_code(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        AppError::Internal("test".to_string()).status_code(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(
        AppError::NotFound("test".to_string()).status_code(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        AppError::RateLimitExceeded.status_code(),
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(
        AppError::NeedsRecovery {
            meet_id: "test".to_string(),
            last_known_seq: 10
        }
        .status_code(),
        StatusCode::CONFLICT
    );
}

#[test]
fn test_app_error_error_codes() {
    assert_eq!(
        AppError::Auth("Invalid credentials".to_string()).error_code(),
        "AUTH_001"
    );
    assert_eq!(
        AppError::Internal("test".to_string()).error_code(),
        "INT_001"
    );
    assert_eq!(
        AppError::NotFound("test".to_string()).error_code(),
        "NF_001"
    );
    assert_eq!(AppError::RateLimitExceeded.error_code(), "RATE_001");
    assert_eq!(AppError::InvalidPassword.error_code(), "AUTH_002");
    assert_eq!(
        AppError::NeedsRecovery {
            meet_id: "test".to_string(),
            last_known_seq: 10
        }
        .error_code(),
        "RECOVERY_001"
    );
}

#[test]
fn test_app_error_sanitized_message() {
    // Test that sensitive information is removed in production messages
    let auth_error = AppError::Auth("username: admin, password: secret123".to_string());
    assert_eq!(auth_error.sanitized_message(), "Authentication failed");

    let internal_error =
        AppError::Internal("Database connection failed with password: dbpass123".to_string());
    assert_eq!(
        internal_error.sanitized_message(),
        "An internal server error occurred"
    );
}

#[test]
fn test_error_from_impls() {
    // Test conversions from other error types
    let io_err = IoError::new(ErrorKind::PermissionDenied, "Permission denied");
    let app_err: AppError = io_err.into();
    assert!(matches!(app_err, AppError::Io(_)));

    let json_err: serde_json::Error =
        serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
    let app_err: AppError = json_err.into();
    assert!(matches!(app_err, AppError::Json(_)));

    let string_err = "String error".to_string();
    let app_err: AppError = string_err.into();
    assert!(matches!(app_err, AppError::Internal(_)));

    let str_err = "Str error";
    let app_err: AppError = str_err.into();
    assert!(matches!(app_err, AppError::Internal(_)));
}
