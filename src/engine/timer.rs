use chrono::Utc;
use uuid::Uuid;

use tracing::{debug, info};

use crate::db::models::{Execution, ProcessInstance, TimerStartTrigger};
use crate::error::{EngineError, Result};
use crate::parser::FlowNodeKind;

use super::Engine;

impl Engine {
    /// Fire a specific timer job by ID, regardless of its due_date.
    /// Used by the background executor after it has already claimed the job via SKIP LOCKED.
    pub async fn fire_timer_job(&self, job_id: Uuid) -> Result<()> {
        debug!(job_id = %job_id, "firing timer job");
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

        let def_row: (Uuid,) =
            sqlx::query_as("SELECT definition_id FROM process_instances WHERE id = $1")
                .bind(job.instance_id)
                .fetch_one(&self.pool)
                .await?;

        let graph = self.load_graph(def_row.0).await?;

        let timer_exec: Execution = sqlx::query_as("SELECT * FROM executions WHERE id = $1")
            .bind(job.execution_id)
            .fetch_one(&self.pool)
            .await?;
        let element_id = timer_exec.element_id.clone();
        let parallel_scope = timer_exec.parent_id;

        let (current_graph, _) =
            Self::find_element_graph(&element_id, &graph).ok_or_else(|| {
                EngineError::Internal(format!("Element '{element_id}' not found in process graph"))
            })?;
        let node = current_graph
            .nodes
            .get(&element_id)
            .expect("find_element_graph contract: element exists in returned graph");

        let mut tx = self.pool.begin().await?;

        match &node.kind {
            FlowNodeKind::IntermediateTimerCatchEvent { .. } => {
                info!(job_id = %job_id, instance_id = %job.instance_id, element_id = %element_id, "timer catch event fired");
                crate::db::execution_history::record_exit(
                    &mut tx,
                    job.instance_id,
                    job.execution_id,
                    &element_id,
                    "intermediateCatchEvent",
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
                    Self::run_to_wait(&mut tx, job.instance_id, &next_id, &graph, parallel_scope)
                        .await?;
                }
            }

            FlowNodeKind::BoundaryTimerEvent {
                attached_to,
                cancelling,
                ..
            } => {
                info!(job_id = %job_id, instance_id = %job.instance_id, element_id = %element_id, cancelling = %cancelling, "boundary timer event fired");
                // Cancel the associated user task if this is an interrupting boundary event.
                if *cancelling {
                    sqlx::query(
                        "UPDATE tasks SET state = 'cancelled' \
                         WHERE execution_id = (SELECT id FROM executions \
                             WHERE instance_id = $1 AND element_id = $2 AND state = 'active' \
                             LIMIT 1)",
                    )
                    .bind(job.instance_id)
                    .bind(attached_to)
                    .execute(&mut *tx)
                    .await?;

                    // Close out the host execution.
                    sqlx::query(
                        "UPDATE execution_history SET left_at = NOW() \
                         WHERE execution_id = (SELECT id FROM executions \
                             WHERE instance_id = $1 AND element_id = $2 AND state = 'active' \
                             LIMIT 1) AND left_at IS NULL",
                    )
                    .bind(job.instance_id)
                    .bind(attached_to)
                    .execute(&mut *tx)
                    .await?;

                    sqlx::query(
                        "UPDATE executions SET state = 'cancelled' \
                         WHERE instance_id = $1 AND element_id = $2 AND state = 'active'",
                    )
                    .bind(job.instance_id)
                    .bind(attached_to)
                    .execute(&mut *tx)
                    .await?;
                }

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
                    serde_json::json!({"boundary": true}),
                )
                .await?;

                // Close out the boundary event execution itself (it fired).
                sqlx::query(
                    "UPDATE executions SET state = 'completed' WHERE id = $1 AND state = 'active'",
                )
                .bind(job.execution_id)
                .execute(&mut *tx)
                .await?;

                let next_ids: Vec<String> = current_graph
                    .outgoing
                    .get(&element_id)
                    .cloned()
                    .unwrap_or_default();
                for next_id in next_ids {
                    Self::run_to_wait(&mut tx, job.instance_id, &next_id, &graph, parallel_scope)
                        .await?;
                }

                // Reschedule non-cancelling cycle boundary timers.
                if !*cancelling {
                    if let Some(expr) = &job.timer_expression {
                        let reschedule = match job.repetitions_remaining {
                            None => true,     // infinite cycle
                            Some(1) => false, // last repetition
                            Some(_) => true,
                        };
                        if reschedule {
                            let new_reps = job.repetitions_remaining.map(|n| n - 1);
                            let (_, interval) = super::helpers::parse_cycle(expr)?;
                            let due = Utc::now() + interval;
                            let new_job_id: Uuid = sqlx::query_scalar(
                                "INSERT INTO jobs \
                                 (instance_id, execution_id, job_type, due_date, \
                                  timer_expression, repetitions_remaining, retries, state) \
                                 VALUES ($1, $2, 'timer', $3, $4, $5, 1, 'pending') \
                                 RETURNING id",
                            )
                            .bind(job.instance_id)
                            .bind(job.execution_id)
                            .bind(due)
                            .bind(expr)
                            .bind(new_reps)
                            .fetch_one(&mut *tx)
                            .await?;
                            crate::db::process_events::record_job(
                                &mut *tx,
                                job.instance_id,
                                Some(job.execution_id),
                                Some(element_id.as_str()),
                                Some(new_job_id),
                                "timer",
                                "job_created",
                                serde_json::json!({"due_date": due, "rescheduled_from": job.id}),
                            )
                            .await?;
                        }
                    }
                }
            }

            _ => {
                return Err(EngineError::Conflict(format!(
                    "Job {job_id} element '{element_id}' is not a timer event"
                )));
            }
        }

