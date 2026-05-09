use super::extractors::{Json, Path};
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
use crate::db::roles::{self, RoleWithPermissions, UserRoleRow};
use crate::db::users;
use crate::error::{EngineError, Result};
use crate::state::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/roles", get(list_roles))
        .route("/api/v1/users/{id}/roles", get(list_user_roles))
        .route("/api/v1/users/{id}/roles", post(assign_role))
        .route("/api/v1/users/{id}/roles/{role_id}", delete(revoke_role))
}

/// GET /api/v1/roles — list all global built-in roles and their permissions.
/// Any authenticated user may call this (no specific permission required).
#[tracing::instrument(skip_all)]
async fn list_roles(
    State(state): State<Arc<AppState>>,
    _principal: Principal,
) -> Result<Json<Vec<RoleWithPermissions>>> {
    let roles = roles::list_global_roles(&state.pool).await?;
    Ok(Json(roles))
}

/// GET /api/v1/users/{id}/roles — list roles assigned to a user.
/// Requires `role.manage`. The target user must belong to the caller's org.
#[tracing::instrument(skip_all, fields(user_id = %id))]
async fn list_user_roles(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<UserRoleRow>>> {
    principal.require(Permission::RoleManage)?;
    ensure_user_in_org(&state, id, principal.org_id).await?;
    let rows = roles::list_user_roles(&state.pool, id).await?;
    Ok(Json(rows))
}

#[derive(Debug, Deserialize)]
struct AssignRoleRequest {
    role_name: String,
}

/// POST /api/v1/users/{id}/roles — assign a global built-in role to a user.
/// Requires `role.manage`. The target user must belong to the caller's org.
#[tracing::instrument(skip_all, fields(user_id = %id))]
async fn assign_role(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(id): Path<Uuid>,
    Json(req): Json<AssignRoleRequest>,
) -> Result<StatusCode> {
    principal.require(Permission::RoleManage)?;
    ensure_user_in_org(&state, id, principal.org_id).await?;
    let found = roles::assign_role(&state.pool, id, &req.role_name, principal.user_id).await?;
    if !found {
        return Err(EngineError::NotFound(format!("role '{}'", req.role_name)));
    }
    Ok(StatusCode::NO_CONTENT)
}

/// DELETE /api/v1/users/{id}/roles/{role_id} — revoke a role from a user.
/// Requires `role.manage`. The target user must belong to the caller's org.
#[tracing::instrument(skip_all, fields(user_id = %id, role_id = %role_id))]
async fn revoke_role(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path((id, role_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode> {
    principal.require(Permission::RoleManage)?;
    ensure_user_in_org(&state, id, principal.org_id).await?;
    let deleted = roles::revoke_role(&state.pool, id, role_id, principal.org_id).await?;
    if !deleted {
        return Err(EngineError::NotFound(format!(
            "role assignment for user {id} / role {role_id}"
        )));
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn ensure_user_in_org(state: &Arc<AppState>, user_id: Uuid, org_id: Uuid) -> Result<()> {
    let user = users::find_by_id(&state.pool, user_id)
        .await?
        .ok_or_else(|| EngineError::NotFound(format!("user {user_id}")))?;
    if user.org_id != org_id {
        return Err(EngineError::NotFound(format!("user {user_id}")));
    }
    Ok(())
}
