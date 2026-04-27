use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

use crate::db::decision_definitions;
use crate::error::{EngineError, Result};
use crate::state::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/decisions", post(deploy_decisions))
        .route("/api/v1/decisions", get(list_decisions))
}

/// POST /api/v1/decisions
/// Body: raw DMN XML
/// Header: X-Org-Id: <uuid>
async fn deploy_decisions(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: String,
) -> Result<(StatusCode, Json<serde_json::Value>)> {
    let org_id = extract_org_id(&headers)?;

    let tables = crate::dmn::parse(&body)?;

    let mut deployed = Vec::new();
    for table in &tables {
        let def = decision_definitions::deploy(
            &state.pool,
            org_id,
            &table.decision_key,
            table.name.as_deref(),
            &body,
        )
        .await?;

        deployed.push(json!({
            "id": def.id,
            "decision_key": def.decision_key,
            "version": def.version,
            "name": def.name,
        }));
    }

    Ok((StatusCode::CREATED, Json(json!({ "deployed": deployed }))))
}

/// GET /api/v1/decisions
/// Header: X-Org-Id: <uuid>
async fn list_decisions(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>> {
    let org_id = extract_org_id(&headers)?;
    let defs = decision_definitions::list(&state.pool, org_id).await?;

    let list: Vec<serde_json::Value> = defs
        .iter()
        .map(|d| {
            json!({
                "id": d.id,
                "decision_key": d.decision_key,
                "version": d.version,
                "name": d.name,
                "deployed_at": d.deployed_at,
            })
        })
        .collect();

    Ok(Json(json!(list)))
}

fn extract_org_id(headers: &HeaderMap) -> Result<Uuid> {
    let val = headers
        .get("x-org-id")
        .ok_or_else(|| EngineError::Validation("Missing X-Org-Id header".to_string()))?
        .to_str()
        .map_err(|_| EngineError::Validation("X-Org-Id header is not valid UTF-8".to_string()))?;

    val.parse::<Uuid>()
        .map_err(|_| EngineError::Validation(format!("X-Org-Id is not a valid UUID: {val}")))
}
