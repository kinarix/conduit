use uuid::Uuid;

use tracing::{debug, info, warn};

use crate::db::models::{EventSubscription, Execution};
use crate::error::{EngineError, Result};
use crate::parser::FlowNodeKind;

use super::{Engine, VariableInput};

impl Engine {
    /// Complete a locked external (service) task and advance the token.
    pub async fn complete_external_task(
        &self,
        job_id: Uuid,
        worker_id: &str,
        variables: &[VariableInput],
    ) -> Result<()> {
        info!(job_id = %job_id, worker_id, variables = variables.len(), "external task completed");

        let job = sqlx::query_as::<_, crate::db::models::Job>("SELECT * FROM jobs WHERE id = $1")
            .bind(job_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| EngineError::NotFound(format!("Job {job_id} not found")))?;

        if job.state != "locked" {
            return Err(EngineError::Conflict(format!(
                "Job {job_id} cannot be completed: state is '{}'",
                job.state
            )));
        }
        if job.locked_by.as_deref() != Some(worker_id) {
            return Err(EngineError::Conflict(format!(
                "Job {job_id} is not locked by worker '{worker_id}'"
            )));
        }

        let def_row: (Uuid,) =
            sqlx::query_as("SELECT definition_id FROM process_instances WHERE id = $1")
                .bind(job.instance_id)
                .fetch_one(&self.pool)
                .await?;

        let graph = self.load_graph(def_row.0).await?;

        let ext_exec: Execution = sqlx::query_as("SELECT * FROM executions WHERE id = $1")
            .bind(job.execution_id)
            .fetch_one(&self.pool)
            .await?;
        let element_id = ext_exec.element_id.clone();
        let parallel_scope = ext_exec.parent_id;

        let mut tx = self.pool.begin().await?;

        for var in variables {
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

        let (current_graph, _) =
            Self::find_element_graph(&element_id, &graph).ok_or_else(|| {
                EngineError::Internal(format!("Element '{element_id}' not found in process graph"))
            })?;
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

    /// Record a failure for a locked external task. Decrements retries; marks instance
    /// as error if the job is exhausted.
    pub async fn fail_external_task(
        &self,
        job_id: Uuid,
        worker_id: &str,
        error_message: &str,
    ) -> Result<()> {
        warn!(job_id = %job_id, worker_id, error = error_message, "external task failed");

        let job = sqlx::query_as::<_, crate::db::models::Job>("SELECT * FROM jobs WHERE id = $1")
            .bind(job_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| EngineError::NotFound(format!("Job {job_id} not found")))?;

        if job.state != "locked" {
            return Err(EngineError::Conflict(format!(
                "Job {job_id} cannot be failed: state is '{}'",
                job.state
            )));
        }
        if job.locked_by.as_deref() != Some(worker_id) {
            return Err(EngineError::Conflict(format!(
                "Job {job_id} is not locked by worker '{worker_id}'"
            )));
        }

        let new_retry_count = job.retry_count + 1;
        let new_state = if new_retry_count >= job.retries {
            "failed"
        } else {
            "pending"
        };
        debug!(job_id = %job_id, retry_count = new_retry_count, retries = job.retries, new_state, "external task retry state");

        let mut tx = self.pool.begin().await?;

        sqlx::query(
            "UPDATE jobs SET \
             retry_count = $1, error_message = $2, \
             locked_by = NULL, locked_until = NULL, state = $3 \
             WHERE id = $4",
        )
        .bind(new_retry_count)
        .bind(error_message)
        .bind(new_state)
        .bind(job_id)
        .execute(&mut *tx)
        .await?;

        if new_state == "failed" {
            sqlx::query(
                "UPDATE process_instances SET state = 'error', ended_at = NOW() WHERE id = $1",
            )
            .bind(job.instance_id)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    /// Throw a BPMN business error from a locked external-task job.
    /// Routes to the best-matching `BoundaryErrorEvent` on the service task (exact error code
    /// first, then catch-all), or terminates the instance if nothing matches.
    pub async fn throw_bpmn_error(
        &self,
        job_id: Uuid,
        worker_id: &str,
        error_code: &str,
        error_message: &str,
        variables: &[VariableInput],
    ) -> Result<()> {
        info!(job_id = %job_id, worker_id, error_code, "BPMN error thrown by worker");

        let job = sqlx::query_as::<_, crate::db::models::Job>("SELECT * FROM jobs WHERE id = $1")
            .bind(job_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| EngineError::NotFound(format!("Job {job_id} not found")))?;

        if job.state != "locked" {
            return Err(EngineError::Conflict(format!(
                "Job {job_id} cannot throw BPMN error: state is '{}'",
                job.state
            )));
        }
        if job.locked_by.as_deref() != Some(worker_id) {
            return Err(EngineError::Conflict(format!(
                "Job {job_id} is not locked by worker '{worker_id}'"
            )));
        }

        let def_row: (Uuid,) =
            sqlx::query_as("SELECT definition_id FROM process_instances WHERE id = $1")
                .bind(job.instance_id)
                .fetch_one(&self.pool)
                .await?;
        let graph = self.load_graph(def_row.0).await?;

        let mut tx = self.pool.begin().await?;

        // Audit: error raised by worker.
        crate::db::process_events::record_error(
            &mut *tx,
            job.instance_id,
            Some(job.execution_id),
            None,
            "error_raised",
            Some(error_code),
            error_message,
        )
        .await?;

        // Claim the best-matching error subscription atomically (exact code wins over catch-all).
        let sub: Option<EventSubscription> = sqlx::query_as(
            "DELETE FROM event_subscriptions \
             WHERE id = ( \
                 SELECT id FROM event_subscriptions \
                 WHERE instance_id = $1 \
                   AND event_type = 'error' \
                   AND (event_name = $2 OR event_name = '') \
                 ORDER BY CASE WHEN event_name = $2 THEN 0 ELSE 1 END ASC, created_at ASC \
                 LIMIT 1 \
             ) RETURNING *",
        )
        .bind(job.instance_id)
        .bind(error_code)
        .fetch_optional(&mut *tx)
        .await?;

        if sub.is_none() {
            warn!(job_id = %job_id, error_code, "no matching BoundaryErrorEvent; terminating instance");
            sqlx::query(
                "UPDATE jobs SET state = 'failed', error_message = $1, \
                 locked_by = NULL, locked_until = NULL WHERE id = $2",
            )
            .bind(error_message)
            .bind(job_id)
            .execute(&mut *tx)
            .await?;
            crate::db::jobs::record_state_change(
                &mut tx,
                job_id,
                "job_failed",
                serde_json::json!({"error_code": error_code, "error_message": error_message}),
            )
            .await?;
            sqlx::query(
                "UPDATE process_instances SET state = 'error', ended_at = NOW() WHERE id = $1",
            )
            .bind(job.instance_id)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            return Ok(());
        }

        let sub = sub.unwrap();

        // Audit: error caught by boundary subscription.
        crate::db::process_events::record_error(
            &mut *tx,
            sub.instance_id,
            Some(sub.execution_id),
            Some(sub.element_id.as_str()),
            "error_caught",
            Some(error_code),
            error_message,
        )
        .await?;

        let (current_graph, _) = Self::find_element_graph(sub.element_id.as_str(), &graph)
            .ok_or_else(|| {
                EngineError::Internal(format!(
                    "Element '{}' not found in process graph",
                    sub.element_id
                ))
            })?;
        let boundary_node = current_graph
            .nodes
            .get(sub.element_id.as_str())
            .ok_or_else(|| {
                EngineError::Internal(format!("Boundary node '{}' not found", sub.element_id))
            })?;

        let (attached_to, cancelling) = match &boundary_node.kind {
            FlowNodeKind::BoundaryErrorEvent {
                attached_to,
                cancelling,
                ..
            } => (attached_to.clone(), *cancelling),
            _ => {
                return Err(EngineError::Internal(format!(
                    "Expected BoundaryErrorEvent at '{}'",
                    sub.element_id
                )));
            }
        };

        // Upsert error variables into the service task's execution scope.
        for var in variables {
            crate::db::variables::upsert_in_tx(
                &mut tx,
                job.instance_id,
                job.execution_id,
                Some(sub.element_id.as_str()),
                &var.name,
                &var.value_type,
                &var.value,
            )
            .await?;
        }

        if cancelling {
            // Cancel the service task job and close its execution.
            sqlx::query(
                "UPDATE jobs SET state = 'cancelled', locked_by = NULL, locked_until = NULL \
                 WHERE id = $1",
            )
            .bind(job_id)
            .execute(&mut *tx)
            .await?;
            crate::db::jobs::record_state_change(
                &mut tx,
                job_id,
                "job_cancelled",
                serde_json::json!({"reason": "boundary_error_cancelled_host"}),
            )
            .await?;

            crate::db::execution_history::record_exit(
                &mut tx,
                job.instance_id,
                job.execution_id,
                attached_to.as_str(),
                "serviceTask",
            )
            .await?;

            sqlx::query("UPDATE executions SET state = 'cancelled' WHERE id = $1")
                .bind(job.execution_id)
                .execute(&mut *tx)
                .await?;

            // Cancel sibling boundary events on the same host task.
            if let Some(all_boundaries) =
                current_graph.attached_to.get(attached_to.as_str()).cloned()
            {
                for bid in &all_boundaries {
                    if bid != &sub.element_id {
                        let cancelled_ids: Vec<Uuid> = sqlx::query_scalar(
                            "UPDATE jobs SET state = 'cancelled' \
                             WHERE execution_id = (SELECT id FROM executions \
                                 WHERE instance_id = $1 AND element_id = $2 LIMIT 1) \
                             AND state IN ('pending', 'locked') \
                             RETURNING id",
                        )
                        .bind(job.instance_id)
                        .bind(bid)
                        .fetch_all(&mut *tx)
                        .await?;
                        crate::db::jobs::record_bulk_cancelled(&mut tx, &cancelled_ids).await?;

                        sqlx::query(
                            "DELETE FROM event_subscriptions \
                             WHERE instance_id = $1 AND element_id = $2",
                        )
                        .bind(job.instance_id)
                        .bind(bid)
                        .execute(&mut *tx)
                        .await?;
                    }
                }
            }
        }

        // Close the boundary event execution and advance from its outgoing flows.
        crate::db::execution_history::record_exit(
            &mut tx,
            job.instance_id,
            sub.execution_id,
            sub.element_id.as_str(),
            "boundaryEvent",
        )
        .await?;

        sqlx::query("UPDATE executions SET state = 'completed' WHERE id = $1")
            .bind(sub.execution_id)
            .execute(&mut *tx)
            .await?;

        let next_ids: Vec<String> = current_graph
            .outgoing
            .get(sub.element_id.as_str())
            .cloned()
            .unwrap_or_default();
        for next_id in next_ids {
            Self::run_to_wait(&mut tx, job.instance_id, &next_id, &graph, None).await?;
        }

        tx.commit().await?;
        info!(job_id = %job_id, boundary_id = %sub.element_id, error_code, "BPMN error routed to boundary event");
        Ok(())
    }
}
