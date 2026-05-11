//! Role *definitions*. Grants live in `db::role_assignments`.
//!
//! Built-in role templates have `org_id IS NULL` and are seeded by
//! migration 031. Custom org-scoped roles have `org_id = <org>`.

use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::Result;

#[derive(Debug, Serialize)]
pub struct RoleWithPermissions {
    pub id: Uuid,
    pub name: String,
    /// `None` = global built-in; `Some(id)` = custom role scoped to that org.
    pub org_id: Option<Uuid>,
    pub permissions: Vec<String>,
}

/// All roles visible to an org: global built-ins + the org's custom roles.
pub async fn list_for_org(pool: &PgPool, org_id: Uuid) -> Result<Vec<RoleWithPermissions>> {
    let roles: Vec<(Uuid, String, Option<Uuid>)> = sqlx::query_as(
        "SELECT id, name, org_id FROM roles WHERE org_id IS NULL OR org_id = $1 ORDER BY name",
    )
    .bind(org_id)
    .fetch_all(pool)
    .await?;

    let mut result = Vec::with_capacity(roles.len());
    for (id, name, oid) in roles {
        let perms: Vec<(String,)> = sqlx::query_as(
            "SELECT permission FROM role_permissions WHERE role_id = $1 ORDER BY permission",
        )
        .bind(id)
        .fetch_all(pool)
        .await?;
        result.push(RoleWithPermissions {
            id,
            name,
            org_id: oid,
            permissions: perms.into_iter().map(|(p,)| p).collect(),
        });
    }
    Ok(result)
}

/// Global built-in roles only.
pub async fn list_global(pool: &PgPool) -> Result<Vec<RoleWithPermissions>> {
    let roles: Vec<(Uuid, String)> =
        sqlx::query_as("SELECT id, name FROM roles WHERE org_id IS NULL ORDER BY name")
            .fetch_all(pool)
            .await?;

    let mut result = Vec::with_capacity(roles.len());
    for (id, name) in roles {
        let perms: Vec<(String,)> = sqlx::query_as(
            "SELECT permission FROM role_permissions WHERE role_id = $1 ORDER BY permission",
        )
        .bind(id)
        .fetch_all(pool)
        .await?;
        result.push(RoleWithPermissions {
            id,
            name,
            org_id: None,
            permissions: perms.into_iter().map(|(p,)| p).collect(),
        });
    }
    Ok(result)
}

/// Look up a built-in role by name (org_id IS NULL).
pub async fn find_builtin_by_name(pool: &PgPool, name: &str) -> Result<Option<Uuid>> {
    let row: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM roles WHERE org_id IS NULL AND name = $1")
            .bind(name)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(|(id,)| id))
}

/// Create a custom org-scoped role with the given permission set.
pub async fn create_custom_role(
    pool: &PgPool,
    org_id: Uuid,
    name: &str,
    permissions: &[String],
) -> Result<RoleWithPermissions> {
    let mut tx = pool.begin().await?;

    let (role_id, role_name): (Uuid, String) =
        sqlx::query_as("INSERT INTO roles (org_id, name) VALUES ($1, $2) RETURNING id, name")
            .bind(org_id)
            .bind(name)
            .fetch_one(&mut *tx)
            .await?;

    for perm in permissions {
        sqlx::query("INSERT INTO role_permissions (role_id, permission) VALUES ($1, $2)")
            .bind(role_id)
            .bind(perm)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;

    Ok(RoleWithPermissions {
        id: role_id,
        name: role_name,
        org_id: Some(org_id),
        permissions: permissions.to_vec(),
    })
}

/// Rename + replace permission set atomically. Errors with `NotFound` if
/// the role is not a custom role of `org_id`.
pub async fn update_custom_role(
    pool: &PgPool,
    role_id: Uuid,
    org_id: Uuid,
    name: &str,
    permissions: &[String],
) -> Result<RoleWithPermissions> {
    let mut tx = pool.begin().await?;

    let row: Option<(Uuid, String)> = sqlx::query_as(
        "UPDATE roles SET name = $2 WHERE id = $1 AND org_id = $3 RETURNING id, name",
    )
    .bind(role_id)
    .bind(name)
    .bind(org_id)
    .fetch_optional(&mut *tx)
    .await?;
    let (id, name) = row.ok_or_else(|| {
        crate::error::EngineError::NotFound(format!("role {role_id} (must be a custom org role)"))
    })?;

    sqlx::query("DELETE FROM role_permissions WHERE role_id = $1")
        .bind(role_id)
        .execute(&mut *tx)
        .await?;

    for perm in permissions {
        sqlx::query("INSERT INTO role_permissions (role_id, permission) VALUES ($1, $2)")
            .bind(role_id)
            .bind(perm)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;

    Ok(RoleWithPermissions {
        id,
        name,
        org_id: Some(org_id),
        permissions: permissions.to_vec(),
    })
}

/// Delete a custom org-scoped role. Returns false if it doesn't exist or is
/// a built-in (`org_id IS NULL`).
pub async fn delete_custom_role(pool: &PgPool, role_id: Uuid, org_id: Uuid) -> Result<bool> {
    let res = sqlx::query("DELETE FROM roles WHERE id = $1 AND org_id = $2")
        .bind(role_id)
        .bind(org_id)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}
