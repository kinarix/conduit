//! Org-scoped admin endpoints — org settings + auth-config.
//!
//! User management lives in `api::users` and `api::members`, role
//! definitions in `api::roles`, role grants in `api::role_assignments`.
//!
//! Routes are nested under `/api/v1/orgs/{org_id}/admin/...` so the
//! extractor pulls `org_id` from the path automatically.

use axum::{
    extract::State,
    routing::get,
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
use crate::error::{EngineError, Result};
use crate::state::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/v1/orgs/{org_id}/admin/org",
            get(get_org).patch(patch_org),
        )
        .route(
            "/api/v1/orgs/{org_id}/admin/auth-config",
            get(get_auth_config).patch(patch_auth_config),
        )
}

// ─── Org settings ────────────────────────────────────────────────────────────

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
    Path(org_id): Path<Uuid>,
) -> Result<Json<OrgResponse>> {
    principal.require(Permission::OrgRead)?;
    let org = db::orgs::get_by_id(&state.pool, org_id)
        .await?
        .ok_or_else(|| EngineError::NotFound(format!("org {org_id}")))?;
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
    Path(org_id): Path<Uuid>,
    Json(req): Json<PatchOrgRequest>,
) -> Result<Json<OrgResponse>> {
    principal.require(Permission::OrgUpdate)?;

    let mut org = db::orgs::get_by_id(&state.pool, org_id)
        .await?
        .ok_or_else(|| EngineError::NotFound(format!("org {org_id}")))?;

    if let Some(name) = req.name.as_deref() {
        let name = name.trim();
        if name.is_empty() {
            return Err(EngineError::Validation("name cannot be empty".into()));
        }
        org = db::orgs::update_name(&state.pool, org_id, name).await?;
    }
    if let Some(completed) = req.setup_completed {
        db::orgs::set_setup_completed(&state.pool, org_id, completed).await?;
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
    oidc_client_secret_set: bool,
    oidc_redirect_uri: Option<String>,
}

async fn get_auth_config(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(org_id): Path<Uuid>,
) -> Result<Json<AuthConfigResponse>> {
    principal.require(Permission::AuthConfigRead)?;

    let row: Option<(String, Option<String>, Option<String>, Option<Vec<u8>>, Option<String>)> =
        sqlx::query_as(
            "SELECT provider, oidc_issuer, oidc_client_id, oidc_client_secret_enc, oidc_redirect_uri \
             FROM org_auth_config WHERE org_id = $1",
        )
        .bind(org_id)
        .fetch_optional(&state.pool)
        .await?;

    Ok(Json(match row {
        Some((provider, issuer, client_id, secret_enc, redirect_uri)) => AuthConfigResponse {
            provider,
            oidc_issuer: issuer,
            oidc_client_id: client_id,
            oidc_client_secret_set: secret_enc.is_some(),
            oidc_redirect_uri: redirect_uri,
        },
        None => AuthConfigResponse {
            provider: "internal".into(),
            oidc_issuer: None,
            oidc_client_id: None,
            oidc_client_secret_set: false,
            oidc_redirect_uri: None,
        },
    }))
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
    Path(org_id): Path<Uuid>,
    Json(req): Json<PatchAuthConfigRequest>,
) -> Result<Json<AuthConfigResponse>> {
    principal.require(Permission::AuthConfigUpdate)?;

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

    db::org_auth_config::upsert(&state.pool, &state.secrets_key, org_id, config).await?;

    let row: Option<(Option<Vec<u8>>,)> = sqlx::query_as(
        "SELECT oidc_client_secret_enc FROM org_auth_config WHERE org_id = $1",
    )
    .bind(org_id)
    .fetch_optional(&state.pool)
    .await?;
    let secret_set = row.and_then(|(s,)| s).is_some();

    Ok(Json(AuthConfigResponse {
        provider: req.provider,
        oidc_issuer: req.oidc_issuer,
        oidc_client_id: req.oidc_client_id,
        oidc_client_secret_set: secret_set,
        oidc_redirect_uri: req.oidc_redirect_uri,
    }))
}
