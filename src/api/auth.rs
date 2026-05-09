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

#[derive(Debug, Deserialize)]
struct LoginRequest {
    email: String,
    password: String,
    org_slug: String,
}

#[derive(Debug, Serialize)]
struct LoginResponse {
    access_token: String,
    token_type: &'static str,
    expires_in: i64,
}

#[tracing::instrument(skip_all, fields(email = %req.email, org = %req.org_slug))]
async fn login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>> {
    // The four failure modes — unknown org, unknown user, wrong password,
    // external-auth user trying internal login — all return the same generic
    // U011. Never branch the response on which one fired.
    let creds =
        db::users::find_credentials_by_org_slug_and_email(&state.pool, &req.org_slug, &req.email)
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

// ─── Whoami ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct MeResponse {
    user_id: Uuid,
    org_id: Uuid,
    email: String,
    auth_kind: &'static str,
}

async fn me(principal: Principal) -> Json<MeResponse> {
    let auth_kind = match principal.kind {
        PrincipalKind::Jwt => "jwt",
        PrincipalKind::ApiKey => "api_key",
    };
    Json(MeResponse {
        user_id: principal.user_id,
        org_id: principal.org_id,
        email: principal.email,
        auth_kind,
    })
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
