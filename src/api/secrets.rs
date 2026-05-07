//! Org-scoped secrets API. Plaintext values flow inbound on `POST` only;
//! never returned via `GET` or `LIST`. Rotation is "delete then create".

use super::extractors::{Json, Path};
use axum::{extract::State, http::StatusCode, routing::get, Router};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::db::models::SecretMetadata;
use crate::db::secrets;
use crate::error::{EngineError, Result};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct CreateSecretRequest {
    pub name: String,
    pub value: String,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/v1/orgs/{org_id}/secrets",
            get(list_secrets).post(create_secret),
        )
        .route(
            "/api/v1/orgs/{org_id}/secrets/{name}",
            get(get_secret).delete(delete_secret),
        )
}

async fn list_secrets(
    State(state): State<Arc<AppState>>,
    Path(org_id): Path<Uuid>,
) -> Result<Json<Vec<SecretMetadata>>> {
    let rows = secrets::list(&state.pool, org_id).await?;
    Ok(Json(rows))
}

async fn create_secret(
    State(state): State<Arc<AppState>>,
    Path(org_id): Path<Uuid>,
    Json(req): Json<CreateSecretRequest>,
) -> Result<(StatusCode, Json<SecretMetadata>)> {
    let name = req.name.trim();
    if name.is_empty() {
        return Err(EngineError::Validation("name must not be empty".into()));
    }
    if req.value.is_empty() {
        return Err(EngineError::Validation("value must not be empty".into()));
    }
    let row = secrets::create(&state.pool, &state.secrets_key, org_id, name, &req.value).await?;
    Ok((StatusCode::CREATED, Json(row)))
}

async fn get_secret(
    State(state): State<Arc<AppState>>,
    Path((org_id, name)): Path<(Uuid, String)>,
) -> Result<Json<SecretMetadata>> {
    let row = secrets::get_metadata(&state.pool, org_id, &name)
        .await?
        .ok_or_else(|| EngineError::NotFound(format!("secret '{name}' not found")))?;
    Ok(Json(row))
}

async fn delete_secret(
    State(state): State<Arc<AppState>>,
    Path((org_id, name)): Path<(Uuid, String)>,
) -> Result<StatusCode> {
    secrets::delete(&state.pool, org_id, &name).await?;
    Ok(StatusCode::NO_CONTENT)
}
