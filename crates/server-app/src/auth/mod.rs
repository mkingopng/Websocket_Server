// ============================
// crates/server-app/src/auth/mod.rs
// ============================
//! Authentication module.
pub mod password;
pub mod rate_limit;
pub mod service;
pub mod service_impl;
pub mod session;
pub mod token_generator;

// Re-export key types for convenience
pub use password::{
    hash_password, hash_password_secure, validate_password_strength, verify_password,
    PasswordRequirements, MIN_PASSWORD_LENGTH,
};
pub use rate_limit::AuthRateLimiter;
pub use service::AuthService;
pub use service_impl::DefaultAuth;
pub use session::{
    PersistentSessionManager, SessionConfig, SessionEntry, SessionManager, SessionManagerTrait,
};
pub use token_generator::generate_secure_token;
