//! Long-lived API keys for service accounts and CI.
//!
//! Wire format: `ck_<24 url-safe random bytes, base64url-encoded>`.
//! The first 8 chars after the `ck_` prefix are stored unhashed as the
//! `prefix` column — used to look up the row, after which argon2 verifies
//! the full plaintext against the stored hash.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use rand::RngCore;

use crate::error::Result;

pub const TOKEN_PREFIX: &str = "ck_";
const SECRET_BYTES: usize = 24; // → 32 url-safe chars
pub const LOOKUP_PREFIX_LEN: usize = 8;

pub struct GeneratedApiKey {
    /// The full `ck_…` plaintext. Returned to the user *once* and never stored.
    pub plaintext: String,
    /// First 8 chars of the random part. Stored in the row for lookup + display.
    pub prefix: String,
    /// argon2 hash of the full plaintext.
    pub hash: String,
}

pub fn generate() -> Result<GeneratedApiKey> {
    let mut bytes = [0u8; SECRET_BYTES];
    rand::thread_rng().fill_bytes(&mut bytes);
    let secret = URL_SAFE_NO_PAD.encode(bytes);
    let prefix = secret[..LOOKUP_PREFIX_LEN].to_string();
    let plaintext = format!("{TOKEN_PREFIX}{secret}");
    let hash = super::password::hash(&plaintext)?;
    Ok(GeneratedApiKey {
        plaintext,
        prefix,
        hash,
    })
}

/// If `token` looks like an API key, return the lookup prefix. Returns
/// `None` for tokens that aren't API keys (the extractor falls through to
/// JWT) or for malformed keys (rejected later as `U401`).
pub fn extract_prefix(token: &str) -> Option<String> {
    let secret = token.strip_prefix(TOKEN_PREFIX)?;
    if secret.len() < LOOKUP_PREFIX_LEN {
        return None;
    }
    Some(secret[..LOOKUP_PREFIX_LEN].to_string())
}

pub fn verify(token: &str, hash: &str) -> bool {
    super::password::verify(token, hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_key_has_expected_shape() {
        let k = generate().unwrap();
        assert!(k.plaintext.starts_with("ck_"));
        assert_eq!(k.prefix.len(), LOOKUP_PREFIX_LEN);
        assert!(k.plaintext.contains(&k.prefix));
        assert!(verify(&k.plaintext, &k.hash));
    }

    #[test]
    fn generated_keys_are_unique() {
        let a = generate().unwrap();
        let b = generate().unwrap();
        assert_ne!(a.plaintext, b.plaintext);
        assert_ne!(a.prefix, b.prefix);
    }

    #[test]
    fn extract_prefix_round_trip() {
        let k = generate().unwrap();
        assert_eq!(extract_prefix(&k.plaintext), Some(k.prefix));
    }

    #[test]
    fn extract_prefix_rejects_non_api_key() {
        assert!(extract_prefix("eyJhbGciOiJIUzI1NiJ9.foo.bar").is_none());
        assert!(extract_prefix("ck_short").is_none());
        assert!(extract_prefix("").is_none());
    }

    #[test]
    fn verify_rejects_tampered_token() {
        let k = generate().unwrap();
        let tampered = format!("{}x", k.plaintext);
        assert!(!verify(&tampered, &k.hash));
    }
}
