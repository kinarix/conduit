use uuid::Uuid;

use tracing::debug;

use crate::db::models::{Execution, Task};
use crate::error::{EngineError, Result};

use super::{Engine, VariableInput};

impl Engine {
    /// Complete a pending user task and advance the token to the next element.
    /// Variables written here are scoped to the task's execution and visible to
    /// gateway condition evaluation within the same transaction.
    pub async fn complete_user_task(
        &self,
        task_id: Uuid,
        variables: &[VariableInput],
    ) -> Result<()> {
        debug!(task_id = %task_id, variables = variables.len(), "completing user task");

        let task = sqlx::query_as::<_, Task>("SELECT * FROM tasks WHERE id = $1")
            .bind(task_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| EngineError::NotFound(format!("Task {task_id} not found")))?;

        if task.state != "pending" {
            return Err(EngineError::Conflict(format!(
                "Task {task_id} cannot be completed: state is '{}'",
                task.state
            )));
        }

        let def_row: (Uuid,) =
            sqlx::query_as("SELECT definition_id FROM process_instances WHERE id = $1")
                .bind(task.instance_id)
                .fetch_one(&self.pool)
                .await?;

        let graph = self.load_graph(def_row.0).await?;

        let task_exec: Execution = sqlx::query_as("SELECT * FROM executions WHERE id = $1")
            .bind(task.execution_id)
            .fetch_one(&self.pool)
            .await?;
        let parallel_scope = task_exec.parent_id;

        let mut tx = self.pool.begin().await?;

        sqlx::query("UPDATE tasks SET state = 'completed', completed_at = NOW() WHERE id = $1")
            .bind(task_id)
            .execute(&mut *tx)
            .await?;

        for var in variables {
            crate::db::variables::upsert_in_tx(
                &mut tx,
                task.instance_id,
                task.execution_id,
                Some(&task.element_id),
                &var.name,
                &var.value_type,
                &var.value,
            )
            .await?;
        }

        crate::db::execution_history::record_exit(
            &mut tx,
            task.instance_id,
            task.execution_id,
            &task.element_id,
            "userTask",
        )
        .await?;

        sqlx::query("UPDATE executions SET state = 'completed' WHERE id = $1")
            .bind(task.execution_id)
            .execute(&mut *tx)
            .await?;

        let (current_graph, _) =
            Self::find_element_graph(&task.element_id, &graph).ok_or_else(|| {
                EngineError::Internal(format!(
                    "Element '{}' not found in process graph",
                    task.element_id
                ))
            })?;

        // Cancel any pending boundary events (timers and signal subscriptions) attached to this task.
        if let Some(boundary_ids) = current_graph.attached_to.get(&task.element_id) {
            for boundary_id in boundary_ids {
                let cancelled_ids: Vec<Uuid> = sqlx::query_scalar(
                    "UPDATE jobs SET state = 'cancelled' \
                     WHERE execution_id = (SELECT id FROM executions \
                         WHERE instance_id = $1 AND element_id = $2 \
                         LIMIT 1) \
                     AND state IN ('pending', 'locked') \
                     RETURNING id",
                )
                .bind(task.instance_id)
                .bind(boundary_id)
                .fetch_all(&mut *tx)
                .await?;
                crate::db::jobs::record_bulk_cancelled(&mut tx, &cancelled_ids).await?;

                sqlx::query(
                    "DELETE FROM event_subscriptions \
                     WHERE instance_id = $1 AND element_id = $2",
                )
                .bind(task.instance_id)
                .bind(boundary_id)
                .execute(&mut *tx)
                .await?;

                sqlx::query(
                    "UPDATE executions SET state = 'cancelled' \
                     WHERE instance_id = $1 AND element_id = $2 AND state = 'active'",
                )
                .bind(task.instance_id)
                .bind(boundary_id)
                .execute(&mut *tx)
                .await?;
            }
        }

        let next_ids: Vec<String> = current_graph
            .outgoing
            .get(&task.element_id)
            .cloned()
            .unwrap_or_default();

        for next_id in next_ids {
            Self::run_to_wait(&mut tx, task.instance_id, &next_id, &graph, parallel_scope).await?;
        }

        tx.commit().await?;
        Ok(())
    }
}
