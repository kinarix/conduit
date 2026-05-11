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
/// Returns an error if the global Admin role does not exist (missing migration seed data).
pub async fn assign_admin(pool: &PgPool, user_id: Uuid) -> Result<()> {
    let result = sqlx::query!(
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

    if result.rows_affected() == 0 {
        // 0 rows means either the Admin role doesn't exist (missing seed data)
        // or the user already has it. Verify the role exists to tell them apart.
        let admin_exists = sqlx::query_scalar!(
            "SELECT EXISTS(SELECT 1 FROM roles WHERE name = 'Admin' AND org_id IS NULL)"
        )
        .fetch_one(pool)
        .await?
        .unwrap_or(false);

        if !admin_exists {
            return Err(crate::error::EngineError::NotFound(
                "global Admin role not found — run `make db-reset && make migrate` to reseed built-in roles".to_string(),
            ));
        }
        // Otherwise user already has the Admin role — idempotent OK.
    }
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
    /// `None` = global built-in; `Some(id)` = custom role scoped to that org.
    /// The UI uses this to distinguish built-ins (immutable) from custom roles.
    pub org_id: Option<Uuid>,
    pub permissions: Vec<String>,
}

/// List all roles visible to an org: global built-ins + the org's custom roles.
pub async fn list_for_org(pool: &PgPool, org_id: Uuid) -> Result<Vec<RoleWithPermissions>> {
    let roles = sqlx::query!(
        "SELECT id, name, org_id FROM roles WHERE org_id IS NULL OR org_id = $1 ORDER BY name",
        org_id
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
            org_id: r.org_id,
            permissions: perms,
        });
    }
    Ok(result)
}

/// Create a custom org-scoped role with the given permission set.
pub async fn create_custom_role(
    pool: &PgPool,
    org_id: Uuid,
    name: &str,
    permissions: &[String],
) -> Result<RoleWithPermissions> {
    let role = sqlx::query!(
        "INSERT INTO roles (org_id, name) VALUES ($1, $2) RETURNING id, name",
        org_id,
        name
    )
    .fetch_one(pool)
    .await?;

    for perm in permissions {
        sqlx::query!(
            "INSERT INTO role_permissions (role_id, permission) VALUES ($1, $2)",
            role.id,
            perm
        )
        .execute(pool)
        .await?;
    }

    Ok(RoleWithPermissions {
        id: role.id,
        name: role.name,
        org_id: Some(org_id),
        permissions: permissions.to_vec(),
    })
}

/// Update a custom org-scoped role: rename and replace its permission set atomically.
/// Returns `EngineError::NotFound` if the role does not exist or is a built-in (org_id IS NULL).
pub async fn update_custom_role(
    pool: &PgPool,
    role_id: Uuid,
    org_id: Uuid,
    name: &str,
    permissions: &[String],
) -> Result<RoleWithPermissions> {
    let mut tx = pool.begin().await?;

    let row = sqlx::query!(
        "UPDATE roles SET name = $2 WHERE id = $1 AND org_id = $3 RETURNING id, name",
        role_id,
        name,
        org_id
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| {
        crate::error::EngineError::NotFound(format!(
            "role {role_id} (must be a custom org role)"
        ))
    })?;

    sqlx::query!("DELETE FROM role_permissions WHERE role_id = $1", role_id)
        .execute(&mut *tx)
        .await?;

    for perm in permissions {
        sqlx::query!(
            "INSERT INTO role_permissions (role_id, permission) VALUES ($1, $2)",
            role_id,
            perm
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(RoleWithPermissions {
        id: row.id,
        name: row.name,
        org_id: Some(org_id),
        permissions: permissions.to_vec(),
    })
}

/// Delete a custom org-scoped role. Returns false if not found or is a built-in (org_id IS NULL).
pub async fn delete_custom_role(pool: &PgPool, role_id: Uuid, org_id: Uuid) -> Result<bool> {
    let res = sqlx::query!(
        "DELETE FROM roles WHERE id = $1 AND org_id = $2",
        role_id,
        org_id
    )
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

/// Replace all roles for `user_id` (within `org_id`) with the given role IDs.
/// All supplied role_ids must be either global or belong to the same org.
pub async fn set_user_roles(
    pool: &PgPool,
    user_id: Uuid,
    org_id: Uuid,
    role_ids: &[Uuid],
) -> Result<()> {
    let mut tx = pool.begin().await?;

    // Delete all current roles for this user
    sqlx::query!("DELETE FROM user_roles WHERE user_id = $1", user_id)
        .execute(&mut *tx)
        .await?;

    for &role_id in role_ids {
        // Verify each role is accessible (global or org-scoped to this org)
        let ok = sqlx::query_scalar!(
            "SELECT EXISTS(SELECT 1 FROM roles WHERE id = $1 AND (org_id IS NULL OR org_id = $2))",
            role_id,
            org_id
        )
        .fetch_one(&mut *tx)
        .await?
        .unwrap_or(false);

        if !ok {
            tx.rollback().await?;
            return Err(crate::error::EngineError::NotFound(format!(
                "role {role_id} not found or not accessible"
            )));
        }

        sqlx::query!(
            "INSERT INTO user_roles (user_id, role_id, granted_by) VALUES ($1, $2, $1) ON CONFLICT DO NOTHING",
            user_id,
            role_id
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
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
            org_id: None,
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
