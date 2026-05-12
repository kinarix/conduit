//! Role assignment grants, split by scope.
//!
//! Two tables drive RBAC:
//!   - `org_role_assignments`    — role granted inside a specific org.
//!   - `global_role_assignments` — role granted platform-wide.
//!
//! A user's effective permissions inside an org are the union of:
//!   1. role_permissions for every `global_role_assignments` row, AND
//!   2. role_permissions for every `org_role_assignments` row in that org.
//!
//! See `migrations/021_roles.sql` for the catalog and built-in role seeds,
//! and `migrations/023_role_assignments.sql` for these tables. Pg-scoped
//! grants live in `process_group_role_assignments` (migration 024).

use std::collections::HashSet;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::permission::Permission;
use crate::error::{EngineError, Result};

// ─── Permission loading ──────────────────────────────────────────────────────

/// Load the set of permissions a user holds via GLOBAL role assignments
/// (apply across every org).
pub async fn load_global_permissions(pool: &PgPool, user_id: Uuid) -> Result<HashSet<Permission>> {
    let rows: Vec<(String,)> = sqlx::query_as(
        r#"
        SELECT rp.permission
        FROM global_role_assignments gra
        JOIN role_permissions rp ON rp.role_id = gra.role_id
        WHERE gra.user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|(s,)| Permission::from_str(&s).ok())
        .collect())
}

/// Load org-scoped permissions for `(user_id, org_id)`.
pub async fn load_org_permissions(
    pool: &PgPool,
    user_id: Uuid,
    org_id: Uuid,
) -> Result<HashSet<Permission>> {
    let rows: Vec<(String,)> = sqlx::query_as(
        r#"
        SELECT rp.permission
        FROM org_role_assignments ora
        JOIN role_permissions rp ON rp.role_id = ora.role_id
        WHERE ora.user_id = $1 AND ora.org_id = $2
        "#,
    )
    .bind(user_id)
    .bind(org_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|(s,)| Permission::from_str(&s).ok())
        .collect())
}

/// Union of global perms + (if `org_id` is Some) org-scoped perms for that
/// org. Used by the Principal extractor.
pub async fn load_all_permissions(
    pool: &PgPool,
    user_id: Uuid,
    org_id: Option<Uuid>,
) -> Result<HashSet<Permission>> {
    let mut perms = load_global_permissions(pool, user_id).await?;
    if let Some(oid) = org_id {
        perms.extend(load_org_permissions(pool, user_id, oid).await?);
    }
    Ok(perms)
}

/// Load every pg-scoped grant the user holds inside `org_id`, indexed by
/// pg_id. Empty map when the user has no pg-scoped grants there. Used by
/// the Principal extractor to populate `pg_permissions`.
pub async fn load_pg_permissions_for_user_in_org(
    pool: &PgPool,
    user_id: Uuid,
    org_id: Uuid,
) -> Result<std::collections::HashMap<Uuid, HashSet<Permission>>> {
    let rows: Vec<(Uuid, String)> = sqlx::query_as(
        r#"
        SELECT pgra.process_group_id, rp.permission
          FROM process_group_role_assignments pgra
          JOIN role_permissions rp ON rp.role_id = pgra.role_id
         WHERE pgra.user_id = $1 AND pgra.org_id = $2
        "#,
    )
    .bind(user_id)
    .bind(org_id)
    .fetch_all(pool)
    .await?;
    let mut map: std::collections::HashMap<Uuid, HashSet<Permission>> =
        std::collections::HashMap::new();
    for (pg_id, perm) in rows {
        if let Ok(p) = Permission::from_str(&perm) {
            map.entry(pg_id).or_default().insert(p);
        }
    }
    Ok(map)
}

/// `true` iff the user has any global role assignment. Used to mark
/// platform admins in `/auth/me` and to bypass per-org membership checks.
pub async fn is_global_admin(pool: &PgPool, user_id: Uuid) -> Result<bool> {
    let (exists,): (bool,) =
        sqlx::query_as("SELECT EXISTS(SELECT 1 FROM global_role_assignments WHERE user_id = $1)")
            .bind(user_id)
            .fetch_one(pool)
            .await?;
    Ok(exists)
}

// ─── Global grants ───────────────────────────────────────────────────────────

/// Grant a global role to `user_id`. Idempotent (UNIQUE(user_id, role_id)).
///
/// `role_id` must be a built-in template (`roles.org_id IS NULL`). The DB
/// allows any role here, but global grants of org-scoped custom roles make
/// no sense — caller should validate.
pub async fn grant_global(
    pool: &PgPool,
    user_id: Uuid,
    role_id: Uuid,
    granted_by: Option<Uuid>,
) -> Result<Uuid> {
    // ON CONFLICT returns no row, so we use SELECT after.
    sqlx::query(
        r#"
        INSERT INTO global_role_assignments (user_id, role_id, granted_by)
        VALUES ($1, $2, $3)
        ON CONFLICT (user_id, role_id) DO NOTHING
        "#,
    )
    .bind(user_id)
    .bind(role_id)
    .bind(granted_by)
    .execute(pool)
    .await?;

    let (id,): (Uuid,) = sqlx::query_as(
        "SELECT id FROM global_role_assignments WHERE user_id = $1 AND role_id = $2",
    )
    .bind(user_id)
    .bind(role_id)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

/// Grant a global role by *name* (built-in templates). Returns Ok(true) if
/// the grant was inserted or already exists; Ok(false) if no built-in role
/// with that name was found.
pub async fn grant_global_by_name(
    pool: &PgPool,
    user_id: Uuid,
    role_name: &str,
    granted_by: Option<Uuid>,
) -> Result<bool> {
    let role_id: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM roles WHERE name = $1 AND org_id IS NULL")
            .bind(role_name)
            .fetch_optional(pool)
            .await?;

    let Some((rid,)) = role_id else {
        return Ok(false);
    };
    grant_global(pool, user_id, rid, granted_by).await?;
    Ok(true)
}

pub async fn revoke_global_by_id(pool: &PgPool, id: Uuid) -> Result<bool> {
    let res = sqlx::query("DELETE FROM global_role_assignments WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

// ─── Org grants ──────────────────────────────────────────────────────────────

/// Grant an org-scoped role. The `role_id` must be either a built-in
/// template (`roles.org_id IS NULL`) or a custom role belonging to the
/// SAME `org_id` as the grant. Returns a `Validation` error otherwise.
pub async fn grant_org(
    pool: &PgPool,
    user_id: Uuid,
    role_id: Uuid,
    org_id: Uuid,
    granted_by: Option<Uuid>,
    granted_in_org_id: Option<Uuid>,
) -> Result<Uuid> {
    // Validate the role is grantable in this org.
    let role: Option<(Option<Uuid>,)> = sqlx::query_as("SELECT org_id FROM roles WHERE id = $1")
        .bind(role_id)
        .fetch_optional(pool)
        .await?;
    let Some((role_org,)) = role else {
        return Err(EngineError::NotFound(format!("role {role_id}")));
    };
    if let Some(ro) = role_org {
        if ro != org_id {
            return Err(EngineError::Validation(format!(
                "role {role_id} is scoped to a different org and cannot be granted in {org_id}"
            )));
        }
    }

    // Membership precondition. Caller can decide whether to auto-create
    // the org_members row; we require it explicitly here for safety.
    let is_member: (bool,) = sqlx::query_as(
        "SELECT EXISTS(SELECT 1 FROM org_members WHERE user_id = $1 AND org_id = $2)",
    )
    .bind(user_id)
    .bind(org_id)
    .fetch_one(pool)
    .await?;
    if !is_member.0 {
        return Err(EngineError::Validation(format!(
            "user {user_id} is not a member of org {org_id}"
        )));
    }

    sqlx::query(
        r#"
        INSERT INTO org_role_assignments (user_id, role_id, org_id, granted_by, granted_in_org_id)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (user_id, role_id, org_id) DO NOTHING
        "#,
    )
    .bind(user_id)
    .bind(role_id)
    .bind(org_id)
    .bind(granted_by)
    .bind(granted_in_org_id)
    .execute(pool)
    .await?;

    let (id,): (Uuid,) = sqlx::query_as(
        r#"
        SELECT id FROM org_role_assignments
        WHERE user_id = $1 AND role_id = $2 AND org_id = $3
        "#,
    )
    .bind(user_id)
    .bind(role_id)
    .bind(org_id)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

/// Revoke a specific org-role grant by its primary-key id; scoped to
/// `org_id` so callers can't revoke grants belonging to another org.
pub async fn revoke_org_by_id(pool: &PgPool, id: Uuid, org_id: Uuid) -> Result<bool> {
    let res = sqlx::query("DELETE FROM org_role_assignments WHERE id = $1 AND org_id = $2")
        .bind(id)
        .bind(org_id)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

// ─── Listings ────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct OrgRoleAssignment {
    pub id: Uuid,
    pub user_id: Uuid,
    pub role_id: Uuid,
    pub org_id: Uuid,
    pub granted_by: Option<Uuid>,
    pub granted_in_org_id: Option<Uuid>,
    pub granted_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct GlobalRoleAssignment {
    pub id: Uuid,
    pub user_id: Uuid,
    pub role_id: Uuid,
    pub granted_by: Option<Uuid>,
    pub granted_at: DateTime<Utc>,
}

pub async fn list_for_org(pool: &PgPool, org_id: Uuid) -> Result<Vec<OrgRoleAssignment>> {
    let rows = sqlx::query_as::<_, OrgRoleAssignment>(
        r#"
        SELECT id, user_id, role_id, org_id, granted_by, granted_in_org_id, granted_at
        FROM org_role_assignments
        WHERE org_id = $1
        ORDER BY granted_at ASC
        "#,
    )
    .bind(org_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn list_for_user_in_org(
    pool: &PgPool,
    user_id: Uuid,
    org_id: Uuid,
) -> Result<Vec<OrgRoleAssignment>> {
    let rows = sqlx::query_as::<_, OrgRoleAssignment>(
        r#"
        SELECT id, user_id, role_id, org_id, granted_by, granted_in_org_id, granted_at
        FROM org_role_assignments
        WHERE user_id = $1 AND org_id = $2
        ORDER BY granted_at ASC
        "#,
    )
    .bind(user_id)
    .bind(org_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn list_global(pool: &PgPool) -> Result<Vec<GlobalRoleAssignment>> {
    let rows = sqlx::query_as::<_, GlobalRoleAssignment>(
        r#"
        SELECT id, user_id, role_id, granted_by, granted_at
        FROM global_role_assignments
        ORDER BY granted_at ASC
        "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn list_global_for_user(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<GlobalRoleAssignment>> {
    let rows = sqlx::query_as::<_, GlobalRoleAssignment>(
        r#"
        SELECT id, user_id, role_id, granted_by, granted_at
        FROM global_role_assignments
        WHERE user_id = $1
        ORDER BY granted_at ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Roles a user has in a given org (names, for /auth/me-style display).
pub async fn role_names_for_user_in_org(
    pool: &PgPool,
    user_id: Uuid,
    org_id: Uuid,
) -> Result<Vec<String>> {
    let rows: Vec<(String,)> = sqlx::query_as(
        r#"
        SELECT r.name
        FROM org_role_assignments ora
        JOIN roles r ON r.id = ora.role_id
        WHERE ora.user_id = $1 AND ora.org_id = $2
        ORDER BY r.name
        "#,
    )
    .bind(user_id)
    .bind(org_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|(n,)| n).collect())
}

/// Global role names for a user.
pub async fn global_role_names_for_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<String>> {
    let rows: Vec<(String,)> = sqlx::query_as(
        r#"
        SELECT r.name
        FROM global_role_assignments gra
        JOIN roles r ON r.id = gra.role_id
        WHERE gra.user_id = $1
        ORDER BY r.name
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|(n,)| n).collect())
}

// ─── PG grants ───────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct PgRoleAssignment {
    pub id: Uuid,
    pub user_id: Uuid,
    pub role_id: Uuid,
    pub process_group_id: Uuid,
    pub org_id: Uuid,
    pub granted_by: Option<Uuid>,
    pub granted_at: DateTime<Utc>,
}

/// One pg-level role grant a user holds in some org. Used for `/me` so the
/// UI can render "Developer in HR Workflows".
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct PgRoleNameRow {
    pub process_group_id: Uuid,
    pub process_group_name: String,
    pub role_name: String,
}

/// Grant a pg-scoped role.
///
/// Validations (any failure is `Validation`, except role missing → `NotFound`):
///   1. role exists; if custom, role.org_id must match the pg's org.
///   2. every permission on the role must be `is_pg_scopable` (else the
///      grant could leak an org-only permission like `secret.read_plaintext`
///      to a non-member through cascade).
///   3. user must be a member of the pg's org (otherwise the composite FK
///      to org_members would raise an FK error; this gives a friendlier
///      message).
///
/// Idempotent (UNIQUE(user_id, role_id, process_group_id)).
pub async fn grant_process_group(
    pool: &PgPool,
    user_id: Uuid,
    role_id: Uuid,
    process_group_id: Uuid,
    granted_by: Option<Uuid>,
) -> Result<Uuid> {
    // 1. Resolve the pg's org_id and the role's org_id.
    let pg: Option<(Uuid,)> = sqlx::query_as("SELECT org_id FROM process_groups WHERE id = $1")
        .bind(process_group_id)
        .fetch_optional(pool)
        .await?;
    let Some((pg_org_id,)) = pg else {
        return Err(EngineError::NotFound(format!(
            "process group {process_group_id}"
        )));
    };

    let role: Option<(Option<Uuid>,)> = sqlx::query_as("SELECT org_id FROM roles WHERE id = $1")
        .bind(role_id)
        .fetch_optional(pool)
        .await?;
    let Some((role_org,)) = role else {
        return Err(EngineError::NotFound(format!("role {role_id}")));
    };
    if let Some(ro) = role_org {
        if ro != pg_org_id {
            return Err(EngineError::Validation(format!(
                "role {role_id} is scoped to a different org and cannot be granted in process_group {process_group_id}"
            )));
        }
    }

    // 2. Reject if any of the role's perms is org-only.
    let perm_rows: Vec<(String,)> =
        sqlx::query_as("SELECT permission FROM role_permissions WHERE role_id = $1")
            .bind(role_id)
            .fetch_all(pool)
            .await?;
    for (s,) in &perm_rows {
        if let Ok(p) = Permission::from_str(s) {
            if !p.is_pg_scopable() {
                return Err(EngineError::Validation(format!(
                    "permission `{p}` cannot be granted at process-group scope (org-only)"
                )));
            }
        }
    }

    // 3. Membership precondition (the composite FK would catch this too).
    let is_member: (bool,) = sqlx::query_as(
        "SELECT EXISTS(SELECT 1 FROM org_members WHERE user_id = $1 AND org_id = $2)",
    )
    .bind(user_id)
    .bind(pg_org_id)
    .fetch_one(pool)
    .await?;
    if !is_member.0 {
        return Err(EngineError::Validation(format!(
            "user {user_id} is not a member of org {pg_org_id}"
        )));
    }

    sqlx::query(
        r#"
        INSERT INTO process_group_role_assignments
            (user_id, role_id, process_group_id, org_id, granted_by)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (user_id, role_id, process_group_id) DO NOTHING
        "#,
    )
    .bind(user_id)
    .bind(role_id)
    .bind(process_group_id)
    .bind(pg_org_id)
    .bind(granted_by)
    .execute(pool)
    .await?;

    let (id,): (Uuid,) = sqlx::query_as(
        r#"
        SELECT id FROM process_group_role_assignments
         WHERE user_id = $1 AND role_id = $2 AND process_group_id = $3
        "#,
    )
    .bind(user_id)
    .bind(role_id)
    .bind(process_group_id)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

pub async fn revoke_pg_by_id(pool: &PgPool, id: Uuid, process_group_id: Uuid) -> Result<bool> {
    let res = sqlx::query(
        "DELETE FROM process_group_role_assignments WHERE id = $1 AND process_group_id = $2",
    )
    .bind(id)
    .bind(process_group_id)
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn list_for_process_group(
    pool: &PgPool,
    process_group_id: Uuid,
) -> Result<Vec<PgRoleAssignment>> {
    let rows = sqlx::query_as::<_, PgRoleAssignment>(
        r#"
        SELECT id, user_id, role_id, process_group_id, org_id, granted_by, granted_at
          FROM process_group_role_assignments
         WHERE process_group_id = $1
         ORDER BY granted_at ASC
        "#,
    )
    .bind(process_group_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn list_pg_for_user_in_org(
    pool: &PgPool,
    user_id: Uuid,
    org_id: Uuid,
) -> Result<Vec<PgRoleAssignment>> {
    let rows = sqlx::query_as::<_, PgRoleAssignment>(
        r#"
        SELECT id, user_id, role_id, process_group_id, org_id, granted_by, granted_at
          FROM process_group_role_assignments
         WHERE user_id = $1 AND org_id = $2
         ORDER BY granted_at ASC
        "#,
    )
    .bind(user_id)
    .bind(org_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// `(pg_id, pg_name, role_name)` rows for `/me`. Ordered by pg_name, role_name.
pub async fn pg_role_names_for_user_in_org(
    pool: &PgPool,
    user_id: Uuid,
    org_id: Uuid,
) -> Result<Vec<PgRoleNameRow>> {
    let rows = sqlx::query_as::<_, PgRoleNameRow>(
        r#"
        SELECT pg.id   AS process_group_id,
               pg.name AS process_group_name,
               r.name  AS role_name
          FROM process_group_role_assignments pgra
          JOIN process_groups pg ON pg.id = pgra.process_group_id
          JOIN roles          r  ON r.id  = pgra.role_id
         WHERE pgra.user_id = $1 AND pgra.org_id = $2
         ORDER BY pg.name, r.name
        "#,
    )
    .bind(user_id)
    .bind(org_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
