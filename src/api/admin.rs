//! Org-scoped admin endpoints — org settings + auth-config.
//!
//! User management lives in `api::users` and `api::members`, role
//! definitions in `api::roles`, role grants in `api::role_assignments`.
//!
//! Routes are nested under `/api/v1/orgs/{org_id}/admin/...` so the
//! extractor pulls `org_id` from the path automatically.

use axum::{extract::State, routing::get, Router};
use chrono::DateTime;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use super::extractors::{Json, Path};
use crate::auth::{Permission, Principal};
use crate::db;
use crate::db::org_auth_config::UpsertAuthConfig;
use crate::db::org_notification_config::UpsertNotificationConfig;
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
        .route(
            "/api/v1/orgs/{org_id}/admin/notification-config",
            get(get_notification_config).patch(patch_notification_config),
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
    admin_email: Option<String>,
    admin_name: Option<String>,
    support_email: Option<String>,
    description: Option<String>,
}

impl OrgResponse {
    fn from_org(org: crate::db::models::Org) -> Self {
        Self {
            id: org.id,
            name: org.name,
            slug: org.slug,
            setup_completed: org.setup_completed,
            created_at: org.created_at,
            admin_email: org.admin_email,
            admin_name: org.admin_name,
            support_email: org.support_email,
            description: org.description,
        }
    }
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
    Ok(Json(OrgResponse::from_org(org)))
}

/// PATCH body for org settings. Contact / description fields are
/// `Option<String>` — `None` (omitted) leaves the column untouched, and
/// a present empty string clears it to NULL. `name` rejects empty.
#[derive(Debug, Deserialize)]
struct PatchOrgRequest {
    name: Option<String>,
    setup_completed: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_some")]
    admin_email: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    admin_name: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    support_email: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    description: Option<Option<String>>,
}

/// Distinguish "field absent" from "field present and null" in PATCH
/// bodies. Outer `None` = absent → leave column alone; outer
/// `Some(None)` = explicit null → clear to NULL; `Some(Some(s))` = set.
fn deserialize_some<'de, T, D>(deserializer: D) -> std::result::Result<Option<T>, D::Error>
where
    T: serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    T::deserialize(deserializer).map(Some)
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

    // Apply contact updates if any of the four fields were present in
    // the PATCH body. `None` (field absent) keeps the existing value;
    // `Some(None)` and `Some(Some(""))` both clear to NULL; `Some(Some(s))`
    // trims and sets, rejecting all-whitespace as a clear.
    let any_contact_present = req.admin_email.is_some()
        || req.admin_name.is_some()
        || req.support_email.is_some()
        || req.description.is_some();
    if any_contact_present {
        let merge = |patch: Option<Option<String>>, current: Option<String>| -> Option<String> {
            match patch {
                None => current,
                Some(None) => None,
                Some(Some(s)) => {
                    let t = s.trim();
                    if t.is_empty() {
                        None
                    } else {
                        Some(t.to_string())
                    }
                }
            }
        };
        let admin_email = merge(req.admin_email, org.admin_email.clone());
        let admin_name = merge(req.admin_name, org.admin_name.clone());
        let support_email = merge(req.support_email, org.support_email.clone());
        let description = merge(req.description, org.description.clone());
        org = db::orgs::update_contacts(
            &state.pool,
            org_id,
            admin_email.as_deref(),
            admin_name.as_deref(),
            support_email.as_deref(),
            description.as_deref(),
        )
        .await?;
    }

    Ok(Json(OrgResponse::from_org(org)))
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

type AuthConfigRow = (
    String,
    Option<String>,
    Option<String>,
    Option<Vec<u8>>,
    Option<String>,
);

