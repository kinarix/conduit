use super::extractors::{Json, Path, Query};
use super::pagination::{with_total, Page};
use axum::{
    extract::State,
    http::StatusCode,
    routing::{delete, get, post},
    Router,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::{Permission, Principal};
use crate::db::models::Org;
use crate::db::orgs;
use crate::error::{EngineError, Result};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct CreateOrgRequest {
    pub name: String,
    pub slug: String,
}

#[derive(Debug, Deserialize)]
pub struct ListOrgsQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/orgs", get(list_orgs))
        .route("/api/v1/orgs", post(create_org))
        .route("/api/v1/orgs/{id}", delete(delete_org))
}

// `/api/v1/orgs` (the flat list/create endpoints) is not org-scoped — the
// extractor leaves `current_org_id` empty. Listing is filtered to the orgs
// the caller is a member of; global admins see every org. Creating requires
// the global `org.create` permission.
#[tracing::instrument(skip_all)]
async fn list_orgs(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Query(params): Query<ListOrgsQuery>,
) -> Result<axum::response::Response> {
    let _ = Page::from_query(params.limit, params.offset);

    let rows = if principal.is_global_admin {
        orgs::list_all(&state.pool).await?
    } else {
        orgs::list_for_user(&state.pool, principal.user_id).await?
    };
    let total = rows.len() as i64;
    Ok(with_total(rows, total))
}

#[tracing::instrument(skip_all, fields(slug = %req.slug))]
async fn create_org(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Json(req): Json<CreateOrgRequest>,
) -> Result<(StatusCode, Json<Org>)> {
    principal.require(Permission::OrgCreate)?;
    if req.name.trim().is_empty() {
        return Err(EngineError::Validation(
            "name must not be empty".to_string(),
        ));
    }
    let slug = req.slug.trim();
    if slug.is_empty() {
        return Err(EngineError::Validation(
            "slug must not be empty".to_string(),
        ));
    }
    let org = orgs::insert(&state.pool, req.name.trim(), slug).await?;

    // The creator becomes a member + OrgOwner of the new org so they can
    // immediately operate inside it without needing a separate grant.
    crate::db::org_members::insert(
        &state.pool,
        principal.user_id,
        org.id,
        Some(principal.user_id),
    )
    .await?;
    if let Some(owner_role_id) =
        crate::db::roles::find_builtin_by_name(&state.pool, "OrgOwner").await?
    {
        crate::db::role_assignments::grant_org(
            &state.pool,
            principal.user_id,
            owner_role_id,
            org.id,
            Some(principal.user_id),
            Some(org.id),
        )
        .await?;
    }

    Ok((StatusCode::CREATED, Json(org)))
}

#[tracing::instrument(skip_all, fields(id = %id))]
async fn delete_org(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    // Deletion lives on the flat route (no org_id in path) — gating on the
    // org-scoped `org.delete` permission would force callers to enter the
    // org first, which is ugly. Instead we require either:
    //   - global org.delete (platform admin), OR
    //   - org.delete inside the target org (caller is OrgOwner of `id`).
    let allowed = if principal.has(Permission::OrgDelete) {
        true
    } else {
        // Membership-scoped delete: load the caller's perms in this specific org.
        let in_org =
            crate::db::role_assignments::load_org_permissions(&state.pool, principal.user_id, id)
                .await?;
        in_org.contains(&Permission::OrgDelete)
    };
    if !allowed {
        return Err(EngineError::Forbidden(
            "permission required: org.delete".to_string(),
        ));
    }
    orgs::delete(&state.pool, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
