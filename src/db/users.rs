use sqlx::PgPool;
use uuid::Uuid;

use crate::db::models::{User, UserCredentials};
use crate::error::Result;

pub async fn insert(
    pool: &PgPool,
    org_id: Uuid,
    auth_provider: &str,
    external_id: Option<&str>,
    email: &str,
    password_hash: Option<&str>,
) -> Result<User> {
    let row = sqlx::query_as::<_, User>(
        r#"
        INSERT INTO users (org_id, auth_provider, external_id, email, password_hash)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id, org_id, auth_provider, external_id, email, created_at
        "#,
    )
    .bind(org_id)
    .bind(auth_provider)
    .bind(external_id)
    .bind(email)
    .bind(password_hash)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// Look up the credentials row for `(org slug, email)`. Returns `None` if
/// either is unknown — the caller must NOT distinguish between "no such
/// org", "no such user", and "wrong password" in its response.
pub async fn find_credentials_by_org_slug_and_email(
    pool: &PgPool,
    org_slug: &str,
    email: &str,
) -> Result<Option<UserCredentials>> {
    let row = sqlx::query_as::<_, UserCredentials>(
        r#"
        SELECT u.id, u.org_id, u.auth_provider, u.email, u.password_hash
        FROM users u
        JOIN orgs o ON o.id = u.org_id
        WHERE o.slug = $1 AND u.email = $2
        "#,
    )
    .bind(org_slug)
    .bind(email)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Used by the `Principal` extractor to confirm a JWT's subject still
/// exists. Returns `None` if the user has been deleted since the token
/// was issued — extractor surfaces `U401`.
pub async fn find_by_id(pool: &PgPool, user_id: Uuid) -> Result<Option<User>> {
    let row = sqlx::query_as::<_, User>(
        r#"
        SELECT id, org_id, auth_provider, external_id, email, created_at
        FROM users
        WHERE id = $1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn list_by_org(pool: &PgPool, org_id: Uuid) -> Result<Vec<User>> {
    let rows = sqlx::query_as::<_, User>(
        r#"
        SELECT id, org_id, auth_provider, external_id, email, created_at
        FROM users
        WHERE org_id = $1
        ORDER BY created_at ASC
        "#,
    )
    .bind(org_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn remove_from_org(pool: &PgPool, user_id: Uuid, org_id: Uuid) -> Result<bool> {
    let res = sqlx::query!(
        "DELETE FROM users WHERE id = $1 AND org_id = $2",
        user_id,
        org_id
    )
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn count(pool: &PgPool) -> Result<i64> {
    let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await?;
    Ok(n)
}
