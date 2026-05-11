//! Org membership endpoints. Adding/removing membership is the precondition
//! for granting org-scoped roles (see `api::role_assignments`).

use super::extractors::{Json, Path};
use axum::{
    extract::State,
    http::StatusCode,
    routing::{delete, get},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::{Permission, Principal};
use crate::db::{org_members, users};
use crate::error::{EngineError, Result};
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct MemberRow {
    pub user_id: Uuid,
    pub email: String,
    pub auth_provider: String,
    pub joined_at: chrono::DateTime<chrono::Utc>,
    pub invited_by: Option<Uuid>,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/v1/orgs/{org_id}/members",
            get(list_members).post(add_member),
        )
        .route(
            "/api/v1/orgs/{org_id}/members/{user_id}",
            delete(remove_member),
        )
}

async fn list_members(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(org_id): Path<Uuid>,
) -> Result<Json<Vec<MemberRow>>> {
    principal.require(Permission::OrgMemberRead)?;
    let members = org_members::list_by_org(&state.pool, org_id).await?;
    let mut out = Vec::with_capacity(members.len());
    for m in members {
        if let Some(u) = users::find_by_id(&state.pool, m.user_id).await? {
            out.push(MemberRow {
                user_id: u.id,
                email: u.email,
                auth_provider: u.auth_provider,
                joined_at: m.joined_at,
                invited_by: m.invited_by,
            });
        }
    }
    Ok(Json(out))
}

#[derive(Debug, Deserialize)]
struct AddMember {
    user_id: Uuid,
}

/// Add an existing global user to this org. Granting them roles is a
/// separate call to `POST /role-assignments`.
async fn add_member(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(org_id): Path<Uuid>,
    Json(req): Json<AddMember>,
) -> Result<StatusCode> {
    principal.require(Permission::OrgMemberCreate)?;
    let _ = users::find_by_id(&state.pool, req.user_id)
        .await?
        .ok_or_else(|| EngineError::NotFound(format!("user {}", req.user_id)))?;
    org_members::insert(&state.pool, req.user_id, org_id, Some(principal.user_id)).await?;
    Ok(StatusCode::CREATED)
}

async fn remove_member(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path((org_id, user_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode> {
    principal.require(Permission::OrgMemberDelete)?;
    let removed = org_members::delete(&state.pool, user_id, org_id).await?;
    if !removed {
        return Err(EngineError::NotFound(format!(
            "membership for user {user_id} in org {org_id}"
        )));
    }
    Ok(StatusCode::NO_CONTENT)
}
