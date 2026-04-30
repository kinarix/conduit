use uuid::Uuid;

use tracing::{debug, info, warn};

use crate::db::models::Execution;
use crate::error::{EngineError, Result};

use super::Engine;

impl Engine {
    /// Fire a single send_message job: deliver the message to the first waiting subscriber,
    /// then continue the SendTask's outgoing flows. Fire-and-continue: if no subscriber is
    /// found (no matching ReceiveTask / IntermediateMessageCatchEvent), the message is dropped
    /// with a warning and the SendTask still completes normally.
    pub async fn fire_send_message_job(&self, job_id: Uuid) -> Result<()> {
        debug!(job_id = %job_id, "firing send message job");
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

        let message_name = job
            .topic
            .as_deref()
            .ok_or_else(|| EngineError::Internal(format!("send_message job {job_id} has no topic")))?
            .to_string();

        let def_row: (Uuid, Uuid) =
            sqlx::query_as("SELECT definition_id, org_id FROM process_instances WHERE id = $1")
                .bind(job.instance_id)
                .fetch_one(&self.pool)
                .await?;
        let graph = self.load_graph(def_row.0).await?;

        let send_exec: Execution = sqlx::query_as("SELECT * FROM executions WHERE id = $1")
            .bind(job.execution_id)
            .fetch_one(&self.pool)
            .await?;
        let element_id = send_exec.element_id.clone();
        let parallel_scope = send_exec.parent_id;

        let (current_graph, _) =
            Self::find_element_graph(&element_id, &graph).ok_or_else(|| {
                EngineError::Internal(format!("Element '{element_id}' not found in process graph"))
            })?;

        // Best-effort delivery — drop message if no subscriber found.
        match self.correlate_message(&message_name, None, &[], def_row.1).await {
            Ok(_) => {
                info!(job_id = %job_id, message = %message_name, "send task delivered message");
            }
            Err(EngineError::NotFound(_)) => {
                warn!(job_id = %job_id, message = %message_name, "send task: no subscriber found, message dropped");
            }
            Err(e) => return Err(e),
        }

        let mut tx = self.pool.begin().await?;

        crate::db::execution_history::record_exit(
            &mut tx,
            job.instance_id,
            job.execution_id,
            &element_id,
            "sendTask",
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

    pub async fn fire_due_send_message_jobs(&self) -> Result<usize> {
        let jobs = crate::db::jobs::fetch_and_lock_many(
            &self.pool,
            "conduit-send-message-executor",
            30,
            None,
            Some("send_message"),
            20,
        )
        .await?;

        let mut count = 0;
        for job in jobs {
            match self.fire_send_message_job(job.id).await {
                Ok(_) => count += 1,
                Err(crate::error::EngineError::Conflict(_)) => {}
                Err(e) => return Err(e),
            }
        }
        if count > 0 {
            info!(count, "fired due send message jobs");
        }
        Ok(count)
    }
}
