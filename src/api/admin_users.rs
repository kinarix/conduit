//! Flat (non-org-scoped) admin endpoints for user management.
//!
//! The route lives outside `/api/v1/orgs/{org_id}/...`, so the `Principal`
//! extractor loads global permissions only — meaning `user.reset_password`
//! must be held GLOBALLY (PlatformAdmin) to call it. The org-scoped variant
//! at `/api/v1/orgs/{org_id}/users/{user_id}/reset-password` is for org
//! admins.

use axum::{extract::State, http::StatusCode, routing::post, Router};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use super::extractors::{Json, Path};
use crate::auth::{self, Permission, Principal};
use crate::db::users;
use crate::error::{EngineError, Result};
use crate::state::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route(
        "/api/v1/admin/users/{user_id}/reset-password",
        post(reset_password),
    )
}

#[derive(Debug, Deserialize)]
struct ResetPasswordRequest {
    new_password: String,
}

#[tracing::instrument(skip_all, fields(user_id = %user_id))]
async fn reset_password(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(user_id): Path<Uuid>,
    Json(req): Json<ResetPasswordRequest>,
) -> Result<StatusCode> {
    principal.require(Permission::UserResetPassword)?;

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
