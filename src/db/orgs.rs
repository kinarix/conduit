use sqlx::PgPool;

use crate::db::models::Org;
use crate::error::Result;

pub async fn list_all(pool: &PgPool) -> Result<Vec<Org>> {
    let rows = sqlx::query_as::<_, Org>("SELECT * FROM orgs ORDER BY created_at DESC")
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

pub async fn insert(pool: &PgPool, name: &str, slug: &str) -> Result<Org> {
    let row = sqlx::query_as::<_, Org>("INSERT INTO orgs (name, slug) VALUES ($1, $2) RETURNING *")
        .bind(name)
        .bind(slug)
        .fetch_one(pool)
        .await?;
    Ok(row)
}
