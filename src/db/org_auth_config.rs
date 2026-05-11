use chacha20poly1305::aead::{Aead, KeyInit, OsRng};
use chacha20poly1305::{AeadCore, ChaCha20Poly1305, Key};
use sqlx::PgPool;
use uuid::Uuid;

use crate::db::models::OrgAuthConfig;
use crate::error::{EngineError, Result};

pub async fn get(pool: &PgPool, org_id: Uuid) -> Result<Option<OrgAuthConfig>> {
    let row = sqlx::query_as::<_, OrgAuthConfig>(
        r#"
        SELECT org_id, provider, oidc_issuer, oidc_client_id, oidc_redirect_uri, updated_at
        FROM org_auth_config
        WHERE org_id = $1
        "#,
    )
    .bind(org_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub struct UpsertAuthConfig<'a> {
    pub provider: &'a str,
    pub oidc_issuer: Option<&'a str>,
    pub oidc_client_id: Option<&'a str>,
    /// Plaintext secret — will be encrypted before storage. Pass `None` to
    /// leave the existing stored secret unchanged.
    pub oidc_client_secret: Option<&'a str>,
    pub oidc_redirect_uri: Option<&'a str>,
}

pub async fn upsert(
    pool: &PgPool,
    key: &[u8; 32],
    org_id: Uuid,
    config: UpsertAuthConfig<'_>,
) -> Result<OrgAuthConfig> {
    let enc = config
        .oidc_client_secret
        .map(|s| encrypt(key, s.as_bytes()))
        .transpose()?;

    // When no new secret is supplied, preserve the existing encrypted value.
    let row = if let Some(enc_bytes) = enc {
        sqlx::query_as::<_, OrgAuthConfig>(
            r#"
            INSERT INTO org_auth_config
                (org_id, provider, oidc_issuer, oidc_client_id, oidc_client_secret_enc, oidc_redirect_uri)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (org_id) DO UPDATE SET
                provider               = EXCLUDED.provider,
                oidc_issuer            = EXCLUDED.oidc_issuer,
                oidc_client_id         = EXCLUDED.oidc_client_id,
                oidc_client_secret_enc = EXCLUDED.oidc_client_secret_enc,
                oidc_redirect_uri      = EXCLUDED.oidc_redirect_uri,
                updated_at             = NOW()
            RETURNING org_id, provider, oidc_issuer, oidc_client_id, oidc_redirect_uri, updated_at
            "#,
        )
        .bind(org_id)
        .bind(config.provider)
        .bind(config.oidc_issuer)
        .bind(config.oidc_client_id)
        .bind(enc_bytes)
        .bind(config.oidc_redirect_uri)
        .fetch_one(pool)
        .await?
    } else {
        sqlx::query_as::<_, OrgAuthConfig>(
            r#"
            INSERT INTO org_auth_config
                (org_id, provider, oidc_issuer, oidc_client_id, oidc_redirect_uri)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (org_id) DO UPDATE SET
                provider          = EXCLUDED.provider,
                oidc_issuer       = EXCLUDED.oidc_issuer,
                oidc_client_id    = EXCLUDED.oidc_client_id,
                oidc_redirect_uri = EXCLUDED.oidc_redirect_uri,
                updated_at        = NOW()
            RETURNING org_id, provider, oidc_issuer, oidc_client_id, oidc_redirect_uri, updated_at
            "#,
        )
        .bind(org_id)
        .bind(config.provider)
        .bind(config.oidc_issuer)
        .bind(config.oidc_client_id)
        .bind(config.oidc_redirect_uri)
        .fetch_one(pool)
        .await?
    };
    Ok(row)
}

/// Concatenates `nonce (12 bytes) || ciphertext` into a single blob.
fn encrypt(key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>> {
    let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
    let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|e| EngineError::Internal(format!("oidc secret encrypt failed: {e}")))?;
    let mut out = nonce.to_vec(); // 12 bytes
    out.extend(ciphertext);
    Ok(out)
}