        tx.commit().await?;
        Ok(())
    }

    /// Fetch and fire all due timer jobs. Returns the count of jobs fired.
    /// Safe to call concurrently from multiple executors — uses SKIP LOCKED.
    pub async fn fire_due_timer_jobs(&self) -> Result<usize> {
        let jobs = crate::db::jobs::fetch_and_lock_many(
            &self.pool,
            "conduit-timer-executor",
            30,
            None,
            Some("timer"),
            20,
        )
        .await?;

        let mut count = 0;
        for job in jobs {
            match self.fire_timer_job(job.id).await {
                Ok(_) => count += 1,
                // Another caller (e.g. a direct test call) fired the job in the window between
                // fetch_and_lock_many returning and fire_timer_job reading state. Treat as done.
                Err(crate::error::EngineError::Conflict(_)) => {}
                Err(e) => return Err(e),
            }
        }
        if count > 0 {
            info!(count, "fired due timer jobs");
        }
        Ok(count)
    }

    /// Register timer-start triggers for all `TimerStartEvent` nodes in a definition.
    /// Called after a new definition is deployed.
    pub async fn schedule_timer_start_events(&self, definition_id: Uuid) -> Result<()> {
        let graph = self.load_graph(definition_id).await?;

        for node in graph.nodes.values() {
            if let FlowNodeKind::TimerStartEvent { timer } = &node.kind {
                let (due_date, repetitions_remaining, timer_expr) = match timer {
                    crate::parser::TimerSpec::Duration(d) => (
                        Utc::now() + super::helpers::parse_duration(d)?,
                        Some(1i32),
                        d.clone(),
                    ),
                    crate::parser::TimerSpec::Cycle(expr) => {
                        let (reps, interval) = super::helpers::parse_cycle(expr)?;
                        (Utc::now() + interval, reps, expr.clone())
                    }
                    crate::parser::TimerSpec::Date(dt_str) => (
                        super::helpers::parse_date(dt_str)?,
                        Some(1i32),
                        dt_str.clone(),
                    ),
                };

                sqlx::query(
                    "INSERT INTO timer_start_triggers \
                     (definition_id, element_id, timer_expression, repetitions_remaining, due_at) \
                     VALUES ($1, $2, $3, $4, $5)",
                )
                .bind(definition_id)
                .bind(&node.id)
                .bind(&timer_expr)
                .bind(repetitions_remaining)
                .bind(due_date)
                .execute(&self.pool)
                .await?;
            }
        }
        Ok(())
    }

    /// Cancel all pending timer-start triggers for a definition (e.g. on re-deploy).
    pub async fn cancel_timer_start_jobs(&self, definition_id: Uuid) -> Result<()> {
        sqlx::query(
            "UPDATE timer_start_triggers SET state = 'cancelled' \
             WHERE definition_id = $1 AND state = 'pending'",
        )
        .bind(definition_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Fetch and fire all due timer-start triggers, creating new process instances.
    pub async fn fire_due_timer_start_triggers(&self) -> Result<usize> {
        let triggers: Vec<TimerStartTrigger> = sqlx::query_as(
            "WITH candidates AS ( \
                 SELECT id FROM timer_start_triggers \
                 WHERE state = 'pending' \
                   AND due_at <= NOW() \
                   AND (locked_until IS NULL OR locked_until < NOW()) \
                 ORDER BY due_at ASC \
                 FOR UPDATE SKIP LOCKED \
                 LIMIT 20 \
             ) \
             UPDATE timer_start_triggers \
             SET locked_by = 'conduit-timer-start', \
                 locked_until = NOW() + interval '30 seconds' \
             WHERE id IN (SELECT id FROM candidates) \
             RETURNING *",
        )
        .fetch_all(&self.pool)
        .await?;

        let mut count = 0;
        for trigger in triggers {
            match self.fire_timer_start_trigger(&trigger).await {
                Ok(_) => count += 1,
                Err(e) => tracing::error!(
                    trigger_id = %trigger.id,
                    error = %e,
                    "timer start trigger failed"
                ),
            }
        }
        if count > 0 {
            info!(count, "fired due timer start triggers");
        }
        Ok(count)
    }

    async fn fire_timer_start_trigger(&self, trigger: &TimerStartTrigger) -> Result<()> {
        let graph = self.load_graph(trigger.definition_id).await?;

        let (org_id, status, disabled_at): (uuid::Uuid, String, Option<chrono::DateTime<chrono::Utc>>) =
            sqlx::query_as(
                "SELECT org_id, status, disabled_at FROM process_definitions WHERE id = $1",
            )
            .bind(trigger.definition_id)
            .fetch_one(&self.pool)
            .await?;

        // Definition was disabled or unpublished after this trigger was scheduled — skip.
        if status != "deployed" || disabled_at.is_some() {
            sqlx::query(
                "UPDATE timer_start_triggers SET state = 'cancelled', \
                 locked_by = NULL, locked_until = NULL WHERE id = $1",
            )
            .bind(trigger.id)
            .execute(&self.pool)
            .await?;
            return Ok(());
        }

        let start_node = graph
            .nodes
            .values()
            .find(|n| n.id == trigger.element_id)
            .ok_or_else(|| {
                EngineError::Internal(format!(
                    "TimerStartEvent '{}' not found in graph",
                    trigger.element_id
                ))
            })?
            .clone();

        let mut tx = self.pool.begin().await?;

        let instance = sqlx::query_as::<_, ProcessInstance>(
            "INSERT INTO process_instances (org_id, definition_id, state, labels) \
             VALUES ($1, $2, 'running', '{}') RETURNING *",
        )
        .bind(org_id)
        .bind(trigger.definition_id)
        .fetch_one(&mut *tx)
        .await?;

        info!(
            instance_id = %instance.id,
            definition_id = %trigger.definition_id,
            element_id = %trigger.element_id,
            "timer start event triggered new instance"
        );

        Self::run_to_wait(&mut tx, instance.id, &start_node.id, &graph, None).await?;

        sqlx::query(
            "UPDATE timer_start_triggers \
             SET state = 'fired', locked_by = NULL, locked_until = NULL \
             WHERE id = $1",
        )
        .bind(trigger.id)
        .execute(&mut *tx)
        .await?;

        // Reschedule if this is a cycle timer with remaining repetitions.
        let reschedule = match trigger.repetitions_remaining {
            None => true,     // infinite cycle
            Some(1) => false, // last/only fire
            Some(_) => true,
        };
        if reschedule {
            let new_reps = trigger.repetitions_remaining.map(|n| n - 1);
            let (_, interval) = super::helpers::parse_cycle(&trigger.timer_expression)?;
            sqlx::query(
                "INSERT INTO timer_start_triggers \
                 (definition_id, element_id, timer_expression, repetitions_remaining, due_at) \
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(trigger.definition_id)
            .bind(&trigger.element_id)
            .bind(&trigger.timer_expression)
            .bind(new_reps)
            .bind(Utc::now() + interval)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }
}
