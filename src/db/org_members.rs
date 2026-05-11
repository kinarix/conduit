//! `org_members` — explicit user-belongs-to-org relation.
//!
//! Membership is the precondition for any org-scoped role grant (FK from
//! `org_role_assignments(user_id, org_id)` plus checks in
//! `db::role_assignments`).
//!
//! A row here without any role assignments is meaningful: it represents an
//! invited user who has no permissions in the org yet.

use sqlx::PgPool;
use uuid::Uuid;

use crate::db::models::OrgMember;
use crate::error::Result;

/// Add `user_id` as a member of `org_id`. Idempotent.
pub async fn insert(
    pool: &PgPool,
    user_id: Uuid,
    org_id: Uuid,
    invited_by: Option<Uuid>,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO org_members (user_id, org_id, invited_by)
        VALUES ($1, $2, $3)
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(user_id)
    .bind(org_id)
    .bind(invited_by)
    .execute(pool)
    .await?;
    Ok(())
}

/// Remove `user_id` from `org_id`. CASCADE wipes their `org_role_assignments`
/// rows in that org via the FK in migration 030. Returns `true` if a row was
/// deleted.
pub async fn delete(pool: &PgPool, user_id: Uuid, org_id: Uuid) -> Result<bool> {
    let res = sqlx::query("DELETE FROM org_members WHERE user_id = $1 AND org_id = $2")
        .bind(user_id)
        .bind(org_id)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

/// `true` iff `user_id` is a member of `org_id`.
pub async fn exists(pool: &PgPool, user_id: Uuid, org_id: Uuid) -> Result<bool> {
    let (exists,): (bool,) = sqlx::query_as(
        "SELECT EXISTS(SELECT 1 FROM org_members WHERE user_id = $1 AND org_id = $2)",
    )
    .bind(user_id)
    .bind(org_id)
    .fetch_one(pool)
    .await?;
    Ok(exists)
}

/// Members of `org_id`, joined with their joined_at timestamp.
pub async fn list_by_org(pool: &PgPool, org_id: Uuid) -> Result<Vec<OrgMember>> {
    let rows = sqlx::query_as::<_, OrgMember>(
        r#"
        SELECT user_id, org_id, invited_by, joined_at
        FROM org_members
        WHERE org_id = $1
        ORDER BY joined_at ASC
        "#,
    )
    .bind(org_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Orgs a user is a member of (raw rows; use `db::orgs::list_for_user` for
/// the join to `orgs`).
pub async fn list_org_ids_for_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<Uuid>> {
    let rows: Vec<(Uuid,)> = sqlx::query_as("SELECT org_id FROM org_members WHERE user_id = $1")
        .bind(user_id)
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}
