use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::JwtKeys;
use crate::error::{EngineError, Result};

/// Claims carried inside Conduit-issued JWTs. `sub` is the user ID, `org`
/// is the user's home org, both required by the principal extractor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,
    pub org: Uuid,
    pub iat: i64,
    pub exp: i64,
    pub iss: String,
}

impl Claims {
    pub fn new(user_id: Uuid, org_id: Uuid, ttl: Duration, issuer: &str) -> Self {
        let now = Utc::now();
        Self {
            sub: user_id,
            org: org_id,
            iat: now.timestamp(),
            exp: (now + ttl).timestamp(),
            iss: issuer.to_string(),
        }
    }
}

pub fn encode_token(claims: &Claims, keys: &JwtKeys) -> Result<String> {
    encode(&Header::new(Algorithm::HS256), claims, &keys.encoding)
        .map_err(|e| EngineError::Internal(format!("jwt encode failed: {e}")))
}

/// Decode + verify (signature, issuer, expiry). Returns `None` on any
/// validation failure — the caller surfaces a generic `U401`, never the
/// specific reason.
pub fn decode_token(token: &str, keys: &JwtKeys, expected_issuer: &str) -> Option<Claims> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.set_issuer(&[expected_issuer]);
    decode::<Claims>(token, &keys.decoding, &validation)
        .ok()
        .map(|d| d.claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys() -> JwtKeys {
        JwtKeys::from_secret("test-secret")
    }

    #[test]
    fn round_trips_a_token() {
        let user_id = Uuid::new_v4();
        let org_id = Uuid::new_v4();
        let claims = Claims::new(user_id, org_id, Duration::seconds(60), "conduit");
        let token = encode_token(&claims, &keys()).unwrap();
        let decoded = decode_token(&token, &keys(), "conduit").unwrap();
        assert_eq!(decoded.sub, user_id);
        assert_eq!(decoded.org, org_id);
    }

    #[test]
    fn rejects_wrong_issuer() {
        let claims = Claims::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Duration::seconds(60),
            "other",
        );
        let token = encode_token(&claims, &keys()).unwrap();
        assert!(decode_token(&token, &keys(), "conduit").is_none());
    }

    #[test]
    fn rejects_wrong_signature() {
        let claims = Claims::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Duration::seconds(60),
            "conduit",
        );
        let token = encode_token(&claims, &keys()).unwrap();
        let other = JwtKeys::from_secret("different-secret");
        assert!(decode_token(&token, &other, "conduit").is_none());
    }

    #[test]
    fn rejects_expired_token() {
        let claims = Claims::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Duration::seconds(-3600),
            "conduit",
        );
        let token = encode_token(&claims, &keys()).unwrap();
        assert!(decode_token(&token, &keys(), "conduit").is_none());
    }

    #[test]
    fn rejects_garbage_token() {
        assert!(decode_token("not.a.jwt", &keys(), "conduit").is_none());
    }
}
