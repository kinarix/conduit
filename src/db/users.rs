use sqlx::PgPool;
use uuid::Uuid;

use crate::db::models::User;
use crate::error::Result;

pub async fn insert(
    pool: &PgPool,
    org_id: Uuid,
    auth_provider: &str,
    external_id: Option<&str>,
    email: &str,
) -> Result<User> {
    let row = sqlx::query_as::<_, User>(
        r#"
        INSERT INTO users (org_id, auth_provider, external_id, email)
        VALUES ($1, $2, $3, $4)
        RETURNING id, org_id, auth_provider, external_id, email, created_at
        "#,
    )
    .bind(org_id)
    .bind(auth_provider)
    .bind(external_id)
    .bind(email)
    .fetch_one(pool)
    .await?;
    Ok(row)
}
