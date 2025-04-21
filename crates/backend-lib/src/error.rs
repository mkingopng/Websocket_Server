// ============================
// openlifter-backend-lib/src/error.rs
// ============================
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

    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

impl AppError {
    /// Get the HTTP status code for this error
    pub fn status_code(&self) -> StatusCode {
        match self {
            AppError::Auth(_) | AppError::InvalidPassword => StatusCode::UNAUTHORIZED,
            AppError::NotFound(_) | AppError::MeetNotFound => StatusCode::NOT_FOUND,
            AppError::InvalidMeetId | AppError::InvalidInput(_) => StatusCode::BAD_REQUEST,
            AppError::RateLimitExceeded => StatusCode::TOO_MANY_REQUESTS,
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
            AppError::InvalidInput(_) => "VAL_001",
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let error_code = self.error_code();
        let message = self.to_string();

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
