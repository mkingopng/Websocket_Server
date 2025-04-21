// server/src/error.rs
//! Central error type + Axum integration.
use axum::{
    response::{IntoResponse, Response},
    Json,
};
use axum::http::StatusCode;
use serde::Serialize;
use thiserror::Error;
use serde_json::json;
use std::fmt;

#[derive(Debug)]
pub enum ServerError {
    Auth(String),
    Internal(String),
    NotFound(String),
    Io(std::io::Error),
    Json(serde_json::Error),
    InvalidPassword,
    MeetNotFound,
    InvalidMeetId,
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServerError::Auth(msg) => write!(f, "Authentication error: {}", msg),
            ServerError::Internal(msg) => write!(f, "Internal error: {}", msg),
            ServerError::NotFound(msg) => write!(f, "Not found: {}", msg),
            ServerError::Io(err) => write!(f, "IO error: {}", err),
            ServerError::Json(err) => write!(f, "JSON error: {}", err),
            ServerError::InvalidPassword => write!(f, "Invalid password"),
            ServerError::MeetNotFound => write!(f, "Meet not found"),
            ServerError::InvalidMeetId => write!(f, "Invalid meet ID"),
        }
    }
}

impl std::error::Error for ServerError {}

impl From<std::io::Error> for ServerError {
    fn from(err: std::io::Error) -> Self {
        ServerError::Io(err)
    }
}

impl From<serde_json::Error> for ServerError {
    fn from(err: serde_json::Error) -> Self {
        ServerError::Json(err)
    }
}

impl From<String> for ServerError {
    fn from(msg: String) -> Self {
        ServerError::Internal(msg)
    }
}

impl From<&str> for ServerError {
    fn from(msg: &str) -> Self {
        ServerError::Internal(msg.to_string())
    }
}

impl ServerError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            ServerError::Auth(_) => StatusCode::UNAUTHORIZED,
            ServerError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ServerError::NotFound(_) => StatusCode::NOT_FOUND,
            ServerError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ServerError::Json(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ServerError::InvalidPassword => StatusCode::BAD_REQUEST,
            ServerError::MeetNotFound => StatusCode::NOT_FOUND,
            ServerError::InvalidMeetId => StatusCode::BAD_REQUEST,
        }
    }
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let body = json!({
            "error": self.to_string(),
        });

        (status, axum::Json(body)).into_response()
    }
}
