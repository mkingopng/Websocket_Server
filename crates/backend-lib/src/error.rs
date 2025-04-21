// ============================
// openlifter-backend-lib/src/error.rs
// ============================
//! Central error type + Axum integration.
use axum::{
    response::{IntoResponse, Response},
};
use axum::http::StatusCode;
use std::fmt;

#[derive(Debug)]
pub enum AppError {
    Auth(String),
    Internal(String),
    NotFound(String),
    Io(std::io::Error),
    Json(serde_json::Error),
    InvalidPassword,
    MeetNotFound,
    InvalidMeetId,
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Auth(msg) => write!(f, "Authentication error: {}", msg),
            AppError::Internal(msg) => write!(f, "Internal error: {}", msg),
            AppError::NotFound(msg) => write!(f, "Not found: {}", msg),
            AppError::Io(err) => write!(f, "IO error: {}", err),
            AppError::Json(err) => write!(f, "JSON error: {}", err),
            AppError::InvalidPassword => write!(f, "Invalid password"),
            AppError::MeetNotFound => write!(f, "Meet not found"),
            AppError::InvalidMeetId => write!(f, "Invalid meet ID"),
        }
    }
}

impl std::error::Error for AppError {}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::Io(err)
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::Json(err)
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

impl AppError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            AppError::Auth(_) => StatusCode::UNAUTHORIZED,
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::InvalidPassword => StatusCode::UNAUTHORIZED,
            AppError::MeetNotFound => StatusCode::NOT_FOUND,
            AppError::InvalidMeetId => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let body = format!("{}", self);
        (status, body).into_response()
    }
} 