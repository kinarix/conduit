//! Platform admin management.
//!
//! Endpoints scoped to the *global* `PlatformAdmin` role. Caller must hold
//! `user.create` / `user.read` / `user.update` / `user.delete` and
//! `role_assignment.create` / `role_assignment.delete` GLOBALLY — the route
//! lives outside `/api/v1/orgs/{org_id}/...` so the Principal extractor
//! resolves only global permissions.
//!
//! Routes:
//!   GET    /api/v1/admin/platform-admins
//!   POST   /api/v1/admin/platform-admins
//!   PATCH  /api/v1/admin/platform-admins/{user_id}
//!   DELETE /api/v1/admin/platform-admins/{user_id}
//!
//! Notes:
//!   - PATCH and DELETE are narrowed to confirmed platform admins so a
//!     PlatformAdmin can't use this endpoint as a back door into arbitrary
//!     user PII.
//!   - DELETE only revokes the grant; the user account stays around (they
//!     may still be a member of orgs). Refuses if revoking would drop the
//!     count of platform admins to zero.

use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, patch},
    Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use super::extractors::{Json, Path};
use crate::auth::{self, Permission, Principal};
use crate::db;
use crate::error::{EngineError, Result};
use crate::state::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/v1/admin/platform-admins",
            get(list_platform_admins).post(create_platform_admin),
        )
        .route(
            "/api/v1/admin/platform-admins/{user_id}",
            patch(patch_platform_admin).delete(revoke_platform_admin),
        )
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct PlatformAdminRow {
    user_id: Uuid,
    email: String,
    name: Option<String>,
    auth_provider: String,
    user_created_at: DateTime<Utc>,
    assignment_id: Uuid,
    granted_at: DateTime<Utc>,
    granted_by: Option<Uuid>,
}

/// Single JOIN to avoid an N+1 against the users table.
async fn list_platform_admins(
    State(state): State<Arc<AppState>>,
    principal: Principal,
) -> Result<Json<Vec<PlatformAdminRow>>> {
    principal.require(Permission::UserRead)?;
    principal.require(Permission::RoleAssignmentRead)?;

    let rows: Vec<PlatformAdminRow> = sqlx::query_as(
        r#"
        SELECT u.id            AS user_id,
               u.email         AS email,
               u.name          AS name,
               u.auth_provider AS auth_provider,
               u.created_at    AS user_created_at,
               gra.id          AS assignment_id,
               gra.granted_at  AS granted_at,
               gra.granted_by  AS granted_by
          FROM global_role_assignments gra
          JOIN roles r ON r.id = gra.role_id
          JOIN users u ON u.id = gra.user_id
         WHERE r.name = 'PlatformAdmin'
           AND r.org_id IS NULL
         ORDER BY LOWER(u.email)
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(rows))
}

#[derive(Debug, Deserialize)]
struct CreatePlatformAdminRequest {
    auth_provider: String,
    external_id: Option<String>,
    email: String,
    password: Option<String>,
    name: Option<String>,
    phone: Option<String>,
}

#[derive(Debug, Serialize)]
struct CreatePlatformAdminResponse {
    user_id: Uuid,
    email: String,
    name: Option<String>,
    auth_provider: String,
    assignment_id: Uuid,
}

/// Create a brand-new user and grant them `PlatformAdmin` in a single
/// transaction. There is intentionally no "promote existing user" path here
/// — keeps the form a single-step flow and avoids accidental promotion.
async fn create_platform_admin(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Json(req): Json<CreatePlatformAdminRequest>,
) -> Result<(StatusCode, Json<CreatePlatformAdminResponse>)> {
    principal.require(Permission::UserCreate)?;
    principal.require(Permission::RoleAssignmentCreate)?;

    if !matches!(req.auth_provider.as_str(), "internal" | "external") {
        return Err(EngineError::Validation(
            "auth_provider must be 'internal' or 'external'".into(),
        ));
    }
    let email = req.email.trim();
    if email.is_empty() {
        return Err(EngineError::Validation("email must not be empty".into()));
    }

    let password_hash = match req.auth_provider.as_str() {
        "internal" => {
            let pw = req.password.as_deref().ok_or_else(|| {
                EngineError::Validation(
                    "password is required for internal auth_provider".into(),
                )
            })?;
            if pw.len() < 8 {
                return Err(EngineError::Validation(
                    "password must be at least 8 characters".into(),
                ));
            }
            Some(auth::password::hash(pw)?)
        }
        _ => {
            if req.password.is_some() {
                return Err(EngineError::Validation(
                    "password must not be set for external auth_provider".into(),
                ));
            }
            None
        }
    };

    let trimmed_name = req.name.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let trimmed_phone = req.phone.as_deref().map(str::trim).filter(|s| !s.is_empty());

    let user = db::users::insert(
        &state.pool,
        &req.auth_provider,
        req.external_id.as_deref(),
        email,
        password_hash.as_deref(),
        trimmed_name,
        trimmed_phone,
    )
    .await
    .map_err(map_email_conflict)?;

    let granted = db::role_assignments::grant_global_by_name(
        &state.pool,
        user.id,
        "PlatformAdmin",
        Some(principal.user_id),
    )
    .await?;
    if !granted {
        return Err(EngineError::Internal(
            "Built-in PlatformAdmin role not found".into(),
        ));
    }

    // Re-fetch the specific PlatformAdmin assignment id for the response.
    let assignment_id: (Uuid,) = sqlx::query_as(
        r#"
        SELECT gra.id
          FROM global_role_assignments gra
          JOIN roles r ON r.id = gra.role_id
         WHERE r.name = 'PlatformAdmin'
           AND r.org_id IS NULL
           AND gra.user_id = $1
        "#,
    )
    .bind(user.id)
    .fetch_one(&state.pool)
    .await?;
    let assignment_id = assignment_id.0;

    Ok((
        StatusCode::CREATED,
        Json(CreatePlatformAdminResponse {
            user_id: user.id,
            email: user.email,
            name: user.name,
            auth_provider: user.auth_provider,
            assignment_id,
        }),
    ))
}

