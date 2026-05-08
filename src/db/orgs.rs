use sqlx::PgPool;

use crate::db::models::Org;
use crate::error::Result;

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

pub async fn insert(pool: &PgPool, name: &str, slug: &str) -> Result<Org> {
    let row = sqlx::query_as::<_, Org>("INSERT INTO orgs (name, slug) VALUES ($1, $2) RETURNING *")
        .bind(name)
        .bind(slug)
        .fetch_one(pool)
        .await?;
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
