// ============================
// openlifter-backend-lib/src/lib.rs
// ============================
//! Core backend-lib functionality for the OpenLifter WebSocket server.

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
use crate::auth::{SessionManager, DefaultAuth, AuthService};
use crate::meet::MeetManager;
use crate::storage::{Storage, FlatFileStorage};
use crate::config::{Settings, SettingsManager};

/// Application state shared across all handlers
#[derive(Clone)]
pub struct AppState {
    /// Manager for all active meets
    pub meets: Arc<MeetManager>,
    /// Authentication service
    pub auth_srv: Arc<dyn AuthService>,
    /// Storage backend
    pub storage: Arc<Box<dyn Storage + Send + Sync>>,
    /// Settings manager
    pub settings: Arc<SettingsManager>,
    /// Rate limiters
    pub rate_limits: Arc<DashMap<String, middleware::rate_limit::RateLimitEntry>>,
}

impl AppState {
    /// Create a new application state
    pub fn new(storage: impl Storage + Send + Sync + 'static, settings: Settings) -> Result<Self, anyhow::Error> {
        let settings = SettingsManager::new(settings)?;
        let mut state = AppState {
            meets: Arc::new(MeetManager::new()),
            auth_srv: Arc::new(DefaultAuth::new(SessionManager::new())),
            storage: Arc::new(Box::new(storage)),
            settings: Arc::new(settings),
            rate_limits: Arc::new(DashMap::new()),
        };
        middleware::rate_limit::add_rate_limiter(&mut state);
        Ok(state)
    }
    
    /// Create a new application state with default settings
    pub fn new_default() -> Result<Self, anyhow::Error> {
        let storage = FlatFileStorage::new("data")?;
        let settings = config::load_settings()?;
        Self::new(storage, settings)
    }
} 