use std::collections::HashSet;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::permission::Permission;
use crate::error::Result;

/// Load every permission held by `user_id` (union across all assigned roles).
pub async fn load_user_permissions(pool: &PgPool, user_id: Uuid) -> Result<HashSet<Permission>> {
    let rows = sqlx::query_scalar!(
        r#"
        SELECT rp.permission
        FROM user_roles ur
        JOIN role_permissions rp ON rp.role_id = ur.role_id
        WHERE ur.user_id = $1
        "#,
        user_id
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .filter_map(|s| Permission::from_str(&s).ok())
        .collect())
}

/// Grant the global `Admin` role to `user_id` (self-grant: `granted_by = user_id`).
/// Idempotent via `ON CONFLICT DO NOTHING`.
pub async fn assign_admin(pool: &PgPool, user_id: Uuid) -> Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO user_roles (user_id, role_id, granted_by)
        SELECT $1, r.id, $1
        FROM roles r
        WHERE r.name = 'Admin' AND r.org_id IS NULL
        ON CONFLICT DO NOTHING
        "#,
        user_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Assign a global built-in role by name to `user_id`.
/// Returns `false` if the role name is not found.
pub async fn assign_role(
    pool: &PgPool,
    user_id: Uuid,
    role_name: &str,
    granted_by: Uuid,
) -> Result<bool> {
    let result = sqlx::query!(
        r#"
        INSERT INTO user_roles (user_id, role_id, granted_by)
        SELECT $1, r.id, $3
        FROM roles r
        WHERE r.name = $2 AND r.org_id IS NULL
        ON CONFLICT DO NOTHING
        "#,
        user_id,
        role_name,
        granted_by
    )
    .execute(pool)
    .await?;
    // rows_affected = 0 either means conflict (already assigned) or role not found.
    // We distinguish by checking if the role exists at all.
    if result.rows_affected() == 0 {
        let exists = sqlx::query_scalar!(
            "SELECT EXISTS(SELECT 1 FROM roles WHERE name = $1 AND org_id IS NULL)",
            role_name
        )
        .fetch_one(pool)
        .await?
        .unwrap_or(false);
        return Ok(exists); // true = already assigned (idempotent OK), false = no such role
    }
    Ok(true)
}

/// Revoke a role from a user, scoped to callers that share the same org.
/// Returns `true` if a row was deleted, `false` if not found or out-of-org.
pub async fn revoke_role(
    pool: &PgPool,
    target_user_id: Uuid,
    role_id: Uuid,
    requester_org_id: Uuid,
) -> Result<bool> {
    let result = sqlx::query!(
        r#"
        DELETE FROM user_roles ur
        USING users u
        WHERE ur.user_id   = $1
          AND ur.role_id   = $2
          AND u.id         = ur.user_id
          AND u.org_id     = $3
        "#,
        target_user_id,
        role_id,
        requester_org_id,
    )
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

// ─── Read queries ─────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct RoleWithPermissions {
    pub id: Uuid,
    pub name: String,
    pub permissions: Vec<String>,
}

/// List all global built-in roles with their permission sets.
pub async fn list_global_roles(pool: &PgPool) -> Result<Vec<RoleWithPermissions>> {
    // Two-query approach to avoid array_agg NULL subtleties.
    let roles = sqlx::query!(
        "SELECT id, name FROM roles WHERE org_id IS NULL ORDER BY name"
    )
    .fetch_all(pool)
    .await?;

    let mut result = Vec::with_capacity(roles.len());
    for r in roles {
        let perms = sqlx::query_scalar!(
            "SELECT permission FROM role_permissions WHERE role_id = $1 ORDER BY permission",
            r.id
        )
        .fetch_all(pool)
        .await?;
        result.push(RoleWithPermissions {
            id: r.id,
            name: r.name,
            permissions: perms,
        });
    }
    Ok(result)
}

#[derive(Debug, Serialize)]
pub struct UserRoleRow {
    pub role_id: Uuid,
    pub role_name: String,
    pub granted_by: Option<Uuid>,
    pub granted_at: DateTime<Utc>,
}

/// List all roles assigned to `user_id`.
pub async fn list_user_roles(pool: &PgPool, user_id: Uuid) -> Result<Vec<UserRoleRow>> {
    let rows = sqlx::query!(
        r#"
        SELECT r.id AS role_id, r.name AS role_name,
               ur.granted_by, ur.granted_at
        FROM user_roles ur
        JOIN roles r ON r.id = ur.role_id
        WHERE ur.user_id = $1
        ORDER BY r.name
        "#,
        user_id
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| UserRoleRow {
            role_id: r.role_id,
            role_name: r.role_name,
            granted_by: r.granted_by,
            granted_at: r.granted_at,
        })
        .collect())
}
