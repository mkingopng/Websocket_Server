// ============================
// crates/backend-lib/src/auth/token_generator.rs
// ============================
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
/** Secure token generation for authentication
This module provides cryptographically secure token generation
for session tokens and CSRF tokens. */
use rand::{rngs::OsRng, RngCore};

/// Default token size in bytes (32 bytes = 256 bits of entropy)
const DEFAULT_TOKEN_BYTES: usize = 32;

/** Generate a cryptographically secure random token
This uses OS-provided entropy to create a secure random token
that is suitable for session IDs and CSRF tokens.
# Returns
A base64 URL-safe encoded string without padding */
pub fn generate_secure_token() -> String {
    generate_secure_token_with_size(DEFAULT_TOKEN_BYTES)
}

/** Generate a cryptographically secure random token with specified size
# Arguments
* `bytes` - The size of the random token in bytes
# Returns
A base64 URL-safe encoded string without padding */
pub fn generate_secure_token_with_size(bytes: usize) -> String {
    let mut buffer = vec![0u8; bytes];
    OsRng.fill_bytes(&mut buffer);
    URL_SAFE_NO_PAD.encode(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_generation() {
        // Generate two tokens and verify they're different
        let token1 = generate_secure_token();
        let token2 = generate_secure_token();

        assert_ne!(token1, token2);

        // Test token length - 32 bytes of entropy encoded in base64. should be about 43-44 char
        assert!(token1.len() >= 42);

        // Test custom size
        let small_token = generate_secure_token_with_size(16);
        let large_token = generate_secure_token_with_size(64);

        assert!(small_token.len() < token1.len());
        assert!(large_token.len() > token1.len());
    }
}
