use super::extractors::Json;
use axum::{extract::State, http::StatusCode, routing::post, Router};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::db::models::User;
use crate::db::users;
use crate::error::{EngineError, Result};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub org_id: Uuid,
    pub auth_provider: String,
    pub external_id: Option<String>,
    pub email: String,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/v1/users", post(create_user))
}

#[tracing::instrument(skip_all, fields(org_id = %req.org_id, auth_provider = %req.auth_provider))]
async fn create_user(
    State(state): State<Arc<AppState>>,
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
    let user = users::insert(
        &state.pool,
        req.org_id,
        &req.auth_provider,
        req.external_id.as_deref(),
        &req.email,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(user)))
}
