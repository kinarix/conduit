use super::extractors::{Json, Path};
use axum::{extract::State, routing::get, Router};
use serde_json::Value as JsonValue;
use std::sync::Arc;
use uuid::Uuid;

use crate::{db::process_layouts, error::Result, state::AppState};

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route(
        "/api/v1/orgs/{org_id}/processes/{process_key}/layout",
        get(get_layout).put(put_layout),
    )
}

async fn get_layout(
    State(state): State<Arc<AppState>>,
    Path((org_id, process_key)): Path<(Uuid, String)>,
) -> Result<Json<JsonValue>> {
    let row = process_layouts::get(&state.pool, org_id, &process_key).await?;
    Ok(Json(row.map(|r| r.layout_data).unwrap_or_default()))
}

async fn put_layout(
    State(state): State<Arc<AppState>>,
    Path((org_id, process_key)): Path<(Uuid, String)>,
    Json(body): Json<JsonValue>,
) -> Result<Json<JsonValue>> {
    let row = process_layouts::upsert(&state.pool, org_id, &process_key, body).await?;
    Ok(Json(row.layout_data))
}
