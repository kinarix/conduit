//! Role definitions API.
//!
//! - `GET /api/v1/roles` — list global built-in role templates (any
//!   authenticated user; useful for the UI's role picker).
//! - `/api/v1/orgs/{org_id}/roles` — CRUD for custom org-scoped roles +
//!   list (built-ins ∪ this org's custom roles).
//!
//! Role *assignments* live in `api::role_assignments` (org-scoped) and
//! `api::admin` (global).

use super::extractors::{Json, Path};
use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, patch},
    Router,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::{Permission, Principal};
use crate::db::roles::{self, RoleWithPermissions};
use crate::error::{EngineError, Result};
use crate::state::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/roles", get(list_builtin_roles))
        .route(
            "/api/v1/orgs/{org_id}/roles",
            get(list_org_roles).post(create_role),
        )
        .route(
            "/api/v1/orgs/{org_id}/roles/{role_id}",
            patch(update_role).delete(remove_role),
        )
}

async fn list_builtin_roles(
    State(state): State<Arc<AppState>>,
    _principal: Principal,
) -> Result<Json<Vec<RoleWithPermissions>>> {
    let roles = roles::list_global(&state.pool).await?;
    Ok(Json(roles))
}

async fn list_org_roles(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(org_id): Path<Uuid>,
) -> Result<Json<Vec<RoleWithPermissions>>> {
    principal.require(Permission::RoleRead)?;
    let roles = roles::list_for_org(&state.pool, org_id).await?;
    Ok(Json(roles))
}

#[derive(Debug, Deserialize)]
struct CreateRoleRequest {
    name: String,
    permissions: Vec<String>,
}

async fn create_role(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(org_id): Path<Uuid>,
    Json(req): Json<CreateRoleRequest>,
) -> Result<(StatusCode, Json<RoleWithPermissions>)> {
    principal.require(Permission::RoleCreate)?;
    let name = req.name.trim();
    if name.is_empty() {
        return Err(EngineError::Validation("name cannot be empty".into()));
    }
    let role = roles::create_custom_role(&state.pool, org_id, name, &req.permissions).await?;
    Ok((StatusCode::CREATED, Json(role)))
}

#[derive(Debug, Deserialize)]
struct UpdateRoleRequest {
    name: String,
    permissions: Vec<String>,
}

async fn update_role(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path((org_id, role_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<UpdateRoleRequest>,
) -> Result<Json<RoleWithPermissions>> {
    principal.require(Permission::RoleUpdate)?;
    let name = req.name.trim();
    if name.is_empty() {
        return Err(EngineError::Validation("name cannot be empty".into()));
    }
    let role =
        roles::update_custom_role(&state.pool, role_id, org_id, name, &req.permissions).await?;
    Ok(Json(role))
}

async fn remove_role(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path((org_id, role_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode> {
    principal.require(Permission::RoleDelete)?;
    let deleted = roles::delete_custom_role(&state.pool, role_id, org_id).await?;
    if !deleted {
        return Err(EngineError::NotFound(format!(
            "role {role_id} (must be a custom org role)"
        )));
    }
    Ok(StatusCode::NO_CONTENT)
}
