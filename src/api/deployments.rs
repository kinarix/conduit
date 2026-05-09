use super::extractors::{Json, Path, Query};
use axum::{
    extract::State,
    http::StatusCode,
    routing::{delete, get, patch, post},
    Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::Principal;
use crate::db::models::ProcessDefinition;
use crate::db::process_definitions;
use crate::error::{EngineError, Result};
use crate::parser;
use crate::parser::FlowNodeKind;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct ListDeploymentsQuery {}

#[derive(Debug, Deserialize)]
pub struct DeployRequest {
    pub process_group_id: Uuid,
    pub owner_id: Option<Uuid>,
    pub key: String,
    pub name: Option<String>,
    pub bpmn_xml: String,
    pub labels: Option<JsonValue>,
}

#[derive(Debug, Deserialize)]
pub struct SaveDraftRequest {
    pub process_group_id: Uuid,
    pub owner_id: Option<Uuid>,
    pub key: String,
    pub name: Option<String>,
    pub bpmn_xml: String,
    pub labels: Option<JsonValue>,
}

/// Look up a process group by id and assert it belongs to the given org.
/// Returns Validation (400) if missing or mismatched.
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

#[derive(Debug, Serialize)]
pub struct DeployResponse {
    pub id: Uuid,
    pub key: String,
    pub version: i32,
    pub status: String,
    pub deployed_at: DateTime<Utc>,
    /// Non-fatal advisories surfaced during BPMN parsing — currently used
    /// for deprecation warnings (e.g. `<conduit:http>`, U010). Always
    /// present, empty when nothing was flagged.
    #[serde(default)]
    pub warnings: Vec<crate::parser::ParseWarning>,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/deployments", get(list_deployments))
        .route("/api/v1/deployments", post(deploy))
        .route("/api/v1/deployments/draft", post(save_draft))
        .route("/api/v1/deployments/draft/new", post(create_draft))
        .route("/api/v1/deployments/by-key", patch(rename_by_key))
        .route("/api/v1/deployments/{id}", get(get_deployment))
        .route("/api/v1/deployments/{id}", delete(delete_deployment))
        .route("/api/v1/deployments/{id}/promote", post(promote_draft))
        .route("/api/v1/deployments/{id}/disabled", patch(set_disabled))
}

#[tracing::instrument(skip_all, fields(org_id = %principal.org_id))]
async fn list_deployments(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Query(_params): Query<ListDeploymentsQuery>,
) -> Result<Json<Vec<ProcessDefinition>>> {
    let defs = process_definitions::list_by_org(&state.pool, principal.org_id).await?;
    Ok(Json(defs))
}

#[tracing::instrument(skip_all, fields(id = %id))]
async fn get_deployment(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> Result<Json<ProcessDefinition>> {
    let def = fetch_definition_in_org(&state, id, principal.org_id).await?;
    Ok(Json(def))
}

#[tracing::instrument(skip_all, fields(org_id = %principal.org_id, process_key = %req.key))]
async fn deploy(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Json(req): Json<DeployRequest>,
) -> Result<(StatusCode, Json<DeployResponse>)> {
    if req.key.trim().is_empty() {
        return Err(EngineError::Validation("key must not be empty".to_string()));
    }

    ensure_process_group_in_org(&state.pool, req.process_group_id, principal.org_id).await?;

    // Parse first — fail fast before touching the DB
    let graph = Arc::new(parser::parse(&req.bpmn_xml)?);
    let warnings = graph.warnings.clone();

    if graph.input_schema.is_none() {
        let gateway_first = graph
            .nodes
            .values()
            .filter(|n| {
                matches!(
                    n.kind,
                    FlowNodeKind::StartEvent
                        | FlowNodeKind::MessageStartEvent { .. }
                        | FlowNodeKind::SignalStartEvent { .. }
                )
            })
            .any(|start| {
                graph
                    .outgoing
                    .get(&start.id)
                    .into_iter()
                    .flatten()
                    .filter_map(|tid| graph.nodes.get(tid))
                    .any(|n| {
                        matches!(
                            n.kind,
                            FlowNodeKind::ExclusiveGateway { .. }
                                | FlowNodeKind::InclusiveGateway { .. }
                        )
                    })
            });
        if gateway_first {
            tracing::warn!(
                process_key = %req.key,
                "process has a gateway immediately after start but no conduit:inputSchema — \
                 missing variables will produce a runtime error instead of a clean 422"
            );
        }
    }

    let prev_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM process_definitions \
         WHERE org_id = $1 AND process_key = $2 AND status = 'deployed' \
         ORDER BY version DESC LIMIT 1",
    )
    .bind(principal.org_id)
    .bind(&req.key)
    .fetch_optional(&state.pool)
    .await
    .map_err(crate::error::EngineError::Database)?;

    let version =
        process_definitions::next_version(&state.pool, principal.org_id, &req.key).await?;

    let labels = req.labels.unwrap_or_else(|| serde_json::json!({}));

    let def = process_definitions::insert(
        &state.pool,
        principal.org_id,
        req.owner_id,
        req.process_group_id,
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

    if let Some(prev) = prev_id {
        state.engine.cancel_timer_start_jobs(prev).await?;
    }
    state.engine.schedule_timer_start_events(def.id).await?;

    for w in &warnings {
        tracing::warn!(
            process_key = %def.process_key,
            element_id = %w.element_id,
            code = %w.code,
            "{}",
            w.message
        );
    }

    Ok((
        StatusCode::CREATED,
        Json(DeployResponse {
            id: def.id,
            key: def.process_key,
            version: def.version,
            status: def.status,
            deployed_at: def.deployed_at,
            warnings,
        }),
    ))
}

#[tracing::instrument(skip_all, fields(org_id = %principal.org_id, process_key = %req.key))]
async fn save_draft(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Json(req): Json<SaveDraftRequest>,
) -> Result<(StatusCode, Json<DeployResponse>)> {
    if req.key.trim().is_empty() {
        return Err(EngineError::Validation("key must not be empty".to_string()));
    }

    ensure_process_group_in_org(&state.pool, req.process_group_id, principal.org_id).await?;

    let labels = req.labels.unwrap_or_else(|| serde_json::json!({}));

    let def = process_definitions::save_draft(
        &state.pool,
        principal.org_id,
        req.owner_id,
        req.process_group_id,
        &req.key,
        req.name.as_deref(),
        &req.bpmn_xml,
        &labels,
    )
    .await?;

    Ok((
        StatusCode::OK,
        Json(DeployResponse {
            id: def.id,
            key: def.process_key,
            version: def.version,
            status: def.status,
            deployed_at: def.deployed_at,
            warnings: vec![],
        }),
    ))
}

#[tracing::instrument(skip_all, fields(org_id = %principal.org_id, process_key = %req.key))]
async fn create_draft(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Json(req): Json<SaveDraftRequest>,
) -> Result<(StatusCode, Json<DeployResponse>)> {
    if req.key.trim().is_empty() {
        return Err(EngineError::Validation("key must not be empty".to_string()));
    }

    ensure_process_group_in_org(&state.pool, req.process_group_id, principal.org_id).await?;

    let labels = req.labels.unwrap_or_else(|| serde_json::json!({}));

    let def = process_definitions::create_draft(
        &state.pool,
        principal.org_id,
        req.owner_id,
        req.process_group_id,
        &req.key,
        req.name.as_deref(),
        &req.bpmn_xml,
        &labels,
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(DeployResponse {
            id: def.id,
            key: def.process_key,
            version: def.version,
            status: def.status,
            deployed_at: def.deployed_at,
            warnings: vec![],
        }),
    ))
}

#[derive(Debug, Deserialize)]
pub struct RenameByKeyRequest {
    pub process_group_id: Uuid,
    pub process_key: String,
    pub name: String,
}

#[tracing::instrument(skip_all, fields(org_id = %principal.org_id, process_key = %req.process_key))]
async fn rename_by_key(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Json(req): Json<RenameByKeyRequest>,
) -> Result<StatusCode> {
    ensure_process_group_in_org(&state.pool, req.process_group_id, principal.org_id).await?;
    process_definitions::rename_all_versions(
        &state.pool,
        principal.org_id,
        req.process_group_id,
        &req.process_key,
        &req.name,
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[tracing::instrument(skip_all, fields(id = %id))]
async fn delete_deployment(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    fetch_definition_in_org(&state, id, principal.org_id).await?;

    state.engine.cancel_timer_start_jobs(id).await?;
    process_definitions::delete(&state.pool, id).await?;

    {
        let mut cache = state
            .process_cache
            .write()
            .map_err(|_| EngineError::Internal("process cache lock poisoned".to_string()))?;
        cache.remove(&id);
    }

    Ok(StatusCode::NO_CONTENT)
}

#[tracing::instrument(skip_all, fields(id = %id))]
async fn promote_draft(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> Result<Json<DeployResponse>> {
    let draft = fetch_definition_in_org(&state, id, principal.org_id).await?;
    if draft.status != "draft" {
        return Err(EngineError::Validation(format!(
            "Definition {id} is not a draft"
        )));
    }

    let graph = Arc::new(parser::parse(&draft.bpmn_xml)?);

    if graph.input_schema.is_none() {
        let gateway_first = graph
            .nodes
            .values()
            .filter(|n| {
                matches!(
                    n.kind,
                    FlowNodeKind::StartEvent
                        | FlowNodeKind::MessageStartEvent { .. }
                        | FlowNodeKind::SignalStartEvent { .. }
                )
            })
            .any(|start| {
                graph
                    .outgoing
                    .get(&start.id)
                    .into_iter()
                    .flatten()
                    .filter_map(|tid| graph.nodes.get(tid))
                    .any(|n| {
                        matches!(
                            n.kind,
                            FlowNodeKind::ExclusiveGateway { .. }
                                | FlowNodeKind::InclusiveGateway { .. }
                        )
                    })
            });
        if gateway_first {
            tracing::warn!(
                process_key = %draft.process_key,
                "process has a gateway immediately after start but no conduit:inputSchema — \
                 missing variables will produce a runtime error instead of a clean 422"
            );
        }
    }

    let prev_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM process_definitions \
         WHERE org_id = $1 AND process_key = $2 AND status = 'deployed' \
         ORDER BY version DESC LIMIT 1",
    )
    .bind(draft.org_id)
    .bind(&draft.process_key)
    .fetch_optional(&state.pool)
    .await
    .map_err(crate::error::EngineError::Database)?;

    let def = process_definitions::promote_draft(&state.pool, id).await?;

    let warnings = graph.warnings.clone();

    {
        let mut cache = state
            .process_cache
            .write()
            .map_err(|_| EngineError::Internal("process cache lock poisoned".to_string()))?;
        cache.insert(def.id, graph);
    }

    if let Some(prev) = prev_id {
        state.engine.cancel_timer_start_jobs(prev).await?;
    }
    state.engine.schedule_timer_start_events(def.id).await?;

    for w in &warnings {
        tracing::warn!(
            process_key = %def.process_key,
            element_id = %w.element_id,
            code = %w.code,
            "{}",
            w.message
        );
    }

    Ok(Json(DeployResponse {
        id: def.id,
        key: def.process_key,
        version: def.version,
        status: def.status,
        deployed_at: def.deployed_at,
        warnings,
    }))
}

#[derive(Debug, Deserialize)]
struct SetDisabledRequest {
    disabled: bool,
}

#[tracing::instrument(skip_all, fields(id = %id, disabled = req.disabled))]
async fn set_disabled(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Json(req): Json<SetDisabledRequest>,
) -> Result<Json<ProcessDefinition>> {
    fetch_definition_in_org(&state, id, principal.org_id).await?;
    let def = process_definitions::set_disabled(&state.pool, id, req.disabled).await?;
    if req.disabled {
        state.engine.cancel_timer_start_jobs(def.id).await?;
    } else {
        state.engine.schedule_timer_start_events(def.id).await?;
    }
    Ok(Json(def))
}

/// Tenant-isolation guard. Returns NotFound if the definition doesn't
/// exist or belongs to another org.
async fn fetch_definition_in_org(
    state: &Arc<AppState>,
    id: Uuid,
    org_id: Uuid,
) -> Result<ProcessDefinition> {
    let def = process_definitions::get_by_id(&state.pool, id).await?;
    if def.org_id != org_id {
        return Err(EngineError::NotFound(format!("process_definition {id}")));
    }
    Ok(def)
}
