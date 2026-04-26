use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::sync::Arc;
use uuid::Uuid;

use crate::db::process_definitions;
use crate::error::{EngineError, Result};
use crate::parser;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct DeployRequest {
    pub org_id: Uuid,
    pub owner_id: Option<Uuid>,
    pub key: String,
    pub name: Option<String>,
    pub bpmn_xml: String,
    pub labels: Option<JsonValue>,
}

#[derive(Debug, Serialize)]
pub struct DeployResponse {
    pub id: Uuid,
    pub key: String,
    pub version: i32,
    pub deployed_at: DateTime<Utc>,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/v1/deployments", post(deploy))
}

async fn deploy(
    State(state): State<Arc<AppState>>,
    Json(req): Json<DeployRequest>,
) -> Result<(StatusCode, Json<DeployResponse>)> {
    if req.key.trim().is_empty() {
        return Err(EngineError::Validation("key must not be empty".to_string()));
    }

    // Parse first — fail fast before touching the DB
    let graph = Arc::new(parser::parse(&req.bpmn_xml)?);

    let version = process_definitions::next_version(&state.pool, req.org_id, &req.key).await?;

    let labels = req.labels.unwrap_or_else(|| serde_json::json!({}));

    let def = process_definitions::insert(
        &state.pool,
        req.org_id,
        req.owner_id,
        &req.key,
        version,
        req.name.as_deref(),
        &req.bpmn_xml,
        &labels,
    )
    .await?;

    {
        let mut cache = state
            .process_cache
            .write()
            .map_err(|_| EngineError::Internal("process cache lock poisoned".to_string()))?;
        cache.insert(def.id, graph);
    }

    Ok((
        StatusCode::CREATED,
        Json(DeployResponse {
            id: def.id,
            key: def.process_key,
            version: def.version,
            deployed_at: def.deployed_at,
        }),
    ))
}
