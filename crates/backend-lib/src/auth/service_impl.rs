// ==========
// crates/backend-lib/src/auth/service_impl.rs
// ===========
//! Authentication service implementation
use crate::auth::{AuthRateLimiter, AuthService, PersistentSessionManager};
use crate::error::AppError;
use crate::messages::Session;
use async_trait::async_trait;
use std::any::Any;
use std::net::IpAddr;
use std::sync::Arc;

pub struct DefaultAuth {
    sm: PersistentSessionManager,
    rate_limiter: Arc<AuthRateLimiter>,
}

impl DefaultAuth {
    pub fn new(sm: PersistentSessionManager) -> Self {
        Self {
            sm,
            rate_limiter: Arc::new(AuthRateLimiter::default()),
        }
    }

    pub fn new_with_rate_limiter(
        sm: PersistentSessionManager,
        rate_limiter: Arc<AuthRateLimiter>,
    ) -> Self {
        Self { sm, rate_limiter }
    }

    /// Check if authentication is allowed for this IP
    pub fn check_auth_rate_limit(&self, ip: IpAddr) -> Result<(), AppError> {
        if !self.rate_limiter.check_rate_limit(ip) {
            return Err(AppError::AuthRateLimited);
        }
        Ok(())
    }

    /// Record a failed authentication attempt
    pub fn record_failed_attempt(&self, ip: IpAddr) {
        self.rate_limiter.record_failed_attempt(ip);
    }

    /// Record a successful authentication
    pub fn record_success(&self, ip: IpAddr) {
        self.rate_limiter.record_success(ip);
    }
}

#[async_trait]
impl AuthService for DefaultAuth {
    async fn new_session(&self, meet_id: String, location_name: String, priority: u8) -> String {
        let session = self
            .sm
            .create_session(meet_id, location_name, priority)
            .await;
        session.token
    }

    async fn get_session(&self, token: &str) -> Option<Session> {
        self.sm.get_session(token).await
    }

    async fn validate_session(&self, token: &str) -> bool {
        self.sm.validate_session(token).await
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
