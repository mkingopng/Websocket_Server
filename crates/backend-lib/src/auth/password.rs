// ============================
// openlifter-backend-lib/src/auth/password.rs
// ============================
//! Password hashing and verification.
use scrypt::{password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng}, Scrypt};
use zeroize::Zeroize;

/// Minimum password length
pub const MIN_PASSWORD_LENGTH: usize = 10;

/// Password complexity requirements
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
    let hash = Scrypt
        .hash_password(plain.as_bytes(), &salt)?
        .to_string();
    Ok(hash)
}

/// Verify a password against a hash
pub fn verify_password(hash: &str, plain: &str) -> bool {
    let parsed_hash = match PasswordHash::new(hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    Scrypt.verify_password(plain.as_bytes(), &parsed_hash).is_ok()
}

/// Check if a password meets the complexity requirements
pub fn validate_password_strength(password: &str, requirements: &PasswordRequirements) -> bool {
    if password.len() < requirements.min_length {
        return false;
    }
    
    if requirements.require_uppercase && !password.chars().any(|c| c.is_uppercase()) {
        return false;
    }
    
    if requirements.require_lowercase && !password.chars().any(|c| c.is_lowercase()) {
        return false;
    }
    
    if requirements.require_digit && !password.chars().any(|c| c.is_digit(10)) {
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