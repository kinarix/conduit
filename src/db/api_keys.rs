use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::db::models::{ApiKeyMetadata, User, UserCredentials};
use crate::error::Result;

/// A row that includes the hash + the owning user's `org_id` so the
/// extractor can resolve a Principal in one DB round-trip after the
/// prefix lookup.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ApiKeyLookup {
    pub id: Uuid,
    pub user_id: Uuid,
    pub org_id: Uuid,
    pub email: String,
    pub key_hash: String,
}

pub async fn insert(
    pool: &PgPool,
    user_id: Uuid,
    name: &str,
    prefix: &str,
    key_hash: &str,
) -> Result<ApiKeyMetadata> {
    let row = sqlx::query_as::<_, ApiKeyMetadata>(
        r#"
        INSERT INTO api_keys (user_id, name, prefix, key_hash)
        VALUES ($1, $2, $3, $4)
        RETURNING id, user_id, name, prefix, created_at, last_used_at, revoked_at
        "#,
    )
    .bind(user_id)
    .bind(name)
    .bind(prefix)
    .bind(key_hash)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// Resolve a presented API key by its prefix. Joins through `users` so
/// the extractor gets the org_id and email in one shot. Filters out
/// revoked rows. Returns `None` if no active key matches the prefix.
pub async fn lookup_by_prefix(pool: &PgPool, prefix: &str) -> Result<Option<ApiKeyLookup>> {
    let row = sqlx::query_as::<_, ApiKeyLookup>(
        r#"
        SELECT k.id, k.user_id, u.org_id, u.email, k.key_hash
        FROM api_keys k
        JOIN users u ON u.id = k.user_id
        WHERE k.prefix = $1 AND k.revoked_at IS NULL
        "#,
    )
    .bind(prefix)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn list_by_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<ApiKeyMetadata>> {
    let rows = sqlx::query_as::<_, ApiKeyMetadata>(
        r#"
        SELECT id, user_id, name, prefix, created_at, last_used_at, revoked_at
        FROM api_keys
        WHERE user_id = $1
        ORDER BY created_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Soft-revoke. Returns `true` if a row was updated, `false` if no
/// matching un-revoked key was found for `(id, user_id)`.
pub async fn revoke(pool: &PgPool, key_id: Uuid, user_id: Uuid) -> Result<bool> {
    let res = sqlx::query(
        r#"
        UPDATE api_keys
        SET revoked_at = NOW()
        WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL
        "#,
    )
    .bind(key_id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

/// Fire-and-forget update of `last_used_at`. Best-effort; failures here
/// are logged but never fail the originating request.
pub async fn touch_last_used(pool: &PgPool, key_id: Uuid) -> Result<()> {
    sqlx::query("UPDATE api_keys SET last_used_at = $1 WHERE id = $2")
        .bind(Utc::now())
        .bind(key_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Type aliases used by the principal extractor — kept here so changes to
/// the credentials shape don't ripple into the extractor file.
pub type CredentialsRow = UserCredentials;
pub type UserRow = User;
