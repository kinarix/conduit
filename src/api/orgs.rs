use super::extractors::{Json, Path};
use axum::{
    extract::State,
    http::StatusCode,
    routing::{delete, get, post},
    Router,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::db::models::Org;
use crate::db::orgs;
use crate::error::{EngineError, Result};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct CreateOrgRequest {
    pub name: String,
    pub slug: String,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/orgs", get(list_orgs))
        .route("/api/v1/orgs", post(create_org))
        .route("/api/v1/orgs/{id}", delete(delete_org))
}

async fn list_orgs(State(state): State<Arc<AppState>>) -> Result<Json<Vec<Org>>> {
    let orgs = orgs::list_all(&state.pool).await?;
    Ok(Json(orgs))
}

async fn create_org(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateOrgRequest>,
) -> Result<(StatusCode, Json<Org>)> {
    if req.name.trim().is_empty() || req.slug.trim().is_empty() {
        return Err(EngineError::Validation(
            "name and slug must not be empty".to_string(),
        ));
    }
    let org = orgs::insert(&state.pool, &req.name, &req.slug).await?;
    Ok((StatusCode::CREATED, Json(org)))
}

async fn delete_org(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    orgs::delete(&state.pool, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
