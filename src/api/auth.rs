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
        .route("/api/v1/auth/orgs", get(list_login_orgs))
        .route("/api/v1/me", get(me))
        .route("/api/v1/api-keys", post(create_api_key).get(list_api_keys))
        .route("/api/v1/api-keys/{id}", delete(revoke_api_key))
}

// ─── Login ───────────────────────────────────────────────────────────────────

/// `org_slug` is optional: when absent or empty the request is interpreted as
/// a platform-admin login and routed to the system org `conduit`. The slug
/// `conduit` is reserved (validated in `POST /api/v1/orgs`), so there is no
/// ambiguity with a tenant org.
#[derive(Debug, Deserialize)]
struct LoginRequest {
    email: String,
    password: String,
    #[serde(default)]
    org_slug: Option<String>,
}

#[derive(Debug, Serialize)]
struct LoginResponse {
    access_token: String,
    token_type: &'static str,
    expires_in: i64,
}

const PLATFORM_ORG_SLUG: &str = "conduit";

#[tracing::instrument(skip_all, fields(email = %req.email))]
async fn login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>> {
    let slug = req
        .org_slug
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(PLATFORM_ORG_SLUG);

    // The four failure modes — unknown org, unknown user, wrong password,
    // external-auth user trying internal login — all return the same generic
    // U011. Never branch the response on which one fired.
    let creds =
        db::users::find_credentials_by_org_slug_and_email(&state.pool, slug, &req.email)
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
        creds.org_id,
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

// ─── Login org dropdown (public) ─────────────────────────────────────────────

/// Public — no auth. Backs the org dropdown on the login page. Returns every
/// org's display name and slug (system orgs included, so `Conduit` shows up as
/// the platform-admin sign-in target).
///
/// This is an intentional info-disclosure tradeoff: anyone who can reach the
/// login page can see the list of tenants. Acceptable for single-tenant and
/// small-multi-tenant self-hosted deployments. Public-SaaS operators should
/// front this with a separate sign-in flow.
#[derive(Debug, Serialize)]
struct LoginOrg {
    name: String,
    slug: String,
    is_system: bool,
}

async fn list_login_orgs(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<LoginOrg>>> {
    // System orgs first (Conduit at the top), then real orgs alphabetically.
    let rows = sqlx::query!(
        "SELECT name, slug, is_system FROM orgs ORDER BY is_system DESC, name ASC"
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|r| LoginOrg {
                name: r.name,
                slug: r.slug,
                is_system: r.is_system,
            })
            .collect(),
    ))
}

// ─── Whoami ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct MeResponse {
    user_id: Uuid,
    org_id: Uuid,
    email: String,
    auth_kind: &'static str,
    permissions: Vec<String>,
    roles: Vec<String>,
    setup_completed: bool,
}

async fn me(
    State(state): State<Arc<AppState>>,
    principal: Principal,
) -> Result<Json<MeResponse>> {
    let auth_kind = match principal.kind {
        PrincipalKind::Jwt => "jwt",
        PrincipalKind::ApiKey => "api_key",
    };

    let user_roles = db::roles::list_user_roles(&state.pool, principal.user_id).await?;
    let org = db::orgs::get_by_id(&state.pool, principal.org_id)
        .await?
        .ok_or_else(|| EngineError::NotFound(format!("org {}", principal.org_id)))?;

    let mut permissions: Vec<String> = principal
        .permissions
        .iter()
        .map(|p| p.to_string())
        .collect();
    permissions.sort();

    Ok(Json(MeResponse {
        user_id: principal.user_id,
        org_id: principal.org_id,
        email: principal.email,
        auth_kind,
        permissions,
        roles: user_roles.into_iter().map(|r| r.role_name).collect(),
        setup_completed: org.setup_completed,
    }))
}

// ─── API keys ────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct CreateApiKeyRequest {
    name: String,
}

/// Plaintext key returned **once**. Never logged. The user is expected to
/// copy it immediately; subsequent reads only return the prefix.
#[derive(Debug, Serialize)]
struct CreateApiKeyResponse {
    id: Uuid,
    name: String,
    prefix: String,
    /// Full `ck_…` plaintext. Surface in the UI as "save this now — you will
    /// not see it again." Conduit never stores or echoes the plaintext.
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
