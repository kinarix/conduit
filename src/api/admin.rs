//! Admin panel API (`/api/v1/admin/…`).
//!
//! All routes require authentication. Each route additionally requires a
//! specific permission checked via `principal.require(Permission::…)`.

use axum::{
    extract::State,
    http::StatusCode,
    routing::{delete, get, patch, put},
    Router,
};
use chrono::DateTime;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use super::extractors::{Json, Path};
use crate::auth::{Permission, Principal};
use crate::db;
use crate::db::org_auth_config::UpsertAuthConfig;
use crate::db::roles::RoleWithPermissions;
use crate::error::{EngineError, Result};
use crate::state::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/admin/org", get(get_org).patch(patch_org))
        .route(
            "/api/v1/admin/auth-config",
            get(get_auth_config).patch(patch_auth_config),
        )
        .route("/api/v1/admin/users", get(list_users).post(create_user))
        .route("/api/v1/admin/users/{id}", delete(remove_user))
        .route("/api/v1/admin/users/{id}/roles", put(set_user_roles))
        .route(
            "/api/v1/admin/roles",
            get(list_roles).post(create_role),
        )
        .route(
            "/api/v1/admin/roles/{id}",
            patch(update_role).delete(remove_role),
        )
}

// ─── Org ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct OrgResponse {
    id: Uuid,
    name: String,
    slug: String,
    setup_completed: bool,
    created_at: DateTime<chrono::Utc>,
}

async fn get_org(
    State(state): State<Arc<AppState>>,
    principal: Principal,
) -> Result<Json<OrgResponse>> {
    principal.require(Permission::OrgManage)?;
    let org = db::orgs::get_by_id(&state.pool, principal.org_id)
        .await?
        .ok_or_else(|| EngineError::NotFound(format!("org {}", principal.org_id)))?;
    Ok(Json(OrgResponse {
        id: org.id,
        name: org.name,
        slug: org.slug,
        setup_completed: org.setup_completed,
        created_at: org.created_at,
    }))
}

#[derive(Debug, Deserialize)]
struct PatchOrgRequest {
    name: Option<String>,
    setup_completed: Option<bool>,
}

async fn patch_org(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Json(req): Json<PatchOrgRequest>,
) -> Result<Json<OrgResponse>> {
    principal.require(Permission::OrgManage)?;

    let mut org = db::orgs::get_by_id(&state.pool, principal.org_id)
        .await?
        .ok_or_else(|| EngineError::NotFound(format!("org {}", principal.org_id)))?;

    if let Some(name) = req.name.as_deref() {
        let name = name.trim();
        if name.is_empty() {
            return Err(EngineError::Validation("name cannot be empty".into()));
        }
        org = db::orgs::update_name(&state.pool, principal.org_id, name).await?;
    }
    if let Some(completed) = req.setup_completed {
        db::orgs::set_setup_completed(&state.pool, principal.org_id, completed).await?;
        org.setup_completed = completed;
    }

    Ok(Json(OrgResponse {
        id: org.id,
        name: org.name,
        slug: org.slug,
        setup_completed: org.setup_completed,
        created_at: org.created_at,
    }))
}

// ─── Auth config ─────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct AuthConfigResponse {
    provider: String,
    oidc_issuer: Option<String>,
    oidc_client_id: Option<String>,
    /// Never returned — presence-only indicator.
    oidc_client_secret_set: bool,
    oidc_redirect_uri: Option<String>,
}

async fn get_auth_config(
    State(state): State<Arc<AppState>>,
    principal: Principal,
) -> Result<Json<AuthConfigResponse>> {
    principal.require(Permission::OrgManage)?;

    // Check for stored encrypted secret by querying the raw column
    let row = sqlx::query!(
        "SELECT provider, oidc_issuer, oidc_client_id, oidc_client_secret_enc, oidc_redirect_uri \
         FROM org_auth_config WHERE org_id = $1",
        principal.org_id
    )
    .fetch_optional(&state.pool)
    .await?;

    if let Some(r) = row {
        Ok(Json(AuthConfigResponse {
            provider: r.provider,
            oidc_issuer: r.oidc_issuer,
            oidc_client_id: r.oidc_client_id,
            oidc_client_secret_set: r.oidc_client_secret_enc.is_some(),
            oidc_redirect_uri: r.oidc_redirect_uri,
        }))
    } else {
        Ok(Json(AuthConfigResponse {
            provider: "internal".into(),
            oidc_issuer: None,
            oidc_client_id: None,
            oidc_client_secret_set: false,
            oidc_redirect_uri: None,
        }))
    }
}

