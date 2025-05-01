// ============================
// crates/backend-lib/src/auth/rate_limit.rs
// ============================
//! Rate limiting for authentication attempts.

use dashmap::DashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Default number of failed attempts before rate limiting
const DEFAULT_MAX_ATTEMPTS: u32 = 5;

/// Default lockout duration (5 minutes)
const DEFAULT_LOCKOUT_DURATION: Duration = Duration::from_secs(5 * 60);

/// Entry in the rate limit map
#[derive(Debug, Clone)]
struct RateLimitEntry {
    /// Number of failed attempts
    failed_attempts: u32,
    /// Time of the last failed attempt
    last_failure: Instant,
    /// Whether the IP is currently locked out
    is_locked_out: bool,
    /// When the lockout expires
    lockout_expiry: Option<Instant>,
}

/// Rate limiter for authentication attempts
#[derive(Debug, Clone)]
pub struct AuthRateLimiter {
    /// Map of IP addresses to rate limit entries
    attempts: Arc<DashMap<IpAddr, RateLimitEntry>>,
    /// Maximum number of failed attempts before lockout
    max_attempts: u32,
    /// Duration of lockout period
    lockout_duration: Duration,
}

impl Default for AuthRateLimiter {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_ATTEMPTS, DEFAULT_LOCKOUT_DURATION)
    }
}

impl AuthRateLimiter {
    /// Create a new auth rate limiter
    pub fn new(max_attempts: u32, lockout_duration: Duration) -> Self {
        Self {
            attempts: Arc::new(DashMap::new()),
            max_attempts,
            lockout_duration,
        }
    }

    /// Record a failed authentication attempt
    pub fn record_failed_attempt(&self, ip: IpAddr) {
        let now = Instant::now();

        // Check if entry exists, if not create it
        let mut entry = self.attempts.entry(ip).or_insert_with(|| RateLimitEntry {
            failed_attempts: 0,
            last_failure: now,
            is_locked_out: false,
            lockout_expiry: None,
        });

        // Check if lockout has expired
        if let Some(expiry) = entry.lockout_expiry {
            if now > expiry {
                // Reset if lockout has expired
                entry.is_locked_out = false;
                entry.failed_attempts = 0;
                entry.lockout_expiry = None;
            }
        }

        // Increment failed attempts
        entry.failed_attempts += 1;
        entry.last_failure = now;

        // Check if we need to lock out
        if entry.failed_attempts >= self.max_attempts {
            entry.is_locked_out = true;
            entry.lockout_expiry = Some(now + self.lockout_duration);

            // Log the lockout
            println!("IP {ip} locked out for authentication attempts");
        }
    }

    /// Record a successful authentication
    pub fn record_success(&self, ip: IpAddr) {
        // On successful auth, remove the entry
        self.attempts.remove(&ip);
    }

    /// Check if an IP is allowed to attempt authentication
    pub fn check_rate_limit(&self, ip: IpAddr) -> bool {
        if let Some(entry) = self.attempts.get(&ip) {
            // If locked out and lockout hasn't expired, deny
            if entry.is_locked_out {
                if let Some(expiry) = entry.lockout_expiry {
                    if Instant::now() < expiry {
                        return false;
                    }
                }
            }
        }

        true
    }

    /// Clean up expired lockouts
    pub fn cleanup(&self) {
        let now = Instant::now();

        // Remove expired entries
        self.attempts.retain(|_, entry| {
            // If locked out but expired, remove
            if entry.is_locked_out {
                if let Some(expiry) = entry.lockout_expiry {
                    return now < expiry;
                }
            }

            // Otherwise, keep entries for a day
            now.duration_since(entry.last_failure) < Duration::from_secs(24 * 60 * 60)
        });
    }
}
