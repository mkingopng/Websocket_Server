// ============================
// openlifter-backend-lib/src/auth/mod.rs
// ============================
//! Authentication module.

pub mod password;
pub mod session;
mod service;
mod service_impl;

pub use password::{hash_password, verify_password, validate_password_strength, PasswordRequirements, MIN_PASSWORD_LENGTH};
pub use session::{Session, SessionManager, SESSION_TTL};
pub use service::AuthService;
pub use service_impl::DefaultAuth; 