// ===========================
// server/tests/websocket.rs
// ===========================
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use common::{ClientToServer};
use server::{
    AppState,
    ws_router,
    SessionManager,
    FlatFileStorage,
    MeetManager,
};
use std::sync::Arc;

async fn create_test_app_state() -> AppState {
    AppState {
        meets: Arc::new(MeetManager::new()),
        auth: Arc::new(SessionManager::new()),
        storage: Arc::new(FlatFileStorage::new("test-data").expect("Failed to initialize storage")),
    }
}

#[tokio::test]
async fn test_create_meet() {
    let state = create_test_app_state();
    let app = ws_router::router(state);

    let create_meet_msg = ClientToServer::CreateMeet {
        this_location_name: "Test Location".to_string(),
        password: "test123".to_string(),
        endpoints: vec![common::EndpointPriority {
            location_name: "Test Location".to_string(),
            priority: 1,
        }],
    };

    let json = serde_json::to_string(&create_meet_msg).unwrap();
    
    let response = app
        .oneshot(Request::builder()
            .uri("/ws")
            .body(Body::from(json))
            .unwrap())
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    // In a real test, we would parse the response and verify the meet was created
    // For now, we're just testing that the endpoint responds correctly
}

#[tokio::test]
async fn test_join_meet() {
    // This test would require creating a meet first, then joining it
    // For simplicity, we're just setting up the structure
    let state = create_test_app_state();
    let app = ws_router::router(state);
    
    // Create a meet first
    // ...
    
    // Then try to join it
    let join_meet_msg = ClientToServer::JoinMeet {
        meet_id: "test-meet-id".to_string(),
        password: "test123".to_string(),
        location_name: "Test Location".to_string(),
    };
    
    let json = serde_json::to_string(&join_meet_msg).unwrap();
    
    let response = app
        .oneshot(Request::builder()
            .uri("/ws")
            .body(Body::from(json))
            .unwrap())
        .await
        .unwrap();
    
    // We expect this to fail since we didn't actually create the meet
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
