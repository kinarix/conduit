use serde_json::Value as JsonValue;
use sqlx::PgPool;
use uuid::Uuid;

use crate::db::models::ProcessDefinition;
use crate::error::{EngineError, Result};

#[allow(clippy::too_many_arguments)]
pub async fn insert(
    pool: &PgPool,
    org_id: Uuid,
    owner_id: Option<Uuid>,
    process_group_id: Uuid,
    process_key: &str,
    version: i32,
    name: Option<&str>,
    bpmn_xml: &str,
    labels: &JsonValue,
) -> Result<ProcessDefinition> {
    let row = sqlx::query_as::<_, ProcessDefinition>(
        r#"
        INSERT INTO process_definitions (org_id, owner_id, process_group_id, process_key, version, name, bpmn_xml, labels, status)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'deployed')
        RETURNING *
        "#,
    )
    .bind(org_id)
    .bind(owner_id)
    .bind(process_group_id)
    .bind(process_key)
    .bind(version)
    .bind(name)
    .bind(bpmn_xml)
    .bind(labels)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// Strict create for a new process. Fails with Conflict if any definition (draft
/// or deployed) already exists for (org_id, process_key). Used by the "create new
/// process" flow so it cannot silently overwrite an existing draft.
#[allow(clippy::too_many_arguments)]
pub async fn create_draft(
    pool: &PgPool,
    org_id: Uuid,
    owner_id: Option<Uuid>,
    process_group_id: Uuid,
    process_key: &str,
    name: Option<&str>,
    bpmn_xml: &str,
    labels: &JsonValue,
) -> Result<ProcessDefinition> {
    let (key_exists,): (bool,) = sqlx::query_as(
        "SELECT EXISTS(SELECT 1 FROM process_definitions WHERE org_id = $1 AND process_key = $2)",
    )
    .bind(org_id)
    .bind(process_key)
    .fetch_one(pool)
    .await?;
    if key_exists {
        return Err(EngineError::Conflict(format!(
            "A process with key '{process_key}' already exists in this organisation"
        )));
    }

    if let Some(n) = name.filter(|s| !s.trim().is_empty()) {
        let (name_exists,): (bool,) = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM process_definitions WHERE org_id = $1 AND process_group_id = $2 AND name = $3)",
        )
        .bind(org_id)
        .bind(process_group_id)
        .bind(n)
        .fetch_one(pool)
        .await?;
        if name_exists {
            return Err(EngineError::Conflict(format!(
                "A process named '{n}' already exists in this process group"
            )));
        }
    }

    let row = sqlx::query_as::<_, ProcessDefinition>(
        r#"
        INSERT INTO process_definitions (org_id, owner_id, process_group_id, process_key, version, name, bpmn_xml, labels, status)
        VALUES ($1, $2, $3, $4, 1, $5, $6, $7, 'draft')
        RETURNING *
        "#,
    )
    .bind(org_id)
    .bind(owner_id)
    .bind(process_group_id)
    .bind(process_key)
    .bind(name)
    .bind(bpmn_xml)
    .bind(labels)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// Upsert a draft for (org_id, process_key). Only one draft per key is allowed.
/// If a draft already exists it is overwritten; otherwise a new row is inserted at
/// version = max_deployed_version + 1.
#[allow(clippy::too_many_arguments)]
pub async fn save_draft(
    pool: &PgPool,
    org_id: Uuid,
    owner_id: Option<Uuid>,
    process_group_id: Uuid,
    process_key: &str,
    name: Option<&str>,
    bpmn_xml: &str,
    labels: &JsonValue,
) -> Result<ProcessDefinition> {
    // Names must be unique across distinct process_keys in an org. Same name is
    // allowed across versions of the same process_key (different rows, same key).
    if let Some(n) = name.filter(|s| !s.trim().is_empty()) {
        let (name_clash,): (bool,) = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM process_definitions WHERE org_id = $1 AND process_group_id = $2 AND name = $3 AND process_key <> $4)",
        )
        .bind(org_id)
        .bind(process_group_id)
        .bind(n)
        .bind(process_key)
        .fetch_one(pool)
        .await?;
        if name_clash {
            return Err(EngineError::Conflict(format!(
                "A different process named '{n}' already exists in this process group"
            )));
        }
    }

    // Determine version for the draft (max deployed + 1, or 1 if no deployed version)
    let (max_deployed,): (Option<i32>,) = sqlx::query_as(
        "SELECT MAX(version) FROM process_definitions WHERE org_id = $1 AND process_key = $2 AND status = 'deployed'",
    )
    .bind(org_id)
    .bind(process_key)
    .fetch_one(pool)
    .await?;
    let draft_version = max_deployed.unwrap_or(0) + 1;

    // process_group_id is preserved (not in DO UPDATE) so re-saving a draft does not move it.
    let row = sqlx::query_as::<_, ProcessDefinition>(
        r#"
        INSERT INTO process_definitions (org_id, owner_id, process_group_id, process_key, version, name, bpmn_xml, labels, status)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'draft')
        ON CONFLICT (org_id, process_key) WHERE status = 'draft'
        DO UPDATE SET
            owner_id  = EXCLUDED.owner_id,
            version   = EXCLUDED.version,
            name      = EXCLUDED.name,
            bpmn_xml  = EXCLUDED.bpmn_xml,
            labels    = EXCLUDED.labels,
            deployed_at = now()
        RETURNING *
        "#,
    )
    .bind(org_id)
    .bind(owner_id)
    .bind(process_group_id)
    .bind(process_key)
    .bind(draft_version)
    .bind(name)
    .bind(bpmn_xml)
    .bind(labels)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// Promote a draft to deployed. If the draft version conflicts with an existing
