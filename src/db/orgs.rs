use sqlx::PgPool;

use crate::db::models::Org;
use crate::error::Result;

pub async fn get_by_id(pool: &PgPool, id: uuid::Uuid) -> Result<Option<Org>> {
    let row = sqlx::query_as::<_, Org>("SELECT * FROM orgs WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn get_by_slug(pool: &PgPool, slug: &str) -> Result<Option<Org>> {
    let row = sqlx::query_as::<_, Org>("SELECT * FROM orgs WHERE slug = $1")
        .bind(slug)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

/// List real (non-system) orgs, newest first. Excludes the hidden `_platform`
/// org and any future system-flagged orgs.
pub async fn list_real(pool: &PgPool) -> Result<Vec<Org>> {
    let rows = sqlx::query_as::<_, Org>(
        "SELECT * FROM orgs WHERE is_system = FALSE ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn list_all(pool: &PgPool) -> Result<Vec<Org>> {
    let rows = sqlx::query_as::<_, Org>("SELECT * FROM orgs ORDER BY created_at DESC")
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

pub async fn list_paginated(pool: &PgPool, limit: i64, offset: i64) -> Result<(Vec<Org>, i64)> {
    let rows =
        sqlx::query_as::<_, Org>("SELECT * FROM orgs ORDER BY created_at DESC LIMIT $1 OFFSET $2")
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;
    let (total,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM orgs")
        .fetch_one(pool)
        .await?;
    Ok((rows, total))
}

// New orgs always start with `setup_completed = FALSE`. The Org Admin's first
// sign-in then drives the org / process-group / first-process wizard, which
// flips this to TRUE on completion. The hidden `_platform` org is the only
// exception and is seeded with `setup_completed = TRUE` directly by migration
// 025, never through this path.
pub async fn insert(pool: &PgPool, name: &str, slug: &str) -> Result<Org> {
    let row = sqlx::query_as::<_, Org>(
        "INSERT INTO orgs (name, slug, setup_completed) VALUES ($1, $2, FALSE) RETURNING *",
    )
    .bind(name)
    .bind(slug)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn set_setup_completed(pool: &PgPool, id: uuid::Uuid, completed: bool) -> Result<()> {
    sqlx::query!("UPDATE orgs SET setup_completed = $1 WHERE id = $2", completed, id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_name(pool: &PgPool, id: uuid::Uuid, name: &str) -> Result<Org> {
    let row = sqlx::query_as::<_, Org>(
        "UPDATE orgs SET name = $1 WHERE id = $2 RETURNING *",
    )
    .bind(name)
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| crate::error::EngineError::NotFound(format!("org {id} not found")))?;
    Ok(row)
}

pub async fn delete(pool: &PgPool, id: uuid::Uuid) -> Result<()> {
    let res = sqlx::query("DELETE FROM orgs WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(crate::error::EngineError::NotFound(format!(
            "org {id} not found"
        )));
    }
    Ok(())
}
