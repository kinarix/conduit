use sqlx::PgPool;
use uuid::Uuid;

use crate::db::models::DecisionDefinition;
use crate::error::{EngineError, Result};

/// Insert a new decision definition, auto-incrementing the version per (org_id, decision_key).
pub async fn deploy(
    pool: &PgPool,
    org_id: Uuid,
    process_group_id: Option<Uuid>,
    decision_key: &str,
    name: Option<&str>,
    dmn_xml: &str,
) -> Result<DecisionDefinition> {
    let row = sqlx::query_as::<_, DecisionDefinition>(
        "INSERT INTO decision_definitions (org_id, process_group_id, decision_key, version, name, dmn_xml)
         VALUES (
             $1, $2, $3,
             COALESCE(
                 (SELECT MAX(version) FROM decision_definitions
                  WHERE org_id = $1 AND decision_key = $3),
                 0
             ) + 1,
             $4, $5
         )
         RETURNING *",
    )
    .bind(org_id)
    .bind(process_group_id)
    .bind(decision_key)
    .bind(name)
    .bind(dmn_xml)
    .fetch_one(pool)
    .await?;

    Ok(row)
}

/// Fetch the latest version of a decision for an org.
pub async fn get_latest(
    pool: &PgPool,
    org_id: Uuid,
    decision_key: &str,
) -> Result<DecisionDefinition> {
    sqlx::query_as::<_, DecisionDefinition>(
        "SELECT * FROM decision_definitions
         WHERE org_id = $1 AND decision_key = $2
         ORDER BY version DESC
         LIMIT 1",
    )
    .bind(org_id)
    .bind(decision_key)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| EngineError::DmnNotFound(format!("Decision '{decision_key}' not found")))
}

/// Delete all versions of a decision for an org.
/// Returns an error if the key is referenced in another decision's DRD or a process's BusinessRuleTask.
pub async fn delete(pool: &PgPool, org_id: Uuid, decision_key: &str) -> Result<()> {
    // Check DRD references in other decisions
    let drd_ref: Option<(String,)> = sqlx::query_as(
        "SELECT decision_key FROM decision_definitions
         WHERE org_id = $1 AND decision_key != $2
           AND strpos(dmn_xml, 'href=\"#' || $2 || '\"') > 0
         LIMIT 1",
    )
    .bind(org_id)
    .bind(decision_key)
    .fetch_optional(pool)
    .await?;

    if let Some((ref_key,)) = drd_ref {
        return Err(EngineError::Validation(format!(
            "Decision '{decision_key}' is required by decision '{ref_key}'"
        )));
    }

    // Check BusinessRuleTask references in process definitions (accept both namespaces)
    let brt_ref: Option<(String,)> = sqlx::query_as(
        "SELECT process_key FROM process_definitions
         WHERE org_id = $1
           AND (strpos(bpmn_xml, 'conduit:decisionRef=\"' || $2 || '\"') > 0
                OR strpos(bpmn_xml, 'camunda:decisionRef=\"' || $2 || '\"') > 0)
         LIMIT 1",
    )
    .bind(org_id)
    .bind(decision_key)
    .fetch_optional(pool)
    .await?;

    if let Some((proc_key,)) = brt_ref {
        return Err(EngineError::Validation(format!(
            "Decision '{decision_key}' is referenced by process '{proc_key}'"
        )));
    }

    sqlx::query("DELETE FROM decision_definitions WHERE org_id = $1 AND decision_key = $2")
        .bind(org_id)
        .bind(decision_key)
        .execute(pool)
        .await?;

    Ok(())
}

/// Rename every version of a decision to `name`, scoped to the same (org, group) bucket.
pub async fn rename_all_versions(
    pool: &PgPool,
    org_id: Uuid,
    process_group_id: Option<Uuid>,
    decision_key: &str,
    name: &str,
) -> Result<()> {
    if name.trim().is_empty() {
        return Err(EngineError::Validation(
            "name must not be empty".to_string(),
        ));
    }
    let (name_clash,): (bool,) = sqlx::query_as(
        "SELECT EXISTS(
           SELECT 1 FROM decision_definitions
           WHERE org_id = $1
             AND (($2::uuid IS NULL AND process_group_id IS NULL) OR process_group_id = $2)
             AND name = $3
             AND decision_key <> $4
         )",
    )
    .bind(org_id)
    .bind(process_group_id)
    .bind(name)
    .bind(decision_key)
    .fetch_one(pool)
    .await?;
    if name_clash {
        return Err(EngineError::Conflict(format!(
            "A different decision named '{name}' already exists in this scope"
        )));
    }
    sqlx::query(
        "UPDATE decision_definitions SET name = $1 WHERE org_id = $2 AND decision_key = $3",
    )
    .bind(name)
    .bind(org_id)
    .bind(decision_key)
    .execute(pool)
    .await?;
    Ok(())
}

