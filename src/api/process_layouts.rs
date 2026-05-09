use super::extractors::{Json, Path};
use axum::{extract::State, routing::get, Router};
use serde_json::Value as JsonValue;
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::Principal;
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
    assert_caller_org(&principal, org_id)?;
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
    assert_caller_org(&principal, org_id)?;
    let row = process_layouts::upsert(&state.pool, org_id, &process_key, body).await?;
    Ok(Json(row.layout_data))
}

fn assert_caller_org(principal: &Principal, path_org: Uuid) -> Result<()> {
    if principal.org_id != path_org {
        return Err(EngineError::NotFound(format!("org {path_org}")));
    }
    Ok(())
}
