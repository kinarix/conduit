use sqlx::PgPool;

use crate::db::models::Org;
use crate::error::Result;

pub async fn get_by_id(pool: &PgPool, id: uuid::Uuid) -> Result<Option<Org>> {
    let row = sqlx::query_as::<_, Org>("SELECT * FROM orgs WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn get_by_slug(pool: &PgPool, slug: &str) -> Result<Option<Org>> {
    let row = sqlx::query_as::<_, Org>("SELECT * FROM orgs WHERE slug = $1")
        .bind(slug)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn list_all(pool: &PgPool) -> Result<Vec<Org>> {
    let rows = sqlx::query_as::<_, Org>("SELECT * FROM orgs ORDER BY created_at DESC")
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

pub async fn list_paginated(pool: &PgPool, limit: i64, offset: i64) -> Result<(Vec<Org>, i64)> {
    let rows =
        sqlx::query_as::<_, Org>("SELECT * FROM orgs ORDER BY created_at DESC LIMIT $1 OFFSET $2")
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;
    let (total,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM orgs")
        .fetch_one(pool)
        .await?;
    Ok((rows, total))
}

/// List the orgs a user is a member of.
pub async fn list_for_user(pool: &PgPool, user_id: uuid::Uuid) -> Result<Vec<Org>> {
    let rows = sqlx::query_as::<_, Org>(
        r#"
        SELECT o.*
        FROM orgs o
        JOIN org_members m ON m.org_id = o.id
        WHERE m.user_id = $1
        ORDER BY o.created_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

// New orgs always start with `setup_completed = FALSE`. The Org Owner's first
// sign-in then drives the org / process-group / first-process wizard, which
// flips this to TRUE on completion.
pub async fn insert(pool: &PgPool, name: &str, slug: &str) -> Result<Org> {
    insert_with_contacts(pool, name, slug, NewOrgContacts::default()).await
}

/// Extended variant of `insert` used by the platform-admin create-org
/// endpoint, where the operator can capture admin/support contacts and
/// a short description up-front. Tests and other call sites that don't
/// care about contact metadata go through `insert`.
pub async fn insert_with_contacts(
    pool: &PgPool,
    name: &str,
    slug: &str,
    contacts: NewOrgContacts<'_>,
) -> Result<Org> {
    let row = sqlx::query_as::<_, Org>(
        r#"
        INSERT INTO orgs (name, slug, setup_completed, admin_email, admin_name, support_email, description)
        VALUES ($1, $2, FALSE, $3, $4, $5, $6)
        RETURNING *
        "#,
    )
    .bind(name)
    .bind(slug)
    .bind(contacts.admin_email)
    .bind(contacts.admin_name)
    .bind(contacts.support_email)
    .bind(contacts.description)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// Optional create-time metadata for an org. Each field is independently
/// nullable; callers that don't care can pass `NewOrgContacts::default()`.
#[derive(Debug, Default, Clone, Copy)]
pub struct NewOrgContacts<'a> {
    pub admin_email: Option<&'a str>,
    pub admin_name: Option<&'a str>,
    pub support_email: Option<&'a str>,
    pub description: Option<&'a str>,
}

pub async fn set_setup_completed(pool: &PgPool, id: uuid::Uuid, completed: bool) -> Result<()> {
    sqlx::query("UPDATE orgs SET setup_completed = $1 WHERE id = $2")
        .bind(completed)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_name(pool: &PgPool, id: uuid::Uuid, name: &str) -> Result<Org> {
    let row = sqlx::query_as::<_, Org>("UPDATE orgs SET name = $1 WHERE id = $2 RETURNING *")
        .bind(name)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| crate::error::EngineError::NotFound(format!("org {id} not found")))?;
    Ok(row)
}

/// PATCH for the optional contact / description columns. Each `Some` is
/// applied (use `Some(None)` semantics by passing an `Option<Option<&str>>`
/// in a future iteration if we ever need to clear *some* fields without
/// touching others; today the admin UI sends every field on every save).
pub async fn update_contacts(
    pool: &PgPool,
    id: uuid::Uuid,
    admin_email: Option<&str>,
    admin_name: Option<&str>,
    support_email: Option<&str>,
    description: Option<&str>,
) -> Result<Org> {
    let row = sqlx::query_as::<_, Org>(
        r#"
        UPDATE orgs
        SET admin_email = $2,
            admin_name = $3,
            support_email = $4,
            description = $5
        WHERE id = $1
        RETURNING *
        "#,
    )
    .bind(id)
    .bind(admin_email)
    .bind(admin_name)
    .bind(support_email)
    .bind(description)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| crate::error::EngineError::NotFound(format!("org {id} not found")))?;
    Ok(row)
}

/// Counts of every entity that could keep an org "non-empty" — the
/// platform-admin delete flow shows these to the operator and refuses
/// the delete if any are non-zero. Returned as `i64` because that's what
/// PG's `count(*)` yields; callers can compare against 0 directly.
#[derive(Debug, Clone, serde::Serialize)]
pub struct OrgStats {
    pub members: i64,
    pub processes: i64,
    pub decisions: i64,
    pub instances: i64,
}

impl OrgStats {
    pub fn is_empty(&self) -> bool {
        self.members == 0 && self.processes == 0 && self.decisions == 0 && self.instances == 0
    }
}

pub async fn stats(pool: &PgPool, id: uuid::Uuid) -> Result<OrgStats> {
    // One round-trip via correlated scalar subqueries — cheaper than
    // four separate count(*)s, and easier than aggregating per-table.
    // All four tables have an `org_id` column.
    let (members, processes, decisions, instances): (i64, i64, i64, i64) = sqlx::query_as(
        r#"
        SELECT
            (SELECT COUNT(*) FROM org_members            WHERE org_id = $1),
            (SELECT COUNT(*) FROM process_definitions    WHERE org_id = $1),
            (SELECT COUNT(*) FROM decision_definitions   WHERE org_id = $1),
            (SELECT COUNT(*) FROM process_instances      WHERE org_id = $1)
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await?;
    Ok(OrgStats { members, processes, decisions, instances })
}

pub async fn delete(pool: &PgPool, id: uuid::Uuid) -> Result<()> {
    let res = sqlx::query("DELETE FROM orgs WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(crate::error::EngineError::NotFound(format!(
            "org {id} not found"
        )));
    }
    Ok(())
}