/// List every version of every decision for an org, optionally filtered to a process group.
/// Ordered by decision_key ASC, version DESC.
pub async fn list_all_versions(
    pool: &PgPool,
    org_id: Uuid,
    process_group_id: Option<Uuid>,
) -> Result<Vec<DecisionDefinition>> {
    let rows = sqlx::query_as::<_, DecisionDefinition>(
        "SELECT *
         FROM decision_definitions
         WHERE org_id = $1
           AND ($2::uuid IS NULL OR process_group_id = $2)
         ORDER BY decision_key ASC, version DESC",
    )
    .bind(org_id)
    .bind(process_group_id)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// List the latest version of each decision for an org, optionally filtered to a process group.
/// Pass `None` to return all org decisions regardless of group.
pub async fn list(
    pool: &PgPool,
    org_id: Uuid,
    process_group_id: Option<Uuid>,
) -> Result<Vec<DecisionDefinition>> {
    let rows = sqlx::query_as::<_, DecisionDefinition>(
        "SELECT DISTINCT ON (decision_key) *
         FROM decision_definitions
         WHERE org_id = $1
           AND ($2::uuid IS NULL OR process_group_id = $2)
         ORDER BY decision_key, version DESC",
    )
    .bind(org_id)
    .bind(process_group_id)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// Paginated variant of `list` / `list_all_versions`. Returns `(rows, total)` where
/// `total` is the count of distinct decision_keys (latest mode) or all rows (all-versions mode)
/// matching the filter, before LIMIT/OFFSET is applied.
pub async fn list_paginated(
    pool: &PgPool,
    org_id: Uuid,
    process_group_id: Option<Uuid>,
    all_versions: bool,
    limit: i64,
    offset: i64,
) -> Result<(Vec<DecisionDefinition>, i64)> {
    let (sql, count_sql) = if all_versions {
        (
            "SELECT *
             FROM decision_definitions
             WHERE org_id = $1
               AND ($2::uuid IS NULL OR process_group_id = $2)
             ORDER BY decision_key ASC, version DESC
             LIMIT $3 OFFSET $4",
            "SELECT COUNT(*)
             FROM decision_definitions
             WHERE org_id = $1
               AND ($2::uuid IS NULL OR process_group_id = $2)",
        )
    } else {
        (
            "SELECT DISTINCT ON (decision_key) *
             FROM decision_definitions
             WHERE org_id = $1
               AND ($2::uuid IS NULL OR process_group_id = $2)
             ORDER BY decision_key, version DESC
             LIMIT $3 OFFSET $4",
            "SELECT COUNT(DISTINCT decision_key)
             FROM decision_definitions
             WHERE org_id = $1
               AND ($2::uuid IS NULL OR process_group_id = $2)",
        )
    };

    let rows = sqlx::query_as::<_, DecisionDefinition>(sql)
        .bind(org_id)
        .bind(process_group_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

    let (total,): (i64,) = sqlx::query_as(count_sql)
        .bind(org_id)
        .bind(process_group_id)
        .fetch_one(pool)
        .await?;

    Ok((rows, total))
}

/// Pg-filtered variant of `list_paginated`. Restricts to decisions whose
/// `process_group_id` is in `pg_ids` (decisions with NULL pg are excluded —
/// they're "unfiled" and can only be accessed via org-level perms).
pub async fn list_paginated_in_pgs(
    pool: &PgPool,
    org_id: Uuid,
    pg_ids: &[Uuid],
    all_versions: bool,
    limit: i64,
    offset: i64,
) -> Result<(Vec<DecisionDefinition>, i64)> {
    if pg_ids.is_empty() {
        return Ok((Vec::new(), 0));
    }
    let pg_vec: Vec<Uuid> = pg_ids.to_vec();
    let (sql, count_sql) = if all_versions {
        (
            "SELECT *
             FROM decision_definitions
             WHERE org_id = $1
               AND process_group_id = ANY($2::uuid[])
             ORDER BY decision_key ASC, version DESC
             LIMIT $3 OFFSET $4",
            "SELECT COUNT(*)
             FROM decision_definitions
             WHERE org_id = $1
               AND process_group_id = ANY($2::uuid[])",
        )
    } else {
        (
            "SELECT DISTINCT ON (decision_key) *
             FROM decision_definitions
             WHERE org_id = $1
               AND process_group_id = ANY($2::uuid[])
             ORDER BY decision_key, version DESC
             LIMIT $3 OFFSET $4",
            "SELECT COUNT(DISTINCT decision_key)
             FROM decision_definitions
             WHERE org_id = $1
               AND process_group_id = ANY($2::uuid[])",
        )
    };
    let rows = sqlx::query_as::<_, DecisionDefinition>(sql)
        .bind(org_id)
        .bind(&pg_vec)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;
    let (total,): (i64,) = sqlx::query_as(count_sql)
        .bind(org_id)
        .bind(&pg_vec)
        .fetch_one(pool)
        .await?;
    Ok((rows, total))
}
