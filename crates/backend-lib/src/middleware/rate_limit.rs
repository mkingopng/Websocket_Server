use std::sync::Arc;
use std::time::{Duration, Instant};
use axum::{
    middleware::Next,
    response::Response,
    http::Request,
    extract::State,
};
use dashmap::DashMap;
use crate::{AppState, error::AppError};
use crate::storage::Storage;

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

    // Get rate limit settings
    let settings = state.settings.get();
    let max_requests = settings.rate_limit.max_requests;
    let window_secs = settings.rate_limit.window_secs;

    // Get or create rate limit entry
    let mut entry = state.rate_limits.entry(client_ip.to_string()).or_insert_with(|| {
        RateLimitEntry {
            requests: 0,
            window_start: Instant::now(),
        }
    });

    // Check if window has expired
    if entry.window_start.elapsed() > Duration::from_secs(window_secs) {
        entry.requests = 0;
        entry.window_start = Instant::now();
    }

    // Check if rate limit exceeded
    if entry.requests >= max_requests {
        return Err(AppError::RateLimitExceeded);
    }

    // Increment request count
    entry.requests += 1;

    // Continue to next middleware/handler
    Ok(next.run(request).await)
}

/// Rate limit entry for a client
#[derive(Debug)]
pub struct RateLimitEntry {
    requests: u32,
    window_start: Instant,
}

/// Add rate limiter to `AppState`
pub fn add_rate_limiter<S: Storage + Send + Sync + 'static>(state: &mut AppState<S>) {
    state.rate_limits = Arc::new(DashMap::new());
} 