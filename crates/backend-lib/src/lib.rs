// ============================
// openlifter-backend-lib/src/lib.rs
// ============================
//! Core backend functionality for the OpenLifter WebSocket server.

pub mod auth;
pub mod error;
pub mod meet;
pub mod meet_actor;
pub mod storage;
pub mod ws_router;
pub mod config;

use std::sync::Arc;
use crate::auth::SessionManager;
use crate::meet::MeetManager;
use crate::storage::{Storage, FlatFileStorage};

/// Application state shared across all handlers
#[derive(Clone)]
pub struct AppState {
    /// Manager for all active meets
    pub meets: Arc<MeetManager>,
    /// Authentication session manager
    pub auth: Arc<SessionManager>,
    /// Storage backend
    pub storage: Arc<Box<dyn Storage + Send + Sync>>,
}

impl AppState {
    /// Create a new application state
    pub fn new(storage: impl Storage + Send + Sync + 'static) -> Self {
        AppState {
            meets: Arc::new(MeetManager::new()),
            auth: Arc::new(SessionManager::new()),
            storage: Arc::new(Box::new(storage)),
        }
    }
    
    /// Create a new application state with default flat file storage
    pub fn new_default() -> anyhow::Result<Self> {
        let storage = FlatFileStorage::new("data")?;
        Ok(Self::new(storage))
    }
} 