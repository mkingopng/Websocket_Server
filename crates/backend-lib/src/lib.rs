// ============================
// openlifter-backend-lib/src/lib.rs
// ============================
//! Core backend-lib functionality for the `OpenLifter` WebSocket server.

pub mod auth;
pub mod error;
pub mod meet;
pub mod meet_actor;
pub mod storage;
pub mod ws_router;
pub mod config;
pub mod metrics;
pub mod middleware;
pub mod handlers;

use std::sync::Arc;
use dashmap::DashMap;
use crate::auth::{AuthService, DefaultAuth, SessionManager};
use crate::meet::MeetManager;
use crate::storage::{Storage, FlatFileStorage};
use crate::config::{Settings, SettingsManager};

/// Application state shared across all handlers
#[derive(Clone)]
pub struct AppState<S: Storage + Send + Sync + 'static> {
    /// Manager for all active meets
    pub meets: Arc<MeetManager>,
    /// Authentication service
    pub auth_srv: Arc<dyn AuthService>,
    /// Storage backend
    pub storage: Arc<S>,
    /// Settings manager
    pub settings: Arc<SettingsManager>,
    /// Rate limiters
    pub rate_limits: Arc<DashMap<String, middleware::rate_limit::RateLimitEntry>>,
}

impl<S: Storage + Send + Sync + 'static> AppState<S> {
    /// Create a new application state
    pub fn new(
        storage: S,
        settings: Settings,
    ) -> Result<Self, anyhow::Error> {
        let session_manager = SessionManager::new();
        let auth_srv: Arc<dyn AuthService> = Arc::new(DefaultAuth::new(session_manager));
        let meets = Arc::new(MeetManager::new());
        let settings = Arc::new(SettingsManager::new(settings)?);
        let rate_limits = Arc::new(DashMap::new());
        
        Ok(AppState {
            storage: Arc::new(storage),
            auth_srv,
            meets,
            rate_limits,
            settings,
        })
    }
    
    /// Create a new application state with default settings
    pub fn new_default() -> Result<Self, anyhow::Error> 
    where
        S: From<FlatFileStorage>,
    {
        let storage = S::from(FlatFileStorage::new("data")?);
        let settings = config::load_settings()?;
        Self::new(storage, settings)
    }
} 