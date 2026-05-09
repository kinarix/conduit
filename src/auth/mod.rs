//! Phase 22 — authentication primitives.
//!
//! Submodules:
//!   * `jwt`       — encode/decode HS256 access tokens.
//!   * `password`  — argon2id hash + verify for user passwords.
//!   * `api_key`   — generate + verify long-lived `ck_…` tokens.
//!   * `principal` — `Principal` type carried by authenticated requests.

pub mod api_key;
pub mod bootstrap;
pub mod jwt;
pub mod password;
pub mod principal;

use crate::config::{Config, TenantIsolation};
use chrono::Duration;
use jsonwebtoken::{DecodingKey, EncodingKey};

pub use principal::{Principal, PrincipalKind};

/// Symmetric HS256 keys derived from `CONDUIT_JWT_SIGNING_KEY`. The same
/// secret is used for both signing (login) and verification (extractor).
pub struct JwtKeys {
    pub encoding: EncodingKey,
    pub decoding: DecodingKey,
}

impl JwtKeys {
    pub fn from_secret(secret: &str) -> Self {
        Self {
            encoding: EncodingKey::from_secret(secret.as_bytes()),
            decoding: DecodingKey::from_secret(secret.as_bytes()),
        }
    }
}

/// Auth-related state held inside `AppState`. Read by the `Principal`
/// extractor on every request and by `POST /auth/login` to mint tokens.
pub struct AuthSettings {
    pub jwt_keys: JwtKeys,
    pub jwt_ttl: Duration,
    pub jwt_issuer: String,
    pub tenant_isolation: TenantIsolation,
}

impl AuthSettings {
    pub fn from_config(c: &Config) -> Self {
        Self {
            jwt_keys: JwtKeys::from_secret(&c.jwt_signing_key),
            jwt_ttl: Duration::seconds(c.jwt_ttl_seconds),
            jwt_issuer: c.jwt_issuer.clone(),
            tenant_isolation: c.tenant_isolation,
        }
    }
}
