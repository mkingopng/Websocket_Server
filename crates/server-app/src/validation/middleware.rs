//! Validation middleware for consolidating repetitive validation patterns.
//!
//! This module provides a centralized validation approach that replaces the
//! repetitive validate_or_error! macro usage throughout the codebase.

use super::{ValidationError, ValidationResult};
use crate::messages::ServerMessage;
use openlifter_common::ClientToServer;
use std::collections::HashMap;

/// Validation middleware that provides structured validation for client messages
pub struct ValidationMiddleware;

/// Validation context containing common information needed for validation
#[derive(Debug, Clone)]
pub struct ValidationContext {
    pub error_mappings: HashMap<String, String>,
}

impl Default for ValidationContext {
    fn default() -> Self {
        let mut error_mappings = HashMap::new();

        // Map validation error types to HTTP-like error codes
        error_mappings.insert("InvalidMeetId".to_string(), "INVALID_MEET_ID".to_string());
        error_mappings.insert(
            "InvalidPassword".to_string(),
            "INVALID_PASSWORD".to_string(),
        );
        error_mappings.insert(
            "InvalidLocationName".to_string(),
            "INVALID_LOCATION".to_string(),
        );
        error_mappings.insert(
            "InvalidSessionToken".to_string(),
            "INVALID_SESSION_TOKEN".to_string(),
        );
        error_mappings.insert("InvalidUpdate".to_string(), "INVALID_UPDATE".to_string());
        error_mappings.insert("InvalidEmail".to_string(), "INVALID_EMAIL".to_string());
        error_mappings.insert("InvalidCsvData".to_string(), "INVALID_CSV_DATA".to_string());
        error_mappings.insert("MeetIdExists".to_string(), "MEET_ID_EXISTS".to_string());

        Self { error_mappings }
    }
}

impl ValidationMiddleware {
    /// Validate a client message and return appropriate server response on error
    pub fn validate_message(
        message: &ClientToServer,
        context: &ValidationContext,
    ) -> Result<(), ServerMessage> {
        match super::validate_client_message(message) {
            Ok(()) => Ok(()),
            Err(validation_error) => {
                let error_code = Self::map_validation_error(&validation_error, context);
                Err(ServerMessage::Error {
                    code: error_code,
                    message: validation_error.to_string(),
                })
            },
        }
    }

    /// Validate individual fields and return server error response on failure
    pub fn validate_field<T>(
        validation_result: ValidationResult<T>,
        error_code: &str,
    ) -> Result<T, ServerMessage> {
        match validation_result {
            Ok(value) => Ok(value),
            Err(e) => Err(ServerMessage::Error {
                code: error_code.to_string(),
                message: e.to_string(),
            }),
        }
    }

    /// Validate multiple fields and collect errors
    pub fn validate_fields(
        validations: Vec<(&str, ValidationResult<()>)>,
    ) -> Result<(), ServerMessage> {
        let mut errors = Vec::new();

        for (field_name, result) in validations {
            if let Err(e) = result {
                errors.push(format!("{}: {}", field_name, e));
            }
        }

        if !errors.is_empty() {
            return Err(ServerMessage::Error {
                code: "VALIDATION_FAILED".to_string(),
                message: errors.join("; "),
            });
        }

        Ok(())
    }

    /// Map ValidationError to appropriate error code
    fn map_validation_error(error: &ValidationError, context: &ValidationContext) -> String {
        let error_type = match error {
            ValidationError::InvalidMeetId(_) => "InvalidMeetId",
            ValidationError::InvalidPassword(_) => "InvalidPassword",
            ValidationError::InvalidLocationName(_) => "InvalidLocationName",
            ValidationError::InvalidSessionToken(_) => "InvalidSessionToken",
            ValidationError::InvalidUpdate(_) => "InvalidUpdate",
            ValidationError::InvalidEmail(_) => "InvalidEmail",
            ValidationError::InvalidCsvData(_) => "InvalidCsvData",
            ValidationError::MeetIdExists(_) => "MeetIdExists",
        };

        context
            .error_mappings
            .get(error_type)
            .cloned()
            .unwrap_or_else(|| "VALIDATION_ERROR".to_string())
    }

    /// Builder pattern for common validation workflows
    pub fn builder() -> ValidationBuilder {
        ValidationBuilder::new()
    }
}

/// Builder for constructing validation workflows
pub struct ValidationBuilder {
    validations: Vec<Box<dyn Fn() -> ValidationResult<()>>>,
    context: ValidationContext,
}

