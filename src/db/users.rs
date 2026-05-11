use sqlx::PgPool;
use uuid::Uuid;

use crate::db::models::{User, UserCredentials};
use crate::error::Result;

/// Insert a new global user identity. The user is not yet a member of any
/// org — caller must follow up with `db::org_members::insert` to grant
/// membership, and `db::role_assignments::grant_*` to grant permissions.
pub async fn insert(
    pool: &PgPool,
    auth_provider: &str,
    external_id: Option<&str>,
    email: &str,
    password_hash: Option<&str>,
) -> Result<User> {
    let row = sqlx::query_as::<_, User>(
        r#"
        INSERT INTO users (auth_provider, external_id, email, password_hash)
        VALUES ($1, $2, $3, $4)
        RETURNING id, auth_provider, external_id, email, created_at
        "#,
    )
    .bind(auth_provider)
    .bind(external_id)
    .bind(email)
    .bind(password_hash)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// Look up the credentials row by email (case-insensitive). Returns `None`
/// if the email is unknown — callers MUST NOT distinguish between "no such
/// user" and "wrong password" in their response.
pub async fn find_credentials_by_email(
    pool: &PgPool,
    email: &str,
) -> Result<Option<UserCredentials>> {
    let row = sqlx::query_as::<_, UserCredentials>(
        r#"
        SELECT id, auth_provider, email, password_hash
        FROM users
        WHERE LOWER(email) = LOWER($1)
        "#,
    )
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
        SELECT id, auth_provider, external_id, email, created_at
        FROM users
        WHERE id = $1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// List all users globally. Callers needing per-org membership should join
/// against `org_members`.
pub async fn list_all(pool: &PgPool) -> Result<Vec<User>> {
    let rows = sqlx::query_as::<_, User>(
        r#"
        SELECT id, auth_provider, external_id, email, created_at
        FROM users
        ORDER BY created_at ASC
        "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// List users who are members of `org_id`.
pub async fn list_by_org(pool: &PgPool, org_id: Uuid) -> Result<Vec<User>> {
    let rows = sqlx::query_as::<_, User>(
        r#"
        SELECT u.id, u.auth_provider, u.external_id, u.email, u.created_at
        FROM users u
        JOIN org_members m ON m.user_id = u.id
        WHERE m.org_id = $1
        ORDER BY u.created_at ASC
        "#,
    )
    .bind(org_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Delete the user globally. Cascades to org_members and role assignments.
pub async fn delete(pool: &PgPool, user_id: Uuid) -> Result<bool> {
    let res = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user_id)
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
