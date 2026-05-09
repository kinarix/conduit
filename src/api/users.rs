use super::extractors::Json;
use axum::{extract::State, http::StatusCode, routing::post, Router};
use serde::Deserialize;
use std::sync::Arc;

use crate::auth;
use crate::auth::Principal;
use crate::db::models::User;
use crate::db::users;
use crate::error::{EngineError, Result};
use crate::state::AppState;

/// New users always land in the calling principal's org. `password` is
/// required when `auth_provider == "internal"`; rejected otherwise.
#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub auth_provider: String,
    pub external_id: Option<String>,
    pub email: String,
    pub password: Option<String>,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/v1/users", post(create_user))
}

#[tracing::instrument(skip_all, fields(org_id = %principal.org_id, auth_provider = %req.auth_provider))]
async fn create_user(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Json(req): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<User>)> {
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
    let user = users::insert(
        &state.pool,
        principal.org_id,
        &req.auth_provider,
        req.external_id.as_deref(),
        &req.email,
        password_hash.as_deref(),
    )
    .await?;
    Ok((StatusCode::CREATED, Json(user)))
}
