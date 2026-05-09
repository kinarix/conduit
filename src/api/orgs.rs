use super::extractors::{Json, Path, Query};
use super::pagination::{with_total, Page};
use axum::{
    extract::State,
    http::StatusCode,
    routing::{delete, get, post},
    Router,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::Principal;
use crate::db::models::Org;
use crate::db::orgs;
use crate::error::{EngineError, Result};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct CreateOrgRequest {
    pub name: String,
    pub slug: String,
}

#[derive(Debug, Deserialize)]
pub struct ListOrgsQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/orgs", get(list_orgs))
        .route("/api/v1/orgs", post(create_org))
        .route("/api/v1/orgs/{id}", delete(delete_org))
}

// Phase 22: orgs endpoints require authentication and are scoped to the
// caller's home org. `list_orgs` returns only the caller's org so a user
// can't enumerate other tenants. `create_org` is reserved for Phase 23
// (RBAC) — until then, the bootstrap-admin env vars are the only path
// for org creation. `delete_org` is gated to the caller's own org.
#[tracing::instrument(skip_all)]
async fn list_orgs(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Query(params): Query<ListOrgsQuery>,
) -> Result<axum::response::Response> {
    let _ = (params.limit, params.offset); // single-row response, no pagination
    let _ = Page::from_query(params.limit, params.offset);
    match orgs::get_by_id(&state.pool, principal.org_id).await? {
        Some(org) => Ok(with_total(vec![org], 1)),
        None => Ok(with_total::<Vec<Org>>(vec![], 0)),
    }
}

// Reserved for Phase 23 RBAC (`org.manage` permission). Until then, no
// authenticated user can mint new orgs from the API — only the
// bootstrap-admin env vars create one on first boot.
#[tracing::instrument(skip_all, fields(slug = %req.slug))]
async fn create_org(
    State(_state): State<Arc<AppState>>,
    _principal: Principal,
    Json(req): Json<CreateOrgRequest>,
) -> Result<(StatusCode, Json<Org>)> {
    let _ = req;
    Err(EngineError::Forbidden(
        "creating orgs requires the org.manage permission (Phase 23)".to_string(),
    ))
}

// Soft-deleting your own org is an irreversible self-destruct; gating to
// `id == principal.org_id` prevents one tenant from nuking another by
// guessing or harvesting org UUIDs. Mismatch returns 404 so the endpoint
// doesn't confirm whether the org exists in another tenancy.
#[tracing::instrument(skip_all, fields(id = %id))]
async fn delete_org(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    if id != principal.org_id {
        return Err(EngineError::NotFound(format!("org {id}")));
    }
    orgs::delete(&state.pool, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
