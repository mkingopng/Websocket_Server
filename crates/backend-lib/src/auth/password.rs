// ============================
// crates/backend-lib/src/auth/password.rs
// ============================
//! Password hashing and verification.
use argon2::Argon2;
use scrypt::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Scrypt,
};
use zeroize::Zeroize;

/// Minimum password length
pub const MIN_PASSWORD_LENGTH: usize = 10;

/// Password complexity requirements
#[allow(clippy::struct_excessive_bools)]
pub struct PasswordRequirements {
    pub min_length: usize,
    pub require_uppercase: bool,
    pub require_lowercase: bool,
    pub require_digit: bool,
    pub require_special: bool,
}

impl Default for PasswordRequirements {
    fn default() -> Self {
        Self {
            min_length: MIN_PASSWORD_LENGTH,
            require_uppercase: true,
            require_lowercase: true,
            require_digit: true,
            require_special: true,
        }
    }
}

/// Hash a password using scrypt
pub fn hash_password(plain: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = Scrypt.hash_password(plain.as_bytes(), &salt)?.to_string();
    Ok(hash)
}

/// Verify a password against a hash
pub fn verify_password(hash: &str, password: &str) -> bool {
    let Ok(parsed_hash) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok()
}

/// Check if a password meets the complexity requirements
pub fn validate_password_strength(password: &str, requirements: &PasswordRequirements) -> bool {
    if password.len() < requirements.min_length {
        return false;
    }

    if requirements.require_uppercase && !password.chars().any(char::is_uppercase) {
        return false;
    }

    if requirements.require_lowercase && !password.chars().any(char::is_lowercase) {
        return false;
    }

    if requirements.require_digit && !password.chars().any(|c| c.is_ascii_digit()) {
        return false;
    }

    if requirements.require_special && !password.chars().any(|c| !c.is_alphanumeric()) {
        return false;
    }

    true
}

/// Securely hash a password and zeroize the original
pub fn hash_password_secure(plain: &mut String) -> anyhow::Result<String> {
    let hash = hash_password(plain)?;
    plain.zeroize();
    Ok(hash)
}
