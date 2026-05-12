use chacha20poly1305::aead::{Aead, KeyInit, OsRng};
use chacha20poly1305::{AeadCore, ChaCha20Poly1305, Key};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{EngineError, Result};

pub struct UpsertNotificationConfig<'a> {
    pub provider: &'a str,
    pub from_email: Option<&'a str>,
    pub from_name: Option<&'a str>,
    /// Plaintext SendGrid API key. `None` = leave the existing stored
    /// value untouched.
    pub sendgrid_api_key: Option<&'a str>,
    pub smtp_host: Option<&'a str>,
    pub smtp_port: Option<i32>,
    pub smtp_username: Option<&'a str>,
    /// Plaintext SMTP password. `None` = leave existing untouched.
    pub smtp_password: Option<&'a str>,
    pub smtp_use_tls: bool,
}

/// Upsert the notification config for `org_id`. Either secret can be
/// independently rotated; passing `None` for either leaves that column
/// alone via a `COALESCE`-style overlay.
pub async fn upsert(
    pool: &PgPool,
    key: &[u8; 32],
    org_id: Uuid,
    config: UpsertNotificationConfig<'_>,
) -> Result<()> {
    let sendgrid_enc = config
        .sendgrid_api_key
        .map(|s| encrypt(key, s.as_bytes()))
        .transpose()?;
    let smtp_pw_enc = config
        .smtp_password
        .map(|s| encrypt(key, s.as_bytes()))
        .transpose()?;

    // `COALESCE(EXCLUDED.x, target.x)` semantics for the two ciphertext
    // columns lets the caller rotate them independently; everything else
    // is a hard overwrite.
    sqlx::query(
        r#"
        INSERT INTO org_notification_config
            (org_id, provider, from_email, from_name,
             sendgrid_api_key_enc,
             smtp_host, smtp_port, smtp_username, smtp_password_enc,
             smtp_use_tls)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        ON CONFLICT (org_id) DO UPDATE SET
            provider             = EXCLUDED.provider,
            from_email           = EXCLUDED.from_email,
            from_name            = EXCLUDED.from_name,
            sendgrid_api_key_enc = COALESCE(EXCLUDED.sendgrid_api_key_enc,
                                            org_notification_config.sendgrid_api_key_enc),
            smtp_host            = EXCLUDED.smtp_host,
            smtp_port            = EXCLUDED.smtp_port,
            smtp_username        = EXCLUDED.smtp_username,
            smtp_password_enc    = COALESCE(EXCLUDED.smtp_password_enc,
                                            org_notification_config.smtp_password_enc),
            smtp_use_tls         = EXCLUDED.smtp_use_tls,
            updated_at           = NOW()
        "#,
    )
    .bind(org_id)
    .bind(config.provider)
    .bind(config.from_email)
    .bind(config.from_name)
    .bind(sendgrid_enc)
    .bind(config.smtp_host)
    .bind(config.smtp_port)
    .bind(config.smtp_username)
    .bind(smtp_pw_enc)
    .bind(config.smtp_use_tls)
    .execute(pool)
    .await?;
    Ok(())
}

/// Concatenates `nonce (12 bytes) || ciphertext` into a single blob.
/// Identical to the helper in `org_auth_config` — duplicated to keep the
/// two modules independent; if they drift, hoist into `crate::auth`.
fn encrypt(key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>> {
    let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
    let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|e| EngineError::Internal(format!("notification secret encrypt failed: {e}")))?;
    let mut out = nonce.to_vec();
    out.extend(ciphertext);
    Ok(out)
}