#[derive(Debug, Deserialize)]
struct PatchAuthConfigRequest {
    provider: String,
    oidc_issuer: Option<String>,
    oidc_client_id: Option<String>,
    oidc_client_secret: Option<String>,
    oidc_redirect_uri: Option<String>,
}

async fn patch_auth_config(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Json(req): Json<PatchAuthConfigRequest>,
) -> Result<Json<AuthConfigResponse>> {
    principal.require(Permission::OrgManage)?;

    if req.provider != "internal" && req.provider != "oidc" {
        return Err(EngineError::Validation(
            "provider must be 'internal' or 'oidc'".into(),
        ));
    }

    let config = UpsertAuthConfig {
        provider: &req.provider,
        oidc_issuer: req.oidc_issuer.as_deref(),
        oidc_client_id: req.oidc_client_id.as_deref(),
        oidc_client_secret: req.oidc_client_secret.as_deref(),
        oidc_redirect_uri: req.oidc_redirect_uri.as_deref(),
    };

    db::org_auth_config::upsert(&state.pool, &state.secrets_key, principal.org_id, config).await?;

    // Re-fetch to check if a secret is set
    let row = sqlx::query!(
        "SELECT oidc_client_secret_enc FROM org_auth_config WHERE org_id = $1",
        principal.org_id
    )
    .fetch_optional(&state.pool)
    .await?;
    let secret_set = row.and_then(|r| r.oidc_client_secret_enc).is_some();

    Ok(Json(AuthConfigResponse {
        provider: req.provider,
        oidc_issuer: req.oidc_issuer,
        oidc_client_id: req.oidc_client_id,
        oidc_client_secret_set: secret_set,
        oidc_redirect_uri: req.oidc_redirect_uri,
    }))
}

// ─── Users ───────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct AdminUserRow {
    id: Uuid,
    email: String,
    auth_provider: String,
    created_at: DateTime<chrono::Utc>,
    roles: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ListUsersQuery {
    /// Optional override of the org to list. Requires `org.create` (platform
    /// admin) when it differs from the caller's home org.
    org_id: Option<Uuid>,
}

async fn list_users(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    axum::extract::Query(q): axum::extract::Query<ListUsersQuery>,
) -> Result<Json<Vec<AdminUserRow>>> {
    // Platform admin may list users in any org; org-scoped callers need
    // user.manage and only see their own org.
    let target_org = match q.org_id {
        Some(id) if id != principal.org_id => {
            principal.require(Permission::OrgCreate)?;
            id
        }
        _ => {
            principal.require(Permission::UserManage)?;
            principal.org_id
        }
    };

    let users = db::users::list_by_org(&state.pool, target_org).await?;
    let mut result = Vec::with_capacity(users.len());
    for u in users {
        let user_roles = db::roles::list_user_roles(&state.pool, u.id).await?;
        result.push(AdminUserRow {
            id: u.id,
            email: u.email,
            auth_provider: u.auth_provider,
            created_at: u.created_at,
            roles: user_roles.into_iter().map(|r| r.role_name).collect(),
        });
    }
    Ok(Json(result))
}

#[derive(Debug, Deserialize)]
struct CreateUserRequest {
    email: String,
    auth_provider: String,
    /// Required when `auth_provider == "internal"`.
    password: Option<String>,
    /// Required when `auth_provider == "external"`.
    external_id: Option<String>,
    /// Optional initial role assignment; applied via `set_user_roles`.
    #[serde(default)]
    role_ids: Vec<Uuid>,
    /// Optional target org. Defaults to the caller's home org. When set to
    /// a different org, the caller must hold `org.create` (platform admin).
    org_id: Option<Uuid>,
}

