// ============================
// openlifter-backend-lib/src/validation/mod.rs
// ============================
//! Message validation module.

use crate::messages::{ClientMessage, Update};
use thiserror::Error;

/// Possible validation errors
#[derive(Error, Debug)]
pub enum ValidationError {
    #[error("Invalid meet ID: {0}")]
    InvalidMeetId(String),

    #[error("Invalid password: {0}")]
    InvalidPassword(String),

    #[error("Invalid location name: {0}")]
    InvalidLocationName(String),

    #[error("Invalid session token: {0}")]
    InvalidSessionToken(String),

    #[error("Invalid update: {0}")]
    InvalidUpdate(String),

    #[error("Invalid email: {0}")]
    InvalidEmail(String),

    #[error("Invalid CSV data: {0}")]
    InvalidCsvData(String),
}

/// Result type for validation operations
pub type ValidationResult<T> = Result<T, ValidationError>;

/// Validate a meet ID
pub fn validate_meet_id(meet_id: &str) -> ValidationResult<()> {
    // Meet ID should not be empty
    if meet_id.is_empty() {
        return Err(ValidationError::InvalidMeetId(
            "Meet ID must not be empty".to_string(),
        ));
    }

    // Meet ID should not be too long (50 chars max)
    if meet_id.len() > 50 {
        return Err(ValidationError::InvalidMeetId(
            "Meet ID must be between 1 and 50 characters".to_string(),
        ));
    }

    // Meet ID should only contain alphanumeric characters and hyphens
    if !meet_id.chars().all(|c| c.is_alphanumeric() || c == '-') {
        return Err(ValidationError::InvalidMeetId(
            "Meet ID must contain only alphanumeric characters and hyphens".to_string(),
        ));
    }

    Ok(())
}

/// Validate a password
pub fn validate_password(password: &str) -> ValidationResult<()> {
    // Password should be at least 10 characters long
    if password.len() < 10 {
        return Err(ValidationError::InvalidPassword(
            "Password must be at least 10 characters".to_string(),
        ));
    }

    // Password should contain at least one uppercase letter, one lowercase letter, and one number
    let has_uppercase = password.chars().any(char::is_uppercase);
    let has_lowercase = password.chars().any(char::is_lowercase);
    let has_digit = password.chars().any(|c| c.is_ascii_digit());

    if !has_uppercase || !has_lowercase || !has_digit {
        return Err(ValidationError::InvalidPassword(
            "Password must contain at least one uppercase letter, one lowercase letter, and one number".to_string(),
        ));
    }

    Ok(())
}

/// Validate a location name
pub fn validate_location_name(location_name: &str) -> ValidationResult<()> {
    // Location name should not be empty
    if location_name.is_empty() {
        return Err(ValidationError::InvalidLocationName(
            "Location name must not be empty".to_string(),
        ));
    }

    // Location name should not be too long (100 chars max)
    if location_name.len() > 100 {
        return Err(ValidationError::InvalidLocationName(
            "Location name must be between 1 and 100 characters".to_string(),
        ));
    }

    Ok(())
}

/// Validate a session token
pub fn validate_session_token(token: &str) -> ValidationResult<()> {
    // Session token should not be empty
    if token.is_empty() {
        return Err(ValidationError::InvalidSessionToken(
            "Session token must not be empty".to_string(),
        ));
    }

    Ok(())
}

/// Validate an email address
pub fn validate_email(email: &str) -> ValidationResult<()> {
    // This is a very basic validation - in a real app, you'd use a proper regex
    if !email.contains('@') || !email.contains('.') {
        return Err(ValidationError::InvalidEmail(
            "Email must be a valid email address".to_string(),
        ));
    }

    Ok(())
}

/// Validate an update
pub fn validate_update(update: &Update) -> ValidationResult<()> {
    // Update location should not be empty
    if update.location.is_empty() {
        return Err(ValidationError::InvalidUpdate(
            "Update location must not be empty".to_string(),
        ));
    }

    // Timestamp should be a positive number
    if update.timestamp <= 0 {
        return Err(ValidationError::InvalidUpdate(
            "Update timestamp must be positive".to_string(),
        ));
    }

    Ok(())
}

/// Validates a client message
pub fn validate_client_message(message: &ClientMessage) -> ValidationResult<()> {
    match message {
        ClientMessage::CreateMeet {
            meet_id,
            password,
            location_name,
            priority: _,
        } => {
            validate_meet_id(meet_id)?;
            validate_password(password)?;
            validate_location_name(location_name)?;
        },
        ClientMessage::JoinMeet {
            meet_id,
            password,
            location_name,
            priority: _,
        } => {
            validate_meet_id(meet_id)?;
            validate_password(password)?;
            validate_location_name(location_name)?;
        },
        ClientMessage::UpdateInit {
            meet_id,
            session_token,
            updates,
        } => {
            validate_meet_id(meet_id)?;
            validate_session_token(session_token)?;

            // Validate each update
            for update in updates {
                validate_update(update)?;
            }
        },
        ClientMessage::ClientPull {
            meet_id,
            session_token,
            last_server_seq: _,
        } => {
            validate_meet_id(meet_id)?;
            validate_session_token(session_token)?;
        },
        ClientMessage::PublishMeet {
            meet_id,
            session_token,
            return_email,
            opl_csv,
        } => {
            validate_meet_id(meet_id)?;
            validate_session_token(session_token)?;
            validate_email(return_email)?;

            // CSV data should not be empty
            if opl_csv.is_empty() {
                return Err(ValidationError::InvalidCsvData(
                    "CSV data must not be empty".to_string(),
                ));
            }
        },
    }

    Ok(())
}
