//! Test-side auth helpers. The signing key here MUST match the key
//! `spawn_test_app` configures on `AppState`.

use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use uuid::Uuid;

use conduit::auth::{jwt::Claims, JwtKeys};

pub const TEST_JWT_KEY: &str = "test-jwt-signing-key";
pub const TEST_JWT_ISSUER: &str = "conduit";

/// Mint a JWT for an existing `(user_id, org_id)` pair. The token uses
/// the same signing key + issuer as the test app's `AppState`.
pub fn mint_jwt(user_id: Uuid, org_id: Uuid) -> String {
    let claims = Claims::new(
        user_id,
        org_id,
        chrono::Duration::seconds(3600),
        TEST_JWT_ISSUER,
    );
    let keys = JwtKeys::from_secret(TEST_JWT_KEY);
    conduit::auth::jwt::encode_token(&claims, &keys).expect("mint_jwt")
}

/// A reqwest client whose default headers carry the Authorization Bearer
/// token. Replace `reqwest::Client::new()` with this in tests that need
/// to call protected endpoints.
pub fn authed_client(token: &str) -> reqwest::Client {
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {token}")).expect("valid bearer header"),
    );
    reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .expect("authed reqwest client")
}
