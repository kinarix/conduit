use super::extractors::{Json, Path};
use axum::{extract::State, routing::get, Router};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::{Permission, Principal};
use crate::error::{EngineError, Result};
use crate::{db::process_layouts, state::AppState};

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route(
        "/api/v1/orgs/{org_id}/processes/{process_key}/layout",
        get(get_layout).put(put_layout),
    )
}

#[tracing::instrument(skip_all, fields(org_id = %org_id, process_key = %process_key))]
async fn get_layout(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path((org_id, process_key)): Path<(Uuid, String)>,
) -> Result<Json<JsonValue>> {
    let pg = pg_for_process_key(&state.pool, org_id, &process_key).await?;
    match pg {
        Some(pg_id) => principal.require_in_pg(Permission::ProcessLayoutRead, pg_id)?,
        None => principal.require(Permission::ProcessLayoutRead)?,
    }
    let row = process_layouts::get(&state.pool, org_id, &process_key).await?;
    Ok(Json(row.map(|r| r.layout_data).unwrap_or_default()))
}

#[tracing::instrument(skip_all, fields(org_id = %org_id, process_key = %process_key))]
async fn put_layout(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path((org_id, process_key)): Path<(Uuid, String)>,
    Json(body): Json<JsonValue>,
) -> Result<Json<JsonValue>> {
    let pg = pg_for_process_key(&state.pool, org_id, &process_key).await?;
    match pg {
        Some(pg_id) => principal.require_in_pg(Permission::ProcessLayoutUpdate, pg_id)?,
        None => principal.require(Permission::ProcessLayoutUpdate)?,
    }
    let row = process_layouts::upsert(&state.pool, org_id, &process_key, body).await?;
    Ok(Json(row.layout_data))
}

/// Resolve the pg_id for a (org, process_key). Layouts are keyed by
/// process_key, not by definition id, so we look up the latest deployed
/// definition's pg. Returns `None` when no definition exists yet (e.g.
/// the modeller is laying out a brand-new process that hasn't been saved):
/// in that case the caller falls back to an org-level check, since
/// per-pg gating can't apply to something with no pg.
async fn pg_for_process_key(
    pool: &PgPool,
    org_id: Uuid,
    process_key: &str,
) -> Result<Option<Uuid>> {
    let row: Option<(Uuid,)> = sqlx::query_as(
        "SELECT process_group_id FROM process_definitions \
         WHERE org_id = $1 AND process_key = $2 \
         ORDER BY version DESC LIMIT 1",
    )
    .bind(org_id)
    .bind(process_key)
    .fetch_optional(pool)
    .await
    .map_err(EngineError::Database)?;
    Ok(row.map(|(pg,)| pg))
}