impl ValidationBuilder {
    pub fn new() -> Self {
        Self {
            validations: Vec::new(),
            context: ValidationContext::default(),
        }
    }

    /// Add a validation step
    pub fn validate<F>(mut self, validation: F) -> Self
    where
        F: Fn() -> ValidationResult<()> + 'static,
    {
        self.validations.push(Box::new(validation));
        self
    }

    /// Add meet ID validation
    pub fn meet_id(self, meet_id: &str) -> ValidationBuilderWithData<String> {
        ValidationBuilderWithData {
            inner: self,
            data: meet_id.to_string(),
            validator: Box::new(|data| super::validate_meet_id(data).map(|_| ())),
            error_code: "INVALID_MEET_ID".to_string(),
        }
    }

    /// Add password validation
    pub fn password(self, password: &str) -> ValidationBuilderWithData<String> {
        ValidationBuilderWithData {
            inner: self,
            data: password.to_string(),
            validator: Box::new(|data| super::validate_password(data).map(|_| ())),
            error_code: "INVALID_PASSWORD".to_string(),
        }
    }

    /// Add location name validation
    pub fn location_name(self, location_name: &str) -> ValidationBuilderWithData<String> {
        ValidationBuilderWithData {
            inner: self,
            data: location_name.to_string(),
            validator: Box::new(|data| super::validate_location_name(data).map(|_| ())),
            error_code: "INVALID_LOCATION".to_string(),
        }
    }

    /// Execute all validations
    pub fn execute(self) -> Result<(), ServerMessage> {
        for validation in self.validations {
            if let Err(e) = validation() {
                let error_code = ValidationMiddleware::map_validation_error(&e, &self.context);
                return Err(ServerMessage::Error {
                    code: error_code,
                    message: e.to_string(),
                });
            }
        }
        Ok(())
    }
}

/// Builder with typed data for validation
pub struct ValidationBuilderWithData<T> {
    inner: ValidationBuilder,
    data: T,
    validator: Box<dyn Fn(&T) -> ValidationResult<()>>,
    error_code: String,
}

impl<T: 'static> ValidationBuilderWithData<T> {
    /// Get the validated data
    pub fn get_data(self) -> Result<T, ServerMessage> {
        match (self.validator)(&self.data) {
            Ok(()) => Ok(self.data),
            Err(e) => Err(ServerMessage::Error {
                code: self.error_code,
                message: e.to_string(),
            }),
        }
    }

    /// Continue building with this validation included
    pub fn and(mut self) -> ValidationBuilder {
        let data = self.data;
        let validator = self.validator;

        self.inner
            .validations
            .push(Box::new(move || validator(&data)));

        self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlifter_common::{ClientToServer, EndpointPriority};

    #[test]
    fn test_validation_middleware_valid_message() {
        let message = ClientToServer::CreateMeet {
            this_location_name: "Valid Location".to_string(),
            password: "ValidPassword123".to_string(),
            endpoints: vec![EndpointPriority {
                location_name: "Location1".to_string(),
                priority: 1,
            }],
        };

        let context = ValidationContext::default();
        let result = ValidationMiddleware::validate_message(&message, &context);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validation_middleware_invalid_message() {
        let message = ClientToServer::CreateMeet {
            this_location_name: "".to_string(), // Invalid: empty
            password: "short".to_string(),      // Invalid: too short
            endpoints: vec![],
        };

        let context = ValidationContext::default();
        let result = ValidationMiddleware::validate_message(&message, &context);
        assert!(result.is_err());

        if let Err(ServerMessage::Error { code, message: _ }) = result {
            assert!(!code.is_empty());
        }
    }

    #[test]
    fn test_validation_builder() {
        let result = ValidationMiddleware::builder()
            .meet_id("valid-meet-id")
            .and()
            .password("ValidPassword123")
            .and()
            .location_name("Valid Location")
            .and()
            .execute();

        assert!(result.is_ok());
    }

    #[test]
    fn test_validation_builder_invalid() {
        let result = ValidationMiddleware::builder()
            .meet_id("") // Invalid: empty
            .and()
            .execute();

        assert!(result.is_err());
    }

    #[test]
    fn test_validate_field() {
        // Valid field
        let result = ValidationMiddleware::validate_field(
            super::super::validate_meet_id("valid-meet"),
            "INVALID_MEET_ID",
        );
        assert!(result.is_ok());

        // Invalid field
        let result = ValidationMiddleware::validate_field(
            super::super::validate_meet_id(""),
            "INVALID_MEET_ID",
        );
        assert!(result.is_err());
    }
}
