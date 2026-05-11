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

use crate::auth::{Permission, Principal};
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

// Org endpoints serve two audiences:
//
//   * Platform admins (perm `org.create`) — see all real orgs across the
//     deployment and may create new ones. The hidden `_platform` org that
//     hosts them is never returned.
//
//   * Org-scoped users — see only their own org and may delete it (with
//     `org.manage`). They cannot enumerate other tenants.
#[tracing::instrument(skip_all)]
async fn list_orgs(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Query(params): Query<ListOrgsQuery>,
) -> Result<axum::response::Response> {
    let _ = (params.limit, params.offset);
    let _ = Page::from_query(params.limit, params.offset);

    if principal.has(Permission::OrgCreate) {
        let rows = orgs::list_real(&state.pool).await?;
        let total = rows.len() as i64;
        return Ok(with_total(rows, total));
    }

    match orgs::get_by_id(&state.pool, principal.org_id).await? {
        Some(org) if !org.is_system => Ok(with_total(vec![org], 1)),
        _ => Ok(with_total::<Vec<Org>>(vec![], 0)),
    }
}

#[tracing::instrument(skip_all, fields(slug = %req.slug))]
async fn create_org(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Json(req): Json<CreateOrgRequest>,
) -> Result<(StatusCode, Json<Org>)> {
    principal.require(Permission::OrgCreate)?;
    if req.name.trim().is_empty() {
        return Err(EngineError::Validation("name must not be empty".to_string()));
    }
    let slug = req.slug.trim();
    if slug.is_empty() {
        return Err(EngineError::Validation("slug must not be empty".to_string()));
    }
    // `conduit` is the system org used to host the platform admin. Defending
    // here too (in addition to the UNIQUE constraint) gives a clean U-coded
    // validation error instead of a DB-level conflict surfacing as 500.
    if slug.eq_ignore_ascii_case("conduit") {
        return Err(EngineError::Validation(
            "the slug 'conduit' is reserved".to_string(),
        ));
    }
    let org = orgs::insert(&state.pool, req.name.trim(), slug).await?;
    Ok((StatusCode::CREATED, Json(org)))
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
    principal.require(Permission::OrgManage)?;
    if id != principal.org_id {
        return Err(EngineError::NotFound(format!("org {id}")));
    }
    orgs::delete(&state.pool, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