async fn create_user(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Json(req): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<AdminUserRow>)> {
    let target_org = match req.org_id {
        Some(id) if id != principal.org_id => {
            principal.require(Permission::OrgCreate)?;
            id
        }
        _ => {
            principal.require(Permission::UserManage)?;
            principal.org_id
        }
    };

    let email = req.email.trim();
    if email.is_empty() {
        return Err(EngineError::Validation("email cannot be empty".into()));
    }

    let (password_hash, external_id) = match req.auth_provider.as_str() {
        "internal" => {
            let password = req
                .password
                .as_deref()
                .map(str::trim)
                .filter(|p| !p.is_empty())
                .ok_or_else(|| {
                    EngineError::Validation(
                        "password is required for internal auth provider".into(),
                    )
                })?;
            (Some(crate::auth::password::hash(password)?), None)
        }
        "external" => {
            let external_id = req
                .external_id
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| {
                    EngineError::Validation(
                        "external_id is required for external auth provider".into(),
                    )
                })?;
            (None, Some(external_id.to_string()))
        }
        other => {
            return Err(EngineError::Validation(format!(
                "auth_provider must be 'internal' or 'external', got '{other}'"
            )));
        }
    };

    let user = db::users::insert(
        &state.pool,
        target_org,
        &req.auth_provider,
        external_id.as_deref(),
        email,
        password_hash.as_deref(),
    )
    .await?;

    if !req.role_ids.is_empty() {
        db::roles::set_user_roles(&state.pool, user.id, target_org, &req.role_ids).await?;
    }

    let user_roles = db::roles::list_user_roles(&state.pool, user.id).await?;
    Ok((
        StatusCode::CREATED,
        Json(AdminUserRow {
            id: user.id,
            email: user.email,
            auth_provider: user.auth_provider,
            created_at: user.created_at,
            roles: user_roles.into_iter().map(|r| r.role_name).collect(),
        }),
    ))
}

async fn remove_user(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(user_id): Path<Uuid>,
) -> Result<StatusCode> {
    principal.require(Permission::UserManage)?;

    if user_id == principal.user_id {
        return Err(EngineError::Validation(
            "cannot remove yourself".into(),
        ));
    }

    let removed = db::users::remove_from_org(&state.pool, user_id, principal.org_id).await?;
    if !removed {
        return Err(EngineError::NotFound(format!("user {user_id}")));
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
struct SetUserRolesRequest {
    role_ids: Vec<Uuid>,
}

async fn set_user_roles(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(user_id): Path<Uuid>,
    Json(req): Json<SetUserRolesRequest>,
) -> Result<StatusCode> {
    principal.require(Permission::RoleManage)?;

    // Confirm target user is in the same org
    let target = db::users::find_by_id(&state.pool, user_id)
        .await?
        .ok_or_else(|| EngineError::NotFound(format!("user {user_id}")))?;
    if target.org_id != principal.org_id {
        return Err(EngineError::NotFound(format!("user {user_id}")));
    }

    db::roles::set_user_roles(&state.pool, user_id, principal.org_id, &req.role_ids).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ─── Roles ───────────────────────────────────────────────────────────────────

async fn list_roles(
    State(state): State<Arc<AppState>>,
    principal: Principal,
) -> Result<Json<Vec<RoleWithPermissions>>> {
    let roles = db::roles::list_for_org(&state.pool, principal.org_id).await?;
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
    Json(req): Json<CreateRoleRequest>,
) -> Result<(StatusCode, Json<RoleWithPermissions>)> {
    principal.require(Permission::RoleManage)?;

    let name = req.name.trim();
    if name.is_empty() {
        return Err(EngineError::Validation("name cannot be empty".into()));
    }

    let role = db::roles::create_custom_role(
        &state.pool,
        principal.org_id,
        name,
        &req.permissions,
    )
    .await?;
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
    Path(role_id): Path<Uuid>,
    Json(req): Json<UpdateRoleRequest>,
) -> Result<Json<RoleWithPermissions>> {
    principal.require(Permission::RoleManage)?;

    let name = req.name.trim();
    if name.is_empty() {
        return Err(EngineError::Validation("name cannot be empty".into()));
    }

    let role = db::roles::update_custom_role(
        &state.pool,
        role_id,
        principal.org_id,
        name,
        &req.permissions,
    )
    .await?;
    Ok(Json(role))
}

async fn remove_role(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(role_id): Path<Uuid>,
) -> Result<StatusCode> {
    principal.require(Permission::RoleManage)?;

    let deleted = db::roles::delete_custom_role(&state.pool, role_id, principal.org_id).await?;
    if !deleted {
        return Err(EngineError::NotFound(format!(
            "role {role_id} (must be a custom org role)"
        )));
    }
    Ok(StatusCode::NO_CONTENT)
}
