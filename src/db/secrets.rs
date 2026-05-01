//! Org-scoped secrets table CRUD with ChaCha20-Poly1305 encryption-at-rest.
//!
//! Plaintext values exist only in three places: the request body when a secret
//! is created, the engine's HTTP path when a value is being attached to an
//! outbound request, and briefly on the stack during decrypt. They are never
//! logged, never returned via the API, and never serialized into BPMN.

use chacha20poly1305::aead::{Aead, KeyInit, OsRng};
use chacha20poly1305::{AeadCore, ChaCha20Poly1305, Key, Nonce};
use sqlx::PgPool;
use uuid::Uuid;

use crate::db::models::{SecretMetadata, SecretRow};
use crate::error::{EngineError, Result};

pub async fn create(
    pool: &PgPool,
    key: &[u8; 32],
    org_id: Uuid,
    name: &str,
    plaintext: &str,
) -> Result<SecretMetadata> {
    let (ciphertext, nonce) = encrypt(key, plaintext.as_bytes())?;
    let row = sqlx::query_as::<_, SecretRow>(
        "INSERT INTO secrets (org_id, name, value_encrypted, nonce) \
         VALUES ($1, $2, $3, $4) RETURNING *",
    )
    .bind(org_id)
    .bind(name)
    .bind(&ciphertext)
    .bind(&nonce)
    .fetch_one(pool)
    .await?;
    Ok(row.into())
}

pub async fn list(pool: &PgPool, org_id: Uuid) -> Result<Vec<SecretMetadata>> {
    let rows =
        sqlx::query_as::<_, SecretRow>("SELECT * FROM secrets WHERE org_id = $1 ORDER BY name")
            .bind(org_id)
            .fetch_all(pool)
            .await?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub async fn get_metadata(
    pool: &PgPool,
    org_id: Uuid,
    name: &str,
) -> Result<Option<SecretMetadata>> {
    let row =
        sqlx::query_as::<_, SecretRow>("SELECT * FROM secrets WHERE org_id = $1 AND name = $2")
            .bind(org_id)
            .bind(name)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(Into::into))
}

/// Resolve a secret's plaintext value. Used by the engine when attaching auth
/// headers to outbound HTTP requests; not exposed via the API.
pub async fn reveal(pool: &PgPool, key: &[u8; 32], org_id: Uuid, name: &str) -> Result<String> {
    let row =
        sqlx::query_as::<_, SecretRow>("SELECT * FROM secrets WHERE org_id = $1 AND name = $2")
            .bind(org_id)
            .bind(name)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| {
                EngineError::NotFound(format!("secret '{name}' not found in org {org_id}"))
            })?;
    let bytes = decrypt(key, &row.value_encrypted, &row.nonce)?;
    String::from_utf8(bytes)
        .map_err(|_| EngineError::Internal(format!("secret '{name}' decoded to non-UTF8 bytes")))
}

pub async fn delete(pool: &PgPool, org_id: Uuid, name: &str) -> Result<()> {
    let res = sqlx::query("DELETE FROM secrets WHERE org_id = $1 AND name = $2")
        .bind(org_id)
        .bind(name)
        .execute(pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(EngineError::NotFound(format!(
            "secret '{name}' not found in org {org_id}"
        )));
    }
    Ok(())
}

fn encrypt(key: &[u8; 32], plaintext: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
    let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
    let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|e| EngineError::Internal(format!("secret encrypt failed: {e}")))?;
    Ok((ciphertext, nonce.to_vec()))
}

fn decrypt(key: &[u8; 32], ciphertext: &[u8], nonce_bytes: &[u8]) -> Result<Vec<u8>> {
    if nonce_bytes.len() != 12 {
        return Err(EngineError::Internal(
            "stored nonce has wrong length".into(),
        ));
    }
    let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
    let nonce = Nonce::from_slice(nonce_bytes);
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| EngineError::Internal(format!("secret decrypt failed: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_encrypt_decrypt() {
        let key = [0xA5u8; 32];
        let (ct, n) = encrypt(&key, b"hello world").unwrap();
        assert_ne!(ct, b"hello world");
        assert_eq!(n.len(), 12);
        let pt = decrypt(&key, &ct, &n).unwrap();
        assert_eq!(pt, b"hello world");
    }

    #[test]
    fn wrong_key_fails_to_decrypt() {
        let key = [0xA5u8; 32];
        let (ct, n) = encrypt(&key, b"secret").unwrap();
        let other = [0x99u8; 32];
        assert!(decrypt(&other, &ct, &n).is_err());
    }

    #[test]
    fn each_call_uses_unique_nonce() {
        let key = [0xA5u8; 32];
        let (ct1, n1) = encrypt(&key, b"same plaintext").unwrap();
        let (ct2, n2) = encrypt(&key, b"same plaintext").unwrap();
        assert_ne!(n1, n2);
        assert_ne!(ct1, ct2);
    }
}
