use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::{delete, get, patch, post},
    Router,
};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use super::extractors::{Json, Path, Query};
use super::pagination::{with_total, Page};
use crate::db::decision_definitions;
use crate::error::{EngineError, Result};
use crate::state::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/decisions", post(deploy_decisions))
        .route("/api/v1/decisions", get(list_decisions))
        .route("/api/v1/decisions/by-key", patch(rename_by_key))
        .route("/api/v1/decisions/test", post(test_decision))
        .route("/api/v1/decisions/{key}", get(get_decision))
        .route("/api/v1/decisions/{key}", delete(delete_decision))
}

#[derive(Debug, Deserialize)]
struct DeployDecisionsQuery {
    process_group_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
struct ListDecisionsQuery {
    process_group_id: Option<Uuid>,
    #[serde(default)]
    all_versions: bool,
    limit: Option<i64>,
    offset: Option<i64>,
}

/// POST /api/v1/decisions
/// Body: raw DMN XML
/// Header: X-Org-Id: <uuid>
/// Query: process_group_id=<uuid>  (optional)
#[tracing::instrument(skip_all, fields(process_group_id = ?q.process_group_id, body_bytes = body.len()))]
async fn deploy_decisions(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(q): Query<DeployDecisionsQuery>,
    body: String,
) -> Result<(StatusCode, Json<serde_json::Value>)> {
    let org_id = extract_org_id(&headers)?;

    if let Some(group_id) = q.process_group_id {
        ensure_process_group_in_org(&state.pool, group_id, org_id).await?;
    }

    let tables = crate::dmn::parse(&body)?;

    let mut deployed = Vec::new();
    for table in &tables {
        let def = decision_definitions::deploy(
            &state.pool,
            org_id,
            q.process_group_id,
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
            "process_group_id": def.process_group_id,
        }));
    }

    Ok((StatusCode::CREATED, Json(json!({ "deployed": deployed }))))
}

/// GET /api/v1/decisions
/// Header: X-Org-Id: <uuid>
/// Query: process_group_id=<uuid>  (optional — filters to that group)
/// Query: all_versions=true        (optional — return every version, default keeps latest only)
/// Query: limit, offset            (optional — defaults: 100, 0; X-Total-Count returned)
#[tracing::instrument(skip_all, fields(process_group_id = ?q.process_group_id, all_versions = q.all_versions))]
async fn list_decisions(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(q): Query<ListDecisionsQuery>,
) -> Result<axum::response::Response> {
    let org_id = extract_org_id(&headers)?;
    let page = Page::from_query(q.limit, q.offset);
    let (defs, total) = decision_definitions::list_paginated(
        &state.pool,
        org_id,
        q.process_group_id,
        q.all_versions,
        page.limit,
        page.offset,
    )
    .await?;

    let list: Vec<serde_json::Value> = defs
        .iter()
        .map(|d| {
            json!({
                "id": d.id,
                "decision_key": d.decision_key,
                "version": d.version,
                "name": d.name,
                "deployed_at": d.deployed_at,
                "process_group_id": d.process_group_id,
            })
        })
        .collect();

    Ok(with_total(list, total))
}

/// GET /api/v1/decisions/:key
/// Returns the latest version of a decision with its parsed table structure.
/// Header: X-Org-Id: <uuid>
#[tracing::instrument(skip_all, fields(key = %key))]
async fn get_decision(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(key): Path<String>,
) -> Result<Json<serde_json::Value>> {
    let org_id = extract_org_id(&headers)?;
    let def = decision_definitions::get_latest(&state.pool, org_id, &key).await?;

    let tables = crate::dmn::parse(&def.dmn_xml)?;
    let table = tables
        .into_iter()
        .find(|t| t.decision_key == key)
        .ok_or_else(|| {
            EngineError::DmnNotFound(format!("Decision '{key}' not found in stored DMN"))
        })?;

    Ok(Json(json!({
        "id": def.id,
        "decision_key": def.decision_key,
        "version": def.version,
        "name": def.name,
        "deployed_at": def.deployed_at,
        "process_group_id": def.process_group_id,
        "table": table,
    })))
}

/// POST /api/v1/decisions/test
/// Body: { "dmn_xml": "...", "context": { "key": value } }
/// Evaluates the first decision table in the supplied XML against the given context.
/// Always returns 200; "error" key indicates a soft evaluation failure (no match, etc.).
#[derive(serde::Deserialize)]
struct TestDecisionBody {
    dmn_xml: String,
    context: HashMap<String, serde_json::Value>,
}

#[tracing::instrument(skip_all, fields(dmn_bytes = req.dmn_xml.len()))]
async fn test_decision(Json(req): Json<TestDecisionBody>) -> Result<Json<serde_json::Value>> {
    let tables = crate::dmn::parse(&req.dmn_xml)?;
    let table = tables
        .into_iter()
        .next()
        .ok_or_else(|| EngineError::Validation("No decision table found in DMN".to_string()))?;

    match crate::dmn::evaluate(&table, &req.context) {
        Ok(output) => Ok(Json(json!({ "output": output }))),
        Err(EngineError::DmnNoMatch) => Ok(Json(
            json!({ "error": "NO_MATCH", "message": "No rule matched the input values" }),
        )),
        Err(EngineError::DmnMultipleMatches) => Ok(Json(
            json!({ "error": "MULTIPLE_MATCHES", "message": "Multiple rules matched (UNIQUE hit policy requires exactly one)" }),
        )),
        Err(e) => Err(e),
    }
}

/// DELETE /api/v1/decisions/:key
/// Deletes all versions of a decision. Returns 409 if referenced by another decision or process.
#[tracing::instrument(skip_all, fields(key = %key))]
async fn delete_decision(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(key): Path<String>,
) -> Result<StatusCode> {
    let org_id = extract_org_id(&headers)?;
    decision_definitions::delete(&state.pool, org_id, &key).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct RenameDecisionRequest {
    decision_key: String,
    name: String,
}

#[tracing::instrument(skip_all, fields(decision_key = %req.decision_key))]
async fn rename_by_key(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<RenameDecisionRequest>,
) -> Result<StatusCode> {
    let org_id = extract_org_id(&headers)?;
    let def = decision_definitions::get_latest(&state.pool, org_id, &req.decision_key).await?;
    decision_definitions::rename_all_versions(
        &state.pool,
        org_id,
        def.process_group_id,
        &req.decision_key,
        req.name.trim(),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
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

async fn ensure_process_group_in_org(
    pool: &sqlx::PgPool,
    process_group_id: Uuid,
    org_id: Uuid,
) -> Result<()> {
    let row: Option<(Uuid,)> = sqlx::query_as("SELECT org_id FROM process_groups WHERE id = $1")
        .bind(process_group_id)
        .fetch_optional(pool)
        .await?;
    match row {
        None => Err(EngineError::Validation(format!(
            "Process group {process_group_id} does not exist"
        ))),
        Some((found_org,)) if found_org != org_id => Err(EngineError::Validation(format!(
            "Process group {process_group_id} does not belong to org {org_id}"
        ))),
        Some(_) => Ok(()),
    }
}
