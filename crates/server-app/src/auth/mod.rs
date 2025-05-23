// ============================
// crates/server-app/src/auth/mod.rs
// ============================
//! Authentication module.
pub mod password;
pub mod persistent_session;
pub mod rate_limit;
mod service;
mod service_impl;
pub mod session;
pub mod token_generator;

pub use password::{
    hash_password, validate_password_strength, verify_password, PasswordRequirements,
    MIN_PASSWORD_LENGTH,
};
pub use persistent_session::PersistentSessionManager;
pub use rate_limit::AuthRateLimiter;
pub use service::AuthService;
pub use service_impl::DefaultAuth;
pub use session::{SessionManager, SESSION_ABSOLUTE_TTL, SESSION_IDLE_TTL};
