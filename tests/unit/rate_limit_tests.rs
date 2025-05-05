// ==============================
// tests/unit/rate_limit_tests.rs
// ==============================
//! This test suite is designed to validate the functionality of the `AuthRateLimiter`
use backend_lib::auth::AuthRateLimiter;
use std::net::{IpAddr, Ipv4Addr};

#[test]
fn test_rate_limiter_allows_initial_attempts() {
    let rate_limiter = AuthRateLimiter::default();
    let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

    // First attempt should be allowed
    assert!(rate_limiter.check_rate_limit(ip));
}

#[test]
fn test_rate_limiter_blocks_after_max_attempts() {
    let rate_limiter = AuthRateLimiter::default();
    let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 2));

    // Record failed attempts (default max is 5)
    for _ in 0..5 {
        rate_limiter.record_failed_attempt(ip);
    }

    // After 5 failures, should be blocked
    assert!(!rate_limiter.check_rate_limit(ip));
}

#[test]
fn test_rate_limiter_resets_after_success() {
    let rate_limiter = AuthRateLimiter::default();
    let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 3));

    // Record 3 failed attempts
    for _ in 0..3 {
        rate_limiter.record_failed_attempt(ip);
    }

    // Should still be allowed
    assert!(rate_limiter.check_rate_limit(ip));

    // Record a success
    rate_limiter.record_success(ip);

    // Failed attempts should be reset
    assert!(rate_limiter.check_rate_limit(ip));

    // Can make 5 more attempts
    for _ in 0..5 {
        rate_limiter.record_failed_attempt(ip);
    }

    // Now should be blocked
    assert!(!rate_limiter.check_rate_limit(ip));
}

#[test]
fn test_different_ips_tracked_separately() {
    let rate_limiter = AuthRateLimiter::default();
    let ip1 = IpAddr::V4(Ipv4Addr::new(192, 168, 0, 1));
    let ip2 = IpAddr::V4(Ipv4Addr::new(192, 168, 0, 2));

    // Block IP1
    for _ in 0..5 {
        rate_limiter.record_failed_attempt(ip1);
    }

    // IP1 should be blocked
    assert!(!rate_limiter.check_rate_limit(ip1));

    // IP2 should still be allowed
    assert!(rate_limiter.check_rate_limit(ip2));
}
