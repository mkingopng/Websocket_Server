// crates/backend-lib/src/middleware/rate_limit.rs

//! Rate limiting middleware
use crate::storage::Storage;
use crate::{error::AppError, AppState};
use axum::{extract::State, http::Request, middleware::Next, response::Response};
use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Rate limiter middleware
pub async fn rate_limit<S: Storage + Send + Sync + 'static>(
    State(state): State<Arc<AppState<S>>>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, AppError> {
    // Get client IP
    let client_ip = request
        .headers()
        .get("x-real-ip")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("unknown");

    // Check rate limit
    if !state.rate_limiter.check_rate_limit(client_ip) {
        return Err(AppError::RateLimitExceeded);
    }

    // Continue to next middleware/handler
    Ok(next.run(request).await)
}

/// Rate limit entry for a client
pub struct RateLimitEntry {
    pub last_request: Instant,
    pub count: u32,
}

pub struct RateLimiter {
    entries: Arc<DashMap<String, RateLimitEntry>>,
    window: Duration,
    max_requests: u32,
}

impl RateLimiter {
    pub fn new(window: Duration, max_requests: u32) -> Self {
        Self {
            entries: Arc::new(DashMap::new()),
            window,
            max_requests,
        }
    }

    pub fn check_rate_limit(&self, client_ip: &str) -> bool {
        let now = Instant::now();
        let mut entry = self
            .entries
            .entry(client_ip.to_string())
            .or_insert_with(|| RateLimitEntry {
                last_request: now,
                count: 0,
            });

        if now.duration_since(entry.last_request) > self.window {
            entry.count = 1;
            entry.last_request = now;
            true
        } else {
            entry.count += 1;
            entry.count <= self.max_requests
        }
    }

    pub fn clear_expired(&self) {
        let now = Instant::now();
        self.entries
            .retain(|_, entry| now.duration_since(entry.last_request) <= self.window);
    }
}

pub fn check_rate_limit<S: Storage + Send + Sync + 'static>(
    state: &Arc<AppState<S>>,
    client_ip: &str,
) -> Result<(), AppError> {
    if !state.rate_limiter.check_rate_limit(client_ip) {
        return Err(AppError::RateLimitExceeded);
    }
    Ok(())
}

pub fn init_rate_limiter<S: Storage + Send + Sync + 'static>(state: &mut AppState<S>) {
    state.rate_limiter = Arc::new(RateLimiter::new(Duration::from_secs(60), 100));
}