async fn get_auth_config(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(org_id): Path<Uuid>,
) -> Result<Json<AuthConfigResponse>> {
    principal.require(Permission::AuthConfigRead)?;

    let row: Option<AuthConfigRow> = sqlx::query_as(
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

    let row: Option<(Option<Vec<u8>>,)> =
        sqlx::query_as("SELECT oidc_client_secret_enc FROM org_auth_config WHERE org_id = $1")
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

// ---- notification config ----------------------------------------------------

#[derive(Debug, Serialize)]
struct NotificationConfigResponse {
    provider: String,
    from_email: Option<String>,
    from_name: Option<String>,
    /// Whether a SendGrid API key has been stored. Plaintext is never
    /// returned — clients use this flag to render a "key is set" hint.
    sendgrid_api_key_set: bool,
    smtp_host: Option<String>,
    smtp_port: Option<i32>,
    smtp_username: Option<String>,
    smtp_password_set: bool,
    smtp_use_tls: bool,
}

/// Tuple shape returned by the inline SELECT — alias for readability.
type NotificationConfigRow = (
    String,          // provider
    Option<String>,  // from_email
    Option<String>,  // from_name
    Option<Vec<u8>>, // sendgrid_api_key_enc
    Option<String>,  // smtp_host
    Option<i32>,     // smtp_port
    Option<String>,  // smtp_username
    Option<Vec<u8>>, // smtp_password_enc
    bool,            // smtp_use_tls
);

async fn get_notification_config(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(org_id): Path<Uuid>,
) -> Result<Json<NotificationConfigResponse>> {
    principal.require(Permission::NotificationConfigRead)?;

    let row: Option<NotificationConfigRow> = sqlx::query_as(
        "SELECT provider, from_email, from_name, sendgrid_api_key_enc, \
                smtp_host, smtp_port, smtp_username, smtp_password_enc, smtp_use_tls \
         FROM org_notification_config WHERE org_id = $1",
    )
    .bind(org_id)
    .fetch_optional(&state.pool)
    .await?;

    Ok(Json(match row {
        Some((
            provider,
            from_email,
            from_name,
            sendgrid_enc,
            smtp_host,
            smtp_port,
            smtp_username,
            smtp_pw_enc,
            smtp_use_tls,
        )) => NotificationConfigResponse {
            provider,
            from_email,
            from_name,
            sendgrid_api_key_set: sendgrid_enc.is_some(),
            smtp_host,
            smtp_port,
            smtp_username,
            smtp_password_set: smtp_pw_enc.is_some(),
            smtp_use_tls,
        },
        None => NotificationConfigResponse {
            provider: "disabled".into(),
            from_email: None,
            from_name: None,
            sendgrid_api_key_set: false,
            smtp_host: None,
            smtp_port: None,
            smtp_username: None,
            smtp_password_set: false,
            smtp_use_tls: true,
        },
    }))
}

#[derive(Debug, Deserialize)]
struct PatchNotificationConfigRequest {
    provider: String,
    from_email: Option<String>,
    from_name: Option<String>,
    /// New plaintext SendGrid key. Omit (or `null`) to preserve the
    /// previously stored value when rotating other fields.
    sendgrid_api_key: Option<String>,
    smtp_host: Option<String>,
    smtp_port: Option<i32>,
    smtp_username: Option<String>,
    /// New plaintext SMTP password. Same omit-to-preserve semantics.
    smtp_password: Option<String>,
    #[serde(default = "default_true")]
    smtp_use_tls: bool,
}

fn default_true() -> bool {
    true
}

async fn patch_notification_config(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(org_id): Path<Uuid>,
    Json(req): Json<PatchNotificationConfigRequest>,
) -> Result<Json<NotificationConfigResponse>> {
    principal.require(Permission::NotificationConfigUpdate)?;

    if !matches!(req.provider.as_str(), "disabled" | "sendgrid" | "smtp") {
        return Err(EngineError::Validation(
            "provider must be 'disabled', 'sendgrid', or 'smtp'".into(),
        ));
    }

    let config = UpsertNotificationConfig {
        provider: &req.provider,
        from_email: req.from_email.as_deref(),
        from_name: req.from_name.as_deref(),
        sendgrid_api_key: req.sendgrid_api_key.as_deref(),
        smtp_host: req.smtp_host.as_deref(),
        smtp_port: req.smtp_port,
        smtp_username: req.smtp_username.as_deref(),
        smtp_password: req.smtp_password.as_deref(),
        smtp_use_tls: req.smtp_use_tls,
    };

    db::org_notification_config::upsert(&state.pool, &state.secrets_key, org_id, config).await?;

    // Re-read so the response reflects post-upsert *_set booleans
    // (preserving callers that didn't rotate a secret).
    get_notification_config(State(state), principal, Path(org_id)).await
}
