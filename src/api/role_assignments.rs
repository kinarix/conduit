//! Role assignment grants.
//!
//! `/api/v1/orgs/{org_id}/role-assignments`        — grants inside one org.
//! `/api/v1/admin/global-role-assignments`         — platform-wide grants.

use super::extractors::{Json, Path};
use axum::{
    extract::State,
    http::StatusCode,
    routing::{delete, get},
    Router,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::{Permission, Principal};
use crate::db::role_assignments::{self, GlobalRoleAssignment, OrgRoleAssignment};
use crate::error::{EngineError, Result};
use crate::state::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/v1/orgs/{org_id}/role-assignments",
            get(list_org_assignments).post(grant_org_assignment),
        )
        .route(
            "/api/v1/orgs/{org_id}/role-assignments/{id}",
            delete(revoke_org_assignment),
        )
        .route(
            "/api/v1/admin/global-role-assignments",
            get(list_global_assignments).post(grant_global_assignment),
        )
        .route(
            "/api/v1/admin/global-role-assignments/{id}",
            delete(revoke_global_assignment),
        )
}

// ─── Org-scoped ──────────────────────────────────────────────────────────────

async fn list_org_assignments(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(org_id): Path<Uuid>,
) -> Result<Json<Vec<OrgRoleAssignment>>> {
    principal.require(Permission::RoleAssignmentRead)?;
    let rows = role_assignments::list_for_org(&state.pool, org_id).await?;
    Ok(Json(rows))
}

#[derive(Debug, Deserialize)]
struct GrantOrgAssignment {
    user_id: Uuid,
    role_id: Uuid,
}

async fn grant_org_assignment(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(org_id): Path<Uuid>,
    Json(req): Json<GrantOrgAssignment>,
) -> Result<(StatusCode, Json<OrgRoleAssignment>)> {
    principal.require(Permission::RoleAssignmentCreate)?;
    let id = role_assignments::grant_org(
        &state.pool,
        req.user_id,
        req.role_id,
        org_id,
        Some(principal.user_id),
        Some(org_id),
    )
    .await?;
    let rows = role_assignments::list_for_user_in_org(&state.pool, req.user_id, org_id).await?;
    let row = rows
        .into_iter()
        .find(|r| r.id == id)
        .ok_or_else(|| EngineError::NotFound(format!("assignment {id}")))?;
    Ok((StatusCode::CREATED, Json(row)))
}

async fn revoke_org_assignment(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path((org_id, id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode> {
    principal.require(Permission::RoleAssignmentDelete)?;
    let deleted = role_assignments::revoke_org_by_id(&state.pool, id, org_id).await?;
    if !deleted {
        return Err(EngineError::NotFound(format!("assignment {id}")));
    }
    Ok(StatusCode::NO_CONTENT)
}

// ─── Global ──────────────────────────────────────────────────────────────────

async fn list_global_assignments(
    State(state): State<Arc<AppState>>,
    principal: Principal,
) -> Result<Json<Vec<GlobalRoleAssignment>>> {
    principal.require(Permission::RoleAssignmentRead)?;
    let rows = role_assignments::list_global(&state.pool).await?;
    Ok(Json(rows))
}

#[derive(Debug, Deserialize)]
struct GrantGlobalAssignment {
    user_id: Uuid,
    role_id: Uuid,
}

async fn grant_global_assignment(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Json(req): Json<GrantGlobalAssignment>,
) -> Result<(StatusCode, Json<GlobalRoleAssignment>)> {
    principal.require(Permission::RoleAssignmentCreate)?;
    let id = role_assignments::grant_global(
        &state.pool,
        req.user_id,
        req.role_id,
        Some(principal.user_id),
    )
    .await?;
    let rows = role_assignments::list_global_for_user(&state.pool, req.user_id).await?;
    let row = rows
        .into_iter()
        .find(|r| r.id == id)
        .ok_or_else(|| EngineError::NotFound(format!("global assignment {id}")))?;
    Ok((StatusCode::CREATED, Json(row)))
}

async fn revoke_global_assignment(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    principal.require(Permission::RoleAssignmentDelete)?;
    let deleted = role_assignments::revoke_global_by_id(&state.pool, id).await?;
    if !deleted {
        return Err(EngineError::NotFound(format!("global assignment {id}")));
    }
    Ok(StatusCode::NO_CONTENT)
}
