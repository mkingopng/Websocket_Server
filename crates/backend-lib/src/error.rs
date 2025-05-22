// crates/backend-lib/src/error.rs

//! Central error type + Axum integration.
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

/// Application error types with error codes and context
#[derive(Error, Debug)]
pub enum AppError {
    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Invalid password")]
    InvalidPassword,

    #[error("Meet not found")]
    MeetNotFound,

    #[error("Invalid meet ID")]
    InvalidMeetId,

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Authentication rate limit exceeded")]
    AuthRateLimited,

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("State inconsistency detected for meet {meet_id}, recovery needed (last_known_seq: {last_known_seq})")]
    NeedsRecovery {
        meet_id: String,
        last_known_seq: u64,
    },
}

impl AppError {
    /// Get the HTTP status code for this error
    pub fn status_code(&self) -> StatusCode {
        match self {
            AppError::Auth(_) | AppError::InvalidPassword => StatusCode::UNAUTHORIZED,
            AppError::NotFound(_) | AppError::MeetNotFound => StatusCode::NOT_FOUND,
            AppError::InvalidMeetId | AppError::InvalidInput(_) => StatusCode::BAD_REQUEST,
            AppError::RateLimitExceeded | AppError::AuthRateLimited => {
                StatusCode::TOO_MANY_REQUESTS
            },
            AppError::NeedsRecovery { .. } => StatusCode::CONFLICT,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Get the error code for this error
    pub fn error_code(&self) -> &'static str {
        match self {
            AppError::Auth(_) => "AUTH_001",
            AppError::Internal(_) => "INT_001",
            AppError::NotFound(_) => "NF_001",
            AppError::Io(_) => "IO_001",
            AppError::Json(_) => "JSON_001",
            AppError::InvalidPassword => "AUTH_002",
            AppError::MeetNotFound => "MEET_001",
            AppError::InvalidMeetId => "MEET_002",
            AppError::RateLimitExceeded => "RATE_001",
            AppError::AuthRateLimited => "AUTH_003",
            AppError::InvalidInput(_) => "VAL_001",
            AppError::NeedsRecovery { .. } => "RECOVERY_001",
        }
    }

    /// Get a sanitized message suitable for production use
    pub fn sanitized_message(&self) -> String {
        match self {
            AppError::Auth(_) => "Authentication failed".to_string(),
            AppError::InvalidPassword => "Authentication failed".to_string(),
            AppError::AuthRateLimited => {
                "Too many authentication attempts, please try again later".to_string()
            },
            AppError::Internal(_) => "An internal server error occurred".to_string(),
            AppError::Json(_) => "Invalid request format".to_string(),
            AppError::Io(_) => "Internal server error".to_string(),
            AppError::NotFound(_) => "Resource not found".to_string(),
            AppError::MeetNotFound => "Resource not found".to_string(),
            AppError::InvalidMeetId => "Invalid resource identifier".to_string(),
            AppError::RateLimitExceeded => {
                "Rate limit exceeded, please try again later".to_string()
            },
            AppError::InvalidInput(_) => "Invalid input provided".to_string(),
            AppError::NeedsRecovery { .. } => "Data synchronization required".to_string(),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let error_code = self.error_code();

        // Use detailed messages in development, sanitized in production
        let message = if cfg!(debug_assertions) {
            self.to_string()
        } else {
            self.sanitized_message()
        };

        // Create a JSON response with error details
        let body = serde_json::json!({
            "error": {
                "code": error_code,
                "message": message,
            }
        });

        (status, axum::Json(body)).into_response()
    }
}

impl<T> From<tokio::sync::mpsc::error::SendError<T>> for AppError {
    fn from(_: tokio::sync::mpsc::error::SendError<T>) -> Self {
        AppError::Internal("Failed to send message".to_string())
    }
}

impl From<String> for AppError {
    fn from(msg: String) -> Self {
        AppError::Internal(msg)
    }
}

impl From<&str> for AppError {
    fn from(msg: &str) -> Self {
        AppError::Internal(msg.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
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

        // Create a JSON error using from_str which will fail parsing and create a valid JsonError
        let json_err: serde_json::Error =
            serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        assert_eq!(
            AppError::Json(json_err).status_code(),
            StatusCode::INTERNAL_SERVER_ERROR
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

        // Create a JSON error using from_str which will fail parsing and create a valid JsonError
        let json_err: serde_json::Error =
            serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        assert_eq!(AppError::Json(json_err).error_code(), "JSON_001");
    }

    #[test]
    fn test_app_error_into_response() {
        // Test conversion to HTTP response
        let error = AppError::NotFound("Resource not found".to_string());
        let response = error.into_response();

        // Verify status code
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        // Extract and verify response body if needed
        // This is a simplistic test; in a real test we'd parse the body and check JSON content
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

    #[tokio::test]
    async fn test_error_serialization() {
        // Create an error and convert it to Response
        let json_err: serde_json::Error =
            serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let app_error = AppError::Json(json_err);
        let response = app_error.into_response();

        // Verify response
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        // Check headers - content type should be application/json
        let response_headers = response.headers();
        assert!(response_headers
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap()
            .contains("application/json"));

        // For a real test, we would extract and check the response body here
    }
}
