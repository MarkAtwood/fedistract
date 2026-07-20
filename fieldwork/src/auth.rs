//! Password hashing (argon2id) and bearer token hashing (SHA-256).

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("invalid password hash format")]
    InvalidHash,
    #[error("password hash error: {0}")]
    HashFailed(String),
}

/// Hash a bearer token for storage/lookup (SHA-256, hex-encoded).
pub fn hash_token(token: &str) -> String {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(token.as_bytes());
    hex::encode(hash)
}

/// Verify a password against an argon2id hash.
pub fn verify_password(password: &str, hash: &str) -> Result<bool, AuthError> {
    use argon2::{Argon2, PasswordHash, PasswordVerifier};
    let parsed = PasswordHash::new(hash).map_err(|_| AuthError::InvalidHash)?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

/// Hash a password with argon2id and a random salt.
pub fn hash_password(password: &str) -> Result<String, AuthError> {
    use argon2::{password_hash::SaltString, Argon2, PasswordHasher};
    use rand::rngs::OsRng;
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| AuthError::HashFailed(e.to_string()))?;
    Ok(hash.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_hash_deterministic() {
        let a = hash_token("test-token");
        let b = hash_token("test-token");
        assert_eq!(a, b);
    }

    #[test]
    fn token_hash_is_hex_sha256() {
        let h = hash_token("hello");
        assert_eq!(h.len(), 64); // SHA-256 = 32 bytes = 64 hex chars
    }

    #[test]
    fn password_round_trip() {
        let hash = hash_password("hunter2").unwrap();
        assert!(verify_password("hunter2", &hash).unwrap());
        assert!(!verify_password("wrong", &hash).unwrap());
    }

    #[test]
    fn verify_rejects_invalid_hash() {
        assert!(verify_password("anything", "not-a-hash").is_err());
    }
}
