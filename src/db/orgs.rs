use sqlx::PgPool;

use crate::db::models::Org;
use crate::error::Result;

pub async fn insert(pool: &PgPool, name: &str, slug: &str) -> Result<Org> {
    let row = sqlx::query_as::<_, Org>("INSERT INTO orgs (name, slug) VALUES ($1, $2) RETURNING *")
        .bind(name)
        .bind(slug)
        .fetch_one(pool)
        .await?;
    Ok(row)
}
