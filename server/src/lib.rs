// server/src/lib.rs
pub mod auth;
pub mod error;
pub mod meet;
pub mod meet_actor;
pub mod storage;
pub mod ws_router;

pub use crate::auth::SessionManager;
pub use crate::error::ServerError;
pub use crate::meet::{Meet, MeetManager};
pub use crate::storage::FlatFileStorage;
pub use crate::ws_router::router as ws_router;

use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub meets: Arc<MeetManager>,
    pub auth: Arc<SessionManager>,
    pub storage: Arc<FlatFileStorage>,
} 
