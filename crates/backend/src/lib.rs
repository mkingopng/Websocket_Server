// ============================
// openlifter-backend/src/lib.rs
// ============================
//! Core backend functionality for the OpenLifter WebSocket server.

pub mod auth;
pub mod error;
pub mod meet_actor;
pub mod storage;
pub mod ws_router;

use dashmap::DashMap;
use std::sync::Arc;

pub use crate::error::AppError;
pub use crate::auth::SessionManager;
pub use crate::storage::FlatFileStorage;

pub type MeetId = String;

/// Handle we keep for every live meet
pub type MeetMap = Arc<DashMap<MeetId, meet_actor::MeetHandle>>;

/// Wrapper for MeetMap to provide additional methods
pub struct MeetManager {
    meets: MeetMap,
}

impl MeetManager {
    pub fn new() -> Self {
        MeetManager {
            meets: Arc::new(DashMap::new()),
        }
    }
    
    pub fn create_meet(&self, meet_id: String, _location_name: String, _client_id: String) {
        let handle = meet_actor::MeetHandle::new(meet_id.clone());
        self.meets.insert(meet_id, handle);
    }
    
    pub fn get_meet(&self, meet_id: &str) -> Option<meet_actor::MeetHandle> {
        self.meets.get(meet_id).map(|entry| entry.value().clone())
    }
    
    pub fn delete_meet(&self, meet_id: &str) {
        self.meets.remove(meet_id);
    }
}

#[derive(Clone)]
pub struct AppState {
    /// Map: live meet id → channels handle (cmd + relay)
    pub meets: Arc<MeetManager>,
    pub auth: Arc<SessionManager>,
    pub storage: Arc<FlatFileStorage>,
}

impl AppState {
    pub fn new() -> Self {
        AppState {
            meets: Arc::new(MeetManager::new()),
            auth: Arc::new(SessionManager::new()),
            storage: Arc::new(FlatFileStorage::new("data").expect("Failed to initialize storage")),
        }
    }
} 