use uuid::Uuid;

use serde_json::Value as JsonValue;
use tracing::{debug, info};

use crate::db::models::Execution;
use crate::error::{EngineError, Result};

use super::{Engine, VariableInput};

impl Engine {
    pub async fn fire_http_task(&self, job_id: Uuid) -> Result<()> {
        debug!(job_id = %job_id, "firing HTTP task");
        let job = sqlx::query_as::<_, crate::db::models::Job>("SELECT * FROM jobs WHERE id = $1")
            .bind(job_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| EngineError::NotFound(format!("Job {job_id} not found")))?;

        if job.state == "completed" || job.state == "cancelled" {
            return Err(EngineError::Conflict(format!(
                "Job {job_id} cannot be fired: state is '{}'",
                job.state
            )));
        }

        let url = job
            .topic
            .as_deref()
            .ok_or_else(|| EngineError::Internal(format!("HTTP task job {job_id} has no URL")))?
            .to_string();

        // Load current instance variables to pass as context.
        let vars: Vec<crate::db::models::Variable> =
            sqlx::query_as("SELECT * FROM variables WHERE instance_id = $1")
                .bind(job.instance_id)
                .fetch_all(&self.pool)
                .await?;

        let mut variables_map = serde_json::Map::new();
        for v in &vars {
            variables_map.insert(v.name.clone(), v.value.clone());
        }

        let payload = serde_json::json!({
            "instance_id": job.instance_id.to_string(),
            "execution_id": job.execution_id.to_string(),
            "variables": variables_map,
        });

        // Make the HTTP call outside any transaction.
        debug!(job_id = %job_id, url = %url, "sending HTTP service task request");
        let client = reqwest::Client::new();
        let resp = client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| EngineError::Internal(format!("HTTP service task call failed: {e}")))?;

        let status = resp.status();
        let body: serde_json::Value = resp
            .json()
            .await
            .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

        if !status.is_success() {
            return Err(EngineError::Internal(format!(
                "HTTP service task URL '{url}' returned {status}"
            )));
        }

        let output_vars = Self::parse_http_response_variables(&body);
        info!(job_id = %job_id, instance_id = %job.instance_id, status = %status, "HTTP service task completed");

        let def_row: (Uuid,) =
            sqlx::query_as("SELECT definition_id FROM process_instances WHERE id = $1")
                .bind(job.instance_id)
                .fetch_one(&self.pool)
                .await?;
        let graph = self.load_graph(def_row.0).await?;

        let http_exec: Execution = sqlx::query_as("SELECT * FROM executions WHERE id = $1")
            .bind(job.execution_id)
            .fetch_one(&self.pool)
            .await?;
        let element_id = http_exec.element_id.clone();
        let parallel_scope = http_exec.parent_id;

        let (current_graph, _) =
            Self::find_element_graph(&element_id, &graph).ok_or_else(|| {
                EngineError::Internal(format!("Element '{element_id}' not found in process graph"))
            })?;

        let mut tx = self.pool.begin().await?;

        for var in &output_vars {
            crate::db::variables::upsert_in_tx(
                &mut tx,
                job.instance_id,
                job.execution_id,
                Some(&element_id),
                &var.name,
                &var.value_type,
                &var.value,
            )
            .await?;
        }

        crate::db::execution_history::record_exit(
            &mut tx,
            job.instance_id,
            job.execution_id,
            &element_id,
            "serviceTask",
        )
        .await?;

        sqlx::query("UPDATE executions SET state = 'completed' WHERE id = $1")
            .bind(job.execution_id)
            .execute(&mut *tx)
            .await?;

        sqlx::query(
            "UPDATE jobs SET state = 'completed', locked_by = NULL, locked_until = NULL \
             WHERE id = $1",
        )
        .bind(job_id)
        .execute(&mut *tx)
        .await?;
        crate::db::jobs::record_state_change(
            &mut tx,
            job_id,
            "job_completed",
            serde_json::json!({}),
        )
        .await?;

        let next_ids: Vec<String> = current_graph
            .outgoing
            .get(&element_id)
            .cloned()
            .unwrap_or_default();
        for next_id in next_ids {
            Self::run_to_wait(&mut tx, job.instance_id, &next_id, &graph, parallel_scope).await?;
        }

        tx.commit().await?;
        Ok(())
    }

    fn parse_http_response_variables(body: &serde_json::Value) -> Vec<VariableInput> {
        let obj = match body.as_object() {
            Some(o) => o,
            None => return vec![],
        };

        // Conduit-native format: { "variables": [{ "name", "value_type", "value" }] }
        if let Some(arr) = obj.get("variables").and_then(|v| v.as_array()) {
            return arr
                .iter()
                .filter_map(|v| {
                    let name = v.get("name")?.as_str()?.to_string();
                    let value_type = v.get("value_type")?.as_str()?.to_string();
                    let value = v.get("value")?.clone();
                    Some(VariableInput { name, value_type, value })
                })
                .collect();
        }

        // Flat format: every top-level key becomes a variable; type inferred from JSON type.
        obj.iter()
            .filter(|(k, _)| k.as_str() != "instance_id" && k.as_str() != "execution_id")
            .map(|(k, v)| {
                let value_type = match v {
                    JsonValue::String(_) => "string",
                    JsonValue::Number(_) => "number",
                    JsonValue::Bool(_) => "boolean",
                    _ => "json",
                }
                .to_string();
                VariableInput {
                    name: k.clone(),
                    value_type,
                    value: v.clone(),
                }
            })
            .collect()
    }

    pub async fn fire_due_http_tasks(&self) -> Result<usize> {
        let jobs = crate::db::jobs::fetch_and_lock_many(
            &self.pool,
            "conduit-http-executor",
            30,
            None,
            Some("http_task"),
            20,
        )
        .await?;

        let mut count = 0;
        for job in jobs {
            match self.fire_http_task(job.id).await {
                Ok(_) => count += 1,
                Err(crate::error::EngineError::Conflict(_)) => {}
                Err(e) => return Err(e),
            }
        }
        if count > 0 {
            info!(count, "fired due HTTP tasks");
        }
        Ok(count)
    }
}
