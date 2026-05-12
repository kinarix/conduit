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
    // Optional contact / description metadata. Empty strings are treated
    // as omitted so the UI can submit blank fields without persisting
    // them as zero-length strings.
    #[serde(default)]
    pub admin_email: Option<String>,
    #[serde(default)]
    pub admin_name: Option<String>,
    #[serde(default)]
    pub support_email: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
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
        .route("/api/v1/orgs/{id}/stats", get(get_org_stats))
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
    // Empty-string → None so the UI can submit blank optional fields
    // without persisting them as zero-length strings.
    let trim_opt = |s: &Option<String>| {
        s.as_ref()
            .map(|v| v.trim())
            .filter(|v| !v.is_empty())
            .map(|v| v.to_string())
    };
    let admin_email = trim_opt(&req.admin_email);
    let admin_name = trim_opt(&req.admin_name);
    let support_email = trim_opt(&req.support_email);
    let description = trim_opt(&req.description);
    let org = orgs::insert_with_contacts(
        &state.pool,
        req.name.trim(),
        slug,
        crate::db::orgs::NewOrgContacts {
            admin_email: admin_email.as_deref(),
            admin_name: admin_name.as_deref(),
            support_email: support_email.as_deref(),
            description: description.as_deref(),
        },
    )
    .await?;

    // Auto-grant the creator OrgOwner of the new org so they can operate
    // inside it without a separate grant — but only when the creator is
    // a regular user. Global platform admins already have every
    // permission across every org via their global grant, so adding them
    // as an org member would just clutter the org's member list (and
    // imply the platform admin is "part of" this tenant, which they
    // aren't — see ADR-009 amendment 2026-05-12).
    if !principal.is_global_admin {
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

    // Refuse to delete a non-empty org. We could rely on the FK
    // `ON DELETE RESTRICT` on processes / decisions / instances, but the
    // resulting error is an opaque DB constraint violation and members
    // would CASCADE-disappear silently. Doing the explicit check gives
    // the caller a structured, actionable response and lines up with
    // the UI's pre-flight stats fetch.
    let s = orgs::stats(&state.pool, id).await?;
    if !s.is_empty() {
        return Err(EngineError::Validation(format!(
            "org is not empty: {} member(s), {} process definition(s), \
             {} decision definition(s), {} instance(s) — remove these first",
            s.members, s.processes, s.decisions, s.instances,
        )));
    }

    orgs::delete(&state.pool, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[tracing::instrument(skip_all, fields(id = %id))]
async fn get_org_stats(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(id): Path<Uuid>,
) -> Result<Json<crate::db::orgs::OrgStats>> {
    // Visible to anyone with `org.delete` either globally or in this
    // org — same authority surface as the delete endpoint, since the
    // counts are pre-flight data for that flow.
    let allowed = if principal.has(Permission::OrgDelete) {
        true
    } else {
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
    Ok(Json(orgs::stats(&state.pool, id).await?))
}
