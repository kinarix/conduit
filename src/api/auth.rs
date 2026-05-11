//! `POST /auth/login`, `GET /me`, and `POST/GET/DELETE /api-keys`.
//!
//! `/auth/login` is the only public endpoint here — everything else requires
//! a valid Bearer token (resolved by the `Principal` extractor).

use axum::{
    extract::State,
    http::StatusCode,
    routing::{delete, get, post},
    Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use super::extractors::{Json, Path};
use crate::auth::{self, jwt::Claims, Principal, PrincipalKind};
use crate::db;
use crate::error::{EngineError, Result};
use crate::state::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/auth/login", post(login))
        .route("/api/v1/me", get(me))
        .route("/api/v1/api-keys", post(create_api_key).get(list_api_keys))
        .route("/api/v1/api-keys/{id}", delete(revoke_api_key))
}

// ─── Login ───────────────────────────────────────────────────────────────────

/// Email is globally unique (case-insensitive) since migration 028 — no org
/// slug is needed at login time. The client chooses which org to operate in
/// after the fact via the URL it visits.
#[derive(Debug, Deserialize)]
struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Debug, Serialize)]
struct LoginResponse {
    access_token: String,
    token_type: &'static str,
    expires_in: i64,
}

#[tracing::instrument(skip_all, fields(email = %req.email))]
async fn login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>> {
    // The three failure modes — unknown user, wrong password, external-auth
    // user trying internal login — all return the same generic U011.
    let creds = db::users::find_credentials_by_email(&state.pool, &req.email)
        .await?
        .ok_or(EngineError::LoginFailed)?;

    if creds.auth_provider != "internal" {
        return Err(EngineError::LoginFailed);
    }
    let hash = creds
        .password_hash
        .as_deref()
        .ok_or(EngineError::LoginFailed)?;
    if !auth::password::verify(&req.password, hash) {
        return Err(EngineError::LoginFailed);
    }

    let claims = Claims::new(
        creds.id,
        // The token no longer pins to a specific org — set to nil_uuid.
        // The extractor scopes per-request from the URL.
        Uuid::nil(),
        state.auth.jwt_ttl,
        &state.auth.jwt_issuer,
    );
    let token = auth::jwt::encode_token(&claims, &state.auth.jwt_keys)?;
    Ok(Json(LoginResponse {
        access_token: token,
        token_type: "Bearer",
        expires_in: state.auth.jwt_ttl.num_seconds(),
    }))
}

// ─── Whoami ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct MeOrgEntry {
    id: Uuid,
    name: String,
    slug: String,
    setup_completed: bool,
    /// Role names the user has in this org.
    roles: Vec<String>,
}

#[derive(Debug, Serialize)]
struct MeResponse {
    user_id: Uuid,
    email: String,
    auth_kind: &'static str,
    is_global_admin: bool,
    /// Global (cross-org) permissions only. Org-scoped permissions live
    /// per-org under `orgs[].roles`.
    global_permissions: Vec<String>,
    global_roles: Vec<String>,
    orgs: Vec<MeOrgEntry>,
}

async fn me(
    State(state): State<Arc<AppState>>,
    principal: Principal,
) -> Result<Json<MeResponse>> {
    let auth_kind = match principal.kind {
        PrincipalKind::Jwt => "jwt",
        PrincipalKind::ApiKey => "api_key",
    };

    let global_perms_set =
        db::role_assignments::load_global_permissions(&state.pool, principal.user_id).await?;
    let mut global_permissions: Vec<String> =
        global_perms_set.iter().map(|p| p.to_string()).collect();
    global_permissions.sort();

    let global_roles =
        db::role_assignments::global_role_names_for_user(&state.pool, principal.user_id).await?;

    let orgs = db::orgs::list_for_user(&state.pool, principal.user_id).await?;
    let mut org_entries = Vec::with_capacity(orgs.len());
    for o in orgs {
        let roles = db::role_assignments::role_names_for_user_in_org(
            &state.pool,
            principal.user_id,
            o.id,
        )
        .await?;
        org_entries.push(MeOrgEntry {
            id: o.id,
            name: o.name,
            slug: o.slug,
            setup_completed: o.setup_completed,
            roles,
        });
    }

    Ok(Json(MeResponse {
        user_id: principal.user_id,
        email: principal.email,
        auth_kind,
        is_global_admin: principal.is_global_admin,
        global_permissions,
        global_roles,
        orgs: org_entries,
    }))
}

// ─── API keys ────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct CreateApiKeyRequest {
    name: String,
}

/// Plaintext key returned **once**. Never logged.
#[derive(Debug, Serialize)]
struct CreateApiKeyResponse {
    id: Uuid,
    name: String,
    prefix: String,
    plaintext_key: String,
    created_at: DateTime<Utc>,
}

async fn create_api_key(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Json(req): Json<CreateApiKeyRequest>,
) -> Result<(StatusCode, Json<CreateApiKeyResponse>)> {
    if req.name.trim().is_empty() {
        return Err(EngineError::Validation("name is required".to_string()));
    }
    let generated = auth::api_key::generate()?;
    let row = db::api_keys::insert(
        &state.pool,
        principal.user_id,
        req.name.trim(),
        &generated.prefix,
        &generated.hash,
    )
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(CreateApiKeyResponse {
            id: row.id,
            name: row.name,
            prefix: row.prefix,
            plaintext_key: generated.plaintext,
            created_at: row.created_at,
        }),
    ))
}

async fn list_api_keys(
    State(state): State<Arc<AppState>>,
    principal: Principal,
) -> Result<Json<Vec<db::models::ApiKeyMetadata>>> {
    let rows = db::api_keys::list_by_user(&state.pool, principal.user_id).await?;
    Ok(Json(rows))
}

async fn revoke_api_key(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    let revoked = db::api_keys::revoke(&state.pool, id, principal.user_id).await?;
    if !revoked {
        return Err(EngineError::NotFound(format!("api_key {id}")));
    }
    Ok(StatusCode::NO_CONTENT)
}
