use sqlx::PgPool;
use uuid::Uuid;

use crate::db::models::ExecutionHistory;
use crate::error::Result;

pub async fn list_by_instance(pool: &PgPool, instance_id: Uuid) -> Result<Vec<ExecutionHistory>> {
    let rows = sqlx::query_as::<_, ExecutionHistory>(
        "SELECT * FROM execution_history WHERE instance_id = $1 ORDER BY entered_at ASC",
    )
    .bind(instance_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
