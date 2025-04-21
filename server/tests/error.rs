// server/tests/error.rs
use axum::http::StatusCode;
use server::ServerError;
use serde::de::Error as DeError;

#[test]
fn test_error_status_codes() {
    // Test Auth error
    let auth_error = ServerError::Auth("Invalid token".to_string());
    assert_eq!(auth_error.status_code(), StatusCode::UNAUTHORIZED);
    
    // Test Internal error
    let internal_error = ServerError::Internal("Database error".to_string());
    assert_eq!(internal_error.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
    
    // Test NotFound error
    let not_found_error = ServerError::NotFound("Resource not found".to_string());
    assert_eq!(not_found_error.status_code(), StatusCode::NOT_FOUND);
    
    // Test Io error
    let io_error = ServerError::Io(std::io::Error::new(std::io::ErrorKind::Other, "IO error"));
    assert_eq!(io_error.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
    
    // Test Json error
    let json_error = ServerError::Json(serde_json::Error::custom("JSON error"));
    assert_eq!(json_error.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
    
    // Test InvalidPassword error
    let invalid_password_error = ServerError::InvalidPassword;
    assert_eq!(invalid_password_error.status_code(), StatusCode::BAD_REQUEST);
    
    // Test MeetNotFound error
    let meet_not_found_error = ServerError::MeetNotFound;
    assert_eq!(meet_not_found_error.status_code(), StatusCode::NOT_FOUND);
    
    // Test InvalidMeetId error
    let invalid_meet_id_error = ServerError::InvalidMeetId;
    assert_eq!(invalid_meet_id_error.status_code(), StatusCode::BAD_REQUEST);
}

#[test]
fn test_error_from_string() {
    let error = ServerError::from("Test error".to_string());
    match error {
        ServerError::Internal(msg) => assert_eq!(msg, "Test error"),
        _ => panic!("Expected Internal error"),
    }
}

#[test]
fn test_error_from_str() {
    let error = ServerError::from("Test error");
    match error {
        ServerError::Internal(msg) => assert_eq!(msg, "Test error"),
        _ => panic!("Expected Internal error"),
    }
} 