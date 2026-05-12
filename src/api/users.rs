//! Org-scoped user management. Creating a user always lands them in the
//! org named in the path; they get an `org_members` row plus optional
//! initial role grants.
//!
//! Global (cross-org) user CRUD lives under `/api/v1/admin/users` and
//! requires the global `user.create` / `user.delete` permissions.

use super::extractors::{Json, Path};
use axum::{
    extract::State,
    http::StatusCode,
    routing::{delete, get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::{self, Permission, Principal};
use crate::db::models::User;
use crate::db::{org_members, role_assignments, users};
use crate::error::{EngineError, Result};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub auth_provider: String,
    pub external_id: Option<String>,
    pub email: String,
    pub password: Option<String>,
    /// Display name (optional). Free text.
    pub name: Option<String>,
    /// Phone number (optional). Free text; not validated here.
    pub phone: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct OrgUserResponse {
    #[serde(flatten)]
    pub user: User,
    pub joined_at: chrono::DateTime<chrono::Utc>,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/v1/orgs/{org_id}/users",
            get(list_org_users).post(create_user),
        )
        .route(
            "/api/v1/orgs/{org_id}/users/{user_id}",
            delete(remove_from_org),
        )
        .route(
            "/api/v1/orgs/{org_id}/users/{user_id}/reset-password",
            post(reset_org_user_password),
        )
}

#[derive(Debug, Deserialize)]
pub struct ResetPasswordRequest {
    pub new_password: String,
}

/// Admin-initiated password reset for a member of `org_id`. Requires
/// `user.reset_password` (held at global or org scope by PlatformAdmin /
/// OrgOwner / OrgAdmin).
///
/// Hierarchy guard: a non-global caller cannot reset the password of a
/// platform admin who happens to be a member of this org — that would let
/// an OrgAdmin take over a global account.
#[tracing::instrument(skip_all, fields(org_id = %org_id, user_id = %user_id))]
async fn reset_org_user_password(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path((org_id, user_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<ResetPasswordRequest>,
) -> Result<StatusCode> {
    principal.require(Permission::UserResetPassword)?;

    // Target must be a member of THIS org. Otherwise an OrgAdmin in Acme
    // could reset a user who only lives in another org.
    if !org_members::exists(&state.pool, user_id, org_id).await? {
        return Err(EngineError::NotFound(format!(
            "membership for user {user_id} in org {org_id}"
        )));
    }

    // Hierarchy: don't let a non-global caller demote a platform admin.
    let target_is_global = role_assignments::is_global_admin(&state.pool, user_id).await?;
    if target_is_global && !principal.is_global_admin {
        return Err(EngineError::Forbidden(
            "cannot reset password for a platform admin".into(),
        ));
    }

    let target = users::find_by_id(&state.pool, user_id)
        .await?
        .ok_or_else(|| EngineError::NotFound(format!("user {user_id}")))?;
    if target.auth_provider != "internal" {
        return Err(EngineError::Validation(
            "cannot reset password for an external-auth user".into(),
        ));
    }
    if req.new_password.len() < 8 {
        return Err(EngineError::Validation(
            "new_password must be at least 8 characters".into(),
        ));
    }
    let new_hash = auth::password::hash(&req.new_password)?;
    let updated = users::set_password_hash(&state.pool, user_id, &new_hash).await?;
    if !updated {
        return Err(EngineError::NotFound(format!("user {user_id}")));
    }
    Ok(StatusCode::NO_CONTENT)
}

#[tracing::instrument(skip_all, fields(org_id = %org_id))]
async fn list_org_users(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(org_id): Path<Uuid>,
) -> Result<Json<Vec<User>>> {
    principal.require(Permission::OrgMemberRead)?;
    let rows = users::list_by_org(&state.pool, org_id).await?;
    Ok(Json(rows))
}

/// Create a new global user and add them as a member of `org_id`. The
/// caller needs both `user.create` (to create the global identity) and
/// `org_member.create` (to grant membership in this org).
#[tracing::instrument(skip_all, fields(org_id = %org_id, auth_provider = %req.auth_provider))]
async fn create_user(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(org_id): Path<Uuid>,
    Json(req): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<User>)> {
    principal.require(Permission::UserCreate)?;
    principal.require(Permission::OrgMemberCreate)?;

    if !matches!(req.auth_provider.as_str(), "internal" | "external") {
        return Err(EngineError::Validation(
            "auth_provider must be 'internal' or 'external'".to_string(),
        ));
    }
    if req.email.trim().is_empty() {
        return Err(EngineError::Validation(
            "email must not be empty".to_string(),
        ));
    }

    let password_hash = match req.auth_provider.as_str() {
        "internal" => {
            let pw = req.password.as_deref().ok_or_else(|| {
                EngineError::Validation(
                    "password is required for internal auth_provider".to_string(),
                )
            })?;
            if pw.len() < 8 {
                return Err(EngineError::Validation(
                    "password must be at least 8 characters".to_string(),
                ));
            }
            Some(auth::password::hash(pw)?)
        }
        _ => {
            if req.password.is_some() {
                return Err(EngineError::Validation(
                    "password must not be set for external auth_provider".to_string(),
                ));
            }
            None
        }
    };
    let trimmed_name = req.name.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let trimmed_phone = req
        .phone
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let user = users::insert(
        &state.pool,
        &req.auth_provider,
        req.external_id.as_deref(),
        req.email.trim(),
        password_hash.as_deref(),
        trimmed_name,
        trimmed_phone,
    )
    .await?;

    org_members::insert(&state.pool, user.id, org_id, Some(principal.user_id)).await?;

    Ok((StatusCode::CREATED, Json(user)))
}

/// Remove a user from this org (does NOT delete their global identity).
#[tracing::instrument(skip_all, fields(org_id = %org_id, user_id = %user_id))]
async fn remove_from_org(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path((org_id, user_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode> {
    principal.require(Permission::OrgMemberDelete)?;
    if user_id == principal.user_id {
        return Err(EngineError::Validation(
            "cannot remove yourself from the org".into(),
        ));
    }
    let removed = org_members::delete(&state.pool, user_id, org_id).await?;
    if !removed {
        return Err(EngineError::NotFound(format!(
            "membership for user {user_id} in org {org_id}"
        )));
    }
    Ok(StatusCode::NO_CONTENT)
}
