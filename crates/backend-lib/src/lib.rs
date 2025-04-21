// ============================
// openlifter-backend-lib/src/lib.rs
// ============================
//! Core backend-lib functionality for the `OpenLifter` WebSocket server.

pub mod config;
pub mod storage;
pub mod messages;
pub mod auth;
pub mod meet;
pub mod error;
pub mod metrics;
pub mod middleware;
pub mod ws_router;
pub mod meet_actor;
pub mod websocket;

use std::sync::Arc;
use std::error::Error;
use crate::auth::{AuthService, DefaultAuth, SessionManager};
use crate::config::Settings;
use crate::storage::FlatFileStorage;
use crate::middleware::rate_limit::RateLimiter;

/// Application state shared across all handlers
#[derive(Clone)]
pub struct AppState<S> {
    /// Authentication service
    pub auth: Arc<dyn AuthService>,
    /// Session manager
    pub sessions: Arc<SessionManager>,
    /// Settings manager
    pub settings: Arc<Settings>,
    /// Storage backend
    pub storage: S,
    /// Rate limiter
    pub rate_limiter: Arc<RateLimiter>,
}

impl<S> AppState<S> {
    /// Create a new application state
    pub fn new(storage: S, config: Settings) -> Result<Self, Box<dyn Error>> {
        let sessions = Arc::new(SessionManager::new());
        let auth = Arc::new(DefaultAuth::new((*sessions).clone()));
        let settings = Arc::new(config.clone());
        let rate_limiter = Arc::new(RateLimiter::new(
            std::time::Duration::from_secs(60),
            100,
        ));
        
        Ok(Self {
            auth,
            sessions,
            settings,
            storage,
            rate_limiter,
        })
    }
    
    /// Create a new application state with default settings
    pub fn new_default() -> Result<Self, anyhow::Error> 
    where
        S: From<FlatFileStorage>,
    {
        let storage = S::from(FlatFileStorage::new("data")?);
        let settings = Settings::load()?;
        Self::new(storage, settings).map_err(|e| anyhow::anyhow!("{}", e))
    }
} 