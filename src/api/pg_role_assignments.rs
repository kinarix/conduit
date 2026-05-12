//! Process-group-scoped role grants. Phase 23.2.
//!
//! Routes:
//!   GET    /api/v1/orgs/{org_id}/process-groups/{pg_id}/role-assignments
//!   POST   /api/v1/orgs/{org_id}/process-groups/{pg_id}/role-assignments
//!   DELETE /api/v1/orgs/{org_id}/process-groups/{pg_id}/role-assignments/{id}
//!
//! These are managed at org scope (`role_assignment.*` permission). The
//! per-pg validation (role's perms ⊆ pg-scopable, target is a member) is
//! enforced in `db::role_assignments::grant_process_group`. The cross-org
//! safety check (pg belongs to org_id) is enforced by the BEFORE INSERT
//! trigger on `process_group_role_assignments`.

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
use crate::db::process_groups;
use crate::db::role_assignments::{self, PgRoleAssignment};
use crate::error::{EngineError, Result};
use crate::state::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/v1/orgs/{org_id}/process-groups/{pg_id}/role-assignments",
            get(list_pg_assignments).post(grant_pg_assignment),
        )
        .route(
            "/api/v1/orgs/{org_id}/process-groups/{pg_id}/role-assignments/{id}",
            delete(revoke_pg_assignment),
        )
}

async fn list_pg_assignments(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path((org_id, pg_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Vec<PgRoleAssignment>>> {
    principal.require(Permission::RoleAssignmentRead)?;
    ensure_pg_in_org(&state, pg_id, org_id).await?;
    let rows = role_assignments::list_for_process_group(&state.pool, pg_id).await?;
    Ok(Json(rows))
}

#[derive(Debug, Deserialize)]
struct GrantPgAssignment {
    user_id: Uuid,
    role_id: Uuid,
}

async fn grant_pg_assignment(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path((org_id, pg_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<GrantPgAssignment>,
) -> Result<(StatusCode, Json<PgRoleAssignment>)> {
    principal.require(Permission::RoleAssignmentCreate)?;
    ensure_pg_in_org(&state, pg_id, org_id).await?;

    let id = role_assignments::grant_process_group(
        &state.pool,
        req.user_id,
        req.role_id,
        pg_id,
        Some(principal.user_id),
    )
    .await?;

    let rows = role_assignments::list_pg_for_user_in_org(&state.pool, req.user_id, org_id).await?;
    let row = rows
        .into_iter()
        .find(|r| r.id == id)
        .ok_or_else(|| EngineError::NotFound(format!("pg assignment {id}")))?;
    Ok((StatusCode::CREATED, Json(row)))
}

async fn revoke_pg_assignment(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path((org_id, pg_id, id)): Path<(Uuid, Uuid, Uuid)>,
) -> Result<StatusCode> {
    principal.require(Permission::RoleAssignmentDelete)?;
    ensure_pg_in_org(&state, pg_id, org_id).await?;
    let deleted = role_assignments::revoke_pg_by_id(&state.pool, id, pg_id).await?;
    if !deleted {
        return Err(EngineError::NotFound(format!("pg assignment {id}")));
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Reject with NotFound if the pg doesn't exist or belongs to another org.
async fn ensure_pg_in_org(state: &Arc<AppState>, pg_id: Uuid, org_id: Uuid) -> Result<()> {
    let pg = process_groups::get_by_id(&state.pool, pg_id)
        .await?
        .ok_or_else(|| EngineError::NotFound(format!("process_group {pg_id}")))?;
    if pg.org_id != org_id {
        return Err(EngineError::NotFound(format!("process_group {pg_id}")));
    }
    Ok(())
}