#[derive(Debug, Deserialize)]
struct PatchPlatformAdminRequest {
    email: Option<String>,
    name: Option<String>,
}

#[derive(Debug, Serialize)]
struct PatchPlatformAdminResponse {
    user_id: Uuid,
    email: String,
    name: Option<String>,
}

/// Update email / display name on a confirmed platform admin. Narrowed so a
/// platform admin can't repurpose this endpoint to edit arbitrary users.
async fn patch_platform_admin(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(user_id): Path<Uuid>,
    Json(req): Json<PatchPlatformAdminRequest>,
) -> Result<Json<PatchPlatformAdminResponse>> {
    principal.require(Permission::UserUpdate)?;

    // Confirm the target is a platform admin before touching them.
    require_is_platform_admin(&state.pool, user_id).await?;

    let new_email = match req.email.as_deref() {
        Some(s) => {
            let t = s.trim();
            if t.is_empty() {
                return Err(EngineError::Validation("email must not be empty".into()));
            }
            Some(t.to_string())
        }
        None => None,
    };
    let new_name = req.name.as_deref().map(|s| s.trim().to_string());

    // No-op? Re-read and return.
    if new_email.is_none() && new_name.is_none() {
        let user = db::users::find_by_id(&state.pool, user_id)
            .await?
            .ok_or_else(|| EngineError::NotFound(format!("user {user_id}")))?;
        return Ok(Json(PatchPlatformAdminResponse {
            user_id: user.id,
            email: user.email,
            name: user.name,
        }));
    }

    // Single UPDATE with COALESCE; explicit empty `name` ("") clears to NULL,
    // omitted leaves untouched. `email` is non-nullable on the column.
    let row: Option<(Uuid, String, Option<String>)> = sqlx::query_as(
        r#"
        UPDATE users
           SET email = COALESCE($2, email),
               name  = CASE
                         WHEN $3::text IS NULL THEN name
                         WHEN $3::text = ''    THEN NULL
                         ELSE $3::text
                       END
         WHERE id = $1
        RETURNING id, email, name
        "#,
    )
    .bind(user_id)
    .bind(new_email.as_deref())
    .bind(new_name.as_deref())
    .fetch_optional(&state.pool)
    .await
    .map_err(EngineError::from)
    .map_err(map_email_conflict)?;

    let (id, email, name) =
        row.ok_or_else(|| EngineError::NotFound(format!("user {user_id}")))?;

    Ok(Json(PatchPlatformAdminResponse {
        user_id: id,
        email,
        name,
    }))
}

/// Revoke the global `PlatformAdmin` grant. The user account itself is
/// preserved (they may still be an org member). Refuses to drop the platform
/// admin count to zero — there must always be at least one global admin so
/// the platform isn't left unmanageable.
async fn revoke_platform_admin(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(user_id): Path<Uuid>,
) -> Result<StatusCode> {
    principal.require(Permission::RoleAssignmentDelete)?;

    // Find the specific PlatformAdmin assignment row for this user.
    let assignment: Option<(Uuid,)> = sqlx::query_as(
        r#"
        SELECT gra.id
          FROM global_role_assignments gra
          JOIN roles r ON r.id = gra.role_id
         WHERE r.name = 'PlatformAdmin'
           AND r.org_id IS NULL
           AND gra.user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await?;

    let Some((assignment_id,)) = assignment else {
        return Err(EngineError::NotFound(format!(
            "platform admin grant for user {user_id}"
        )));
    };

    // Last-admin guard: refuse if this revoke would orphan the platform.
    let (count,): (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*)
          FROM global_role_assignments gra
          JOIN roles r ON r.id = gra.role_id
         WHERE r.name = 'PlatformAdmin'
           AND r.org_id IS NULL
        "#,
    )
    .fetch_one(&state.pool)
    .await?;
    if count <= 1 {
        return Err(EngineError::Validation(
            "cannot revoke the last platform admin — promote another user first".into(),
        ));
    }

    let deleted = db::role_assignments::revoke_global_by_id(&state.pool, assignment_id).await?;
    if !deleted {
        return Err(EngineError::NotFound(format!(
            "platform admin grant for user {user_id}"
        )));
    }
    Ok(StatusCode::NO_CONTENT)
}

// ─── helpers ─────────────────────────────────────────────────────────────────

async fn require_is_platform_admin(pool: &sqlx::PgPool, user_id: Uuid) -> Result<()> {
    let (is_admin,): (bool,) = sqlx::query_as(
        r#"
        SELECT EXISTS(
            SELECT 1
              FROM global_role_assignments gra
              JOIN roles r ON r.id = gra.role_id
             WHERE r.name = 'PlatformAdmin'
               AND r.org_id IS NULL
               AND gra.user_id = $1
        )
        "#,
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    if !is_admin {
        return Err(EngineError::NotFound(format!(
            "platform admin {user_id}"
        )));
    }
    Ok(())
}

/// Re-map SQLx unique-violation on `users.email` to a clearer message. The
/// default mapping in `From<sqlx::Error> for EngineError` produces a generic
/// "Resource already exists" — fine for most callers, less helpful here.
fn map_email_conflict(err: EngineError) -> EngineError {
    match err {
        EngineError::Conflict(_) => EngineError::Conflict("email already in use".into()),
        other => other,
    }
}
