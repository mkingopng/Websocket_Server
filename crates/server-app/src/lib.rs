// ============================
// crates/server-app/src/lib.rs
// ============================
#![allow(clippy::all, clippy::nursery, clippy::pedantic)]

pub mod auth;
pub mod config;
pub mod error;
pub mod handlers;
pub mod meet;
pub mod meet_actor;
pub mod messages;
pub mod metrics;
pub mod middleware;
pub mod storage;
pub mod validation;
pub mod websocket;
pub mod ws_router;

use crate::auth::{AuthRateLimiter, AuthService, DefaultAuth, PersistentSessionManager};
use crate::config::Settings;
use crate::meet_actor::MeetHandle;
use crate::middleware::rate_limit::RateLimiter;
use crate::storage::FlatFileStorage;
use std::error::Error;
use std::path::PathBuf;
use std::sync::Arc;

/// Application state shared across all handlers
#[derive(Clone)]
pub struct AppState<S> {
    /// Authentication service
    pub auth: Arc<dyn AuthService>,
    /// Session manager
    pub sessions: Arc<PersistentSessionManager>,
    /// Storage backend
    pub storage: S,
    /// Configuration settings
    pub settings: Arc<Settings>,
    /// Rate limiter
    pub rate_limiter: Arc<RateLimiter>,
    /// Auth rate limiter
    pub auth_rate_limiter: Arc<AuthRateLimiter>,
    /// Connected clients by meet ID
    pub clients:
        Arc<dashmap::DashMap<String, Vec<tokio::sync::mpsc::Sender<messages::ServerMessage>>>>,
    /// Active meet handles
    pub meet_handles: Arc<dashmap::DashMap<String, MeetHandle>>,
}

impl<S> AppState<S> {
    /// Create a new application state
    pub async fn new(storage: S, config: &Settings) -> Result<Self, Box<dyn Error>> {
        // Create sessions directory in the storage path
        let sessions_path = PathBuf::from(&config.storage.path).join("sessions");
        let sessions = PersistentSessionManager::new(&sessions_path).await?;

        let auth_rate_limiter = Arc::new(AuthRateLimiter::default());
        let auth = Arc::new(DefaultAuth::new_with_rate_limiter(
            sessions.clone(),
            auth_rate_limiter.clone(),
        ));
        let settings = Arc::new(config.clone());
        let rate_limiter = Arc::new(RateLimiter::new(std::time::Duration::from_secs(60), 100));
        let clients = Arc::new(dashmap::DashMap::new());
        let meet_handles = Arc::new(dashmap::DashMap::new());

        Ok(Self {
            auth,
            sessions: Arc::new(sessions),
            storage,
            settings,
            rate_limiter,
            auth_rate_limiter,
            clients,
            meet_handles,
        })
    }

    /// Create a new application state with default settings
    pub async fn new_default() -> Result<Self, anyhow::Error>
    where
        S: From<FlatFileStorage>,
    {
        let storage = S::from(FlatFileStorage::new("data")?);
        let settings = Settings::load()?;
        Self::new(storage, &settings)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))
    }
}