/// deployed version (race condition), bumps to max+1.
pub async fn promote_draft(pool: &PgPool, draft_id: Uuid) -> Result<ProcessDefinition> {
    // Fetch the draft first
    let draft = sqlx::query_as::<_, ProcessDefinition>(
        "SELECT * FROM process_definitions WHERE id = $1 AND status = 'draft'",
    )
    .bind(draft_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| EngineError::NotFound(format!("Draft {draft_id} not found")))?;

    // Recalculate correct deployed version to avoid collisions
    let (max_deployed,): (Option<i32>,) = sqlx::query_as(
        "SELECT MAX(version) FROM process_definitions WHERE org_id = $1 AND process_key = $2 AND status = 'deployed'",
    )
    .bind(draft.org_id)
    .bind(&draft.process_key)
    .fetch_one(pool)
    .await?;
    let deploy_version = max_deployed.unwrap_or(0) + 1;

    let row = sqlx::query_as::<_, ProcessDefinition>(
        r#"
        UPDATE process_definitions
        SET status = 'deployed', version = $2, deployed_at = now()
        WHERE id = $1
        RETURNING *
        "#,
    )
    .bind(draft_id)
    .bind(deploy_version)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn get_by_id(pool: &PgPool, id: Uuid) -> Result<ProcessDefinition> {
    sqlx::query_as::<_, ProcessDefinition>("SELECT * FROM process_definitions WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| EngineError::NotFound(format!("Process definition {id} not found")))
}

/// Returns the latest *deployed* version for engine use.
pub async fn get_latest_by_key(pool: &PgPool, process_key: &str) -> Result<ProcessDefinition> {
    sqlx::query_as::<_, ProcessDefinition>(
        r#"
        SELECT * FROM process_definitions
        WHERE process_key = $1 AND status = 'deployed'
        ORDER BY version DESC
        LIMIT 1
        "#,
    )
    .bind(process_key)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| {
        EngineError::NotFound(format!(
            "No deployed definition found for key '{process_key}'"
        ))
    })
}

pub async fn list_by_org(pool: &PgPool, org_id: Uuid) -> Result<Vec<ProcessDefinition>> {
    let rows = sqlx::query_as::<_, ProcessDefinition>(
        "SELECT * FROM process_definitions WHERE org_id = $1 ORDER BY deployed_at DESC",
    )
    .bind(org_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Delete a process definition. Refuses with Conflict if any instances reference it
/// (the FK is ON DELETE RESTRICT, but checking up-front yields a clearer 409 than a
/// generic FK violation). Timer-start triggers cascade automatically.
pub async fn delete(pool: &PgPool, id: Uuid) -> Result<()> {
    let (used,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM process_instances WHERE definition_id = $1")
            .bind(id)
            .fetch_one(pool)
            .await?;
    if used > 0 {
        return Err(EngineError::Conflict(format!(
            "Process definition {id} has {used} instance(s). Delete or wait for them to finish first."
        )));
    }

    let res = sqlx::query("DELETE FROM process_definitions WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(EngineError::NotFound(format!(
            "process definition {id} not found"
        )));
    }
    Ok(())
}

pub async fn next_version(pool: &PgPool, org_id: Uuid, process_key: &str) -> Result<i32> {
    let row: (Option<i32>,) = sqlx::query_as(
        "SELECT MAX(version) FROM process_definitions WHERE org_id = $1 AND process_key = $2",
    )
    .bind(org_id)
    .bind(process_key)
    .fetch_one(pool)
    .await?;
    Ok(row.0.unwrap_or(0) + 1)
}
