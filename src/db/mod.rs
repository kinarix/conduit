pub mod event_subscriptions;
pub mod execution_history;
pub mod executions;
pub mod jobs;
pub mod models;
pub mod orgs;
pub mod process_definitions;
pub mod process_instances;
pub mod tasks;
pub mod users;
pub mod variables;

use sqlx::{postgres::PgPoolOptions, Executor, PgPool};
use std::time::Duration;

use crate::config::Config;

pub async fn connect(config: &Config) -> anyhow::Result<PgPool> {
    let statement_timeout_ms = config.db_statement_timeout_ms;

    let pool = PgPoolOptions::new()
        .min_connections(config.db_min_connections)
        .max_connections(config.db_max_connections)
        .acquire_timeout(Duration::from_secs(config.db_acquire_timeout_secs))
        .after_connect(move |conn, _meta| {
            Box::pin(async move {
                conn.execute(sqlx::query(&format!(
                    "SET statement_timeout = {statement_timeout_ms}"
                )))
                .await?;
                Ok(())
            })
        })
        .connect(&config.database_url)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to connect to database: {}", e))?;

    sqlx::query("SELECT 1")
        .execute(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("Database health check failed: {}", e))?;

    Ok(pool)
}
