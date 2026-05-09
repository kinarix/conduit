use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};

use crate::error::{EngineError, Result};

/// Hash a plaintext password with argon2id and a fresh random salt.
/// The output string carries the algorithm, parameters, salt, and digest
/// (PHC string format) so verification needs no side state.
pub fn hash(plain: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(plain.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| EngineError::Internal(format!("password hash failed: {e}")))
}

/// Constant-time verify. Any error (malformed hash, mismatch) → `false`.
/// Never returns *why* — the caller surfaces only the generic auth error.
pub fn verify(plain: &str, hash: &str) -> bool {
    let parsed = match PasswordHash::new(hash) {
        Ok(p) => p,
        Err(_) => return false,
    };
    Argon2::default()
        .verify_password(plain.as_bytes(), &parsed)
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_correct_password() {
        let h = hash("hunter2").unwrap();
        assert!(verify("hunter2", &h));
    }

    #[test]
    fn rejects_wrong_password() {
        let h = hash("hunter2").unwrap();
        assert!(!verify("Hunter2", &h));
    }

    #[test]
    fn rejects_malformed_hash() {
        assert!(!verify("anything", "not-a-real-phc-string"));
    }

    #[test]
    fn different_salts_produce_different_hashes() {
        let a = hash("same").unwrap();
        let b = hash("same").unwrap();
        assert_ne!(a, b);
        assert!(verify("same", &a));
        assert!(verify("same", &b));
    }
}
