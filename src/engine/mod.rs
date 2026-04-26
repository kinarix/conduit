mod evaluator;

use std::collections::HashMap;
use std::sync::Arc;

use chrono::Duration;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use uuid::Uuid;

use crate::db::models::{Execution, ProcessInstance, Task, Variable};
use crate::error::{EngineError, Result};
use crate::parser::{FlowNodeKind, ProcessGraph};
use crate::state::GraphCache;

/// Parse an ISO 8601 duration string into a `chrono::Duration`.
/// Supported forms: PT<n>S, PT<n>M, PT<n>H, P<n>D, and combinations like PT1H30M.
pub fn parse_duration(s: &str) -> Result<Duration> {
    if s.is_empty() {
        return Err(EngineError::Parse("Empty duration string".to_string()));
    }

    let s = s.trim();
    let mut chars = s.chars().peekable();

    if chars.next() != Some('P') {
        return Err(EngineError::Parse(format!(
            "Invalid ISO 8601 duration: '{s}'"
        )));
    }

    let mut total_secs: i64 = 0;
    let mut in_time = false;
    let mut num_buf = String::new();
    let mut parsed_any = false;

    for ch in chars {
        match ch {
            'T' => {
                in_time = true;
            }
            '0'..='9' => {
                num_buf.push(ch);
            }
            'D' if !in_time => {
                let n: i64 = num_buf.parse().map_err(|_| {
                    EngineError::Parse(format!("Invalid number in duration: '{s}'"))
                })?;
                total_secs += n * 86_400;
                num_buf.clear();
                parsed_any = true;
            }
            'H' if in_time => {
                let n: i64 = num_buf.parse().map_err(|_| {
                    EngineError::Parse(format!("Invalid number in duration: '{s}'"))
                })?;
                total_secs += n * 3_600;
                num_buf.clear();
                parsed_any = true;
            }
            'M' if in_time => {
                let n: i64 = num_buf.parse().map_err(|_| {
                    EngineError::Parse(format!("Invalid number in duration: '{s}'"))
                })?;
                total_secs += n * 60;
                num_buf.clear();
                parsed_any = true;
            }
            'S' if in_time => {
                let n: i64 = num_buf.parse().map_err(|_| {
                    EngineError::Parse(format!("Invalid number in duration: '{s}'"))
                })?;
                total_secs += n;
                num_buf.clear();
                parsed_any = true;
            }
            _ => {
                return Err(EngineError::Parse(format!(
                    "Unexpected character '{ch}' in duration: '{s}'"
                )));
            }
        }
    }

    if !parsed_any {
        return Err(EngineError::Parse(format!(
            "No duration components found in: '{s}'"
        )));
    }

    Ok(Duration::seconds(total_secs))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableInput {
    pub name: String,
    pub value_type: String,
    pub value: JsonValue,
}

pub struct Engine {
    pool: PgPool,
    process_cache: GraphCache,
}

impl Engine {
    pub fn new(pool: PgPool, process_cache: GraphCache) -> Self {
        Self {
            pool,
            process_cache,
        }
    }

    /// Load the process graph for a definition, using the in-process cache.
    /// Falls back to parsing from the database on a cache miss (e.g. after restart).
    async fn load_graph(&self, definition_id: Uuid) -> Result<Arc<ProcessGraph>> {
        {
            let cache = self.process_cache.read().unwrap();
            if let Some(graph) = cache.get(&definition_id) {
                return Ok(Arc::clone(graph));
            }
        }

        let row: (String,) =
            sqlx::query_as("SELECT bpmn_xml FROM process_definitions WHERE id = $1")
                .bind(definition_id)
                .fetch_optional(&self.pool)
                .await?
                .ok_or_else(|| {
                    EngineError::NotFound(format!("Process definition {definition_id} not found"))
                })?;

        let graph = crate::parser::parse(&row.0)?;
        let arc = Arc::new(graph);
        {
            let mut cache = self.process_cache.write().unwrap();
            cache.insert(definition_id, Arc::clone(&arc));
        }
        Ok(arc)
    }

    /// Create a new process instance and advance the token until it reaches
    /// a wait state (UserTask, ServiceTask) or completes (EndEvent).
    pub async fn start_instance(
        &self,
        definition_id: Uuid,
        org_id: Uuid,
        labels: &JsonValue,
    ) -> Result<ProcessInstance> {
        let graph = self.load_graph(definition_id).await?;

        let start_node = graph
            .nodes
            .values()
            .find(|n| matches!(n.kind, FlowNodeKind::StartEvent))
            .ok_or_else(|| EngineError::Internal("No start event in process graph".to_string()))?
            .clone();

        let mut tx = self.pool.begin().await?;

        let instance = sqlx::query_as::<_, ProcessInstance>(
            "INSERT INTO process_instances (org_id, definition_id, state, labels) VALUES ($1, $2, 'running', $3) RETURNING *",
        )
        .bind(org_id)
        .bind(definition_id)
        .bind(labels)
        .fetch_one(&mut *tx)
        .await?;

        Self::run_to_wait(&mut tx, instance.id, &start_node.id, &graph).await?;

        // Re-fetch within the transaction to capture any state update (e.g. Start→End).
        let final_instance =
            sqlx::query_as::<_, ProcessInstance>("SELECT * FROM process_instances WHERE id = $1")
                .bind(instance.id)
                .fetch_one(&mut *tx)
                .await?;

        tx.commit().await?;
        Ok(final_instance)
    }

    /// Complete a pending user task and advance the token to the next element.
    /// Variables written here are scoped to the task's execution and visible to
    /// gateway condition evaluation within the same transaction.
    pub async fn complete_user_task(
        &self,
        task_id: Uuid,
        variables: &[VariableInput],
    ) -> Result<()> {
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

        let mut tx = self.pool.begin().await?;

        sqlx::query("UPDATE tasks SET state = 'completed', completed_at = NOW() WHERE id = $1")
            .bind(task_id)
            .execute(&mut *tx)
            .await?;

        for var in variables {
            sqlx::query(
                "INSERT INTO variables (instance_id, execution_id, name, value_type, value) \
                 VALUES ($1, $2, $3, $4, $5) \
                 ON CONFLICT (execution_id, name) \
                 DO UPDATE SET value_type = EXCLUDED.value_type, value = EXCLUDED.value",
            )
            .bind(task.instance_id)
            .bind(task.execution_id)
            .bind(&var.name)
            .bind(&var.value_type)
            .bind(&var.value)
            .execute(&mut *tx)
            .await?;
        }

        sqlx::query(
            "UPDATE execution_history SET left_at = NOW() \
             WHERE execution_id = $1 AND left_at IS NULL",
        )
        .bind(task.execution_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query("UPDATE executions SET state = 'completed' WHERE id = $1")
            .bind(task.execution_id)
            .execute(&mut *tx)
            .await?;

        // Cancel any pending boundary timer jobs attached to this task's element.
        if let Some(boundary_ids) = graph.attached_to.get(&task.element_id) {
            for boundary_id in boundary_ids {
                sqlx::query(
                    "UPDATE jobs SET state = 'cancelled' \
                     WHERE execution_id = (SELECT id FROM executions \
                         WHERE instance_id = $1 AND element_id = $2 \
                         LIMIT 1) \
                     AND state IN ('pending', 'locked')",
                )
                .bind(task.instance_id)
                .bind(boundary_id)
                .execute(&mut *tx)
                .await?;
            }
        }

        let next_ids: Vec<String> = graph
            .outgoing
            .get(&task.element_id)
            .cloned()
            .unwrap_or_default();

        for next_id in next_ids {
            Self::run_to_wait(&mut tx, task.instance_id, &next_id, &graph).await?;
        }

        tx.commit().await?;
        Ok(())
    }

    /// Complete a locked external (service) task and advance the token.
    pub async fn complete_external_task(
        &self,
        job_id: Uuid,
        worker_id: &str,
        variables: &[VariableInput],
    ) -> Result<()> {
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

        let exec_row: (String,) = sqlx::query_as("SELECT element_id FROM executions WHERE id = $1")
            .bind(job.execution_id)
            .fetch_one(&self.pool)
            .await?;
        let element_id = exec_row.0;

        let mut tx = self.pool.begin().await?;

        for var in variables {
            sqlx::query(
                "INSERT INTO variables (instance_id, execution_id, name, value_type, value) \
                 VALUES ($1, $2, $3, $4, $5) \
                 ON CONFLICT (execution_id, name) \
                 DO UPDATE SET value_type = EXCLUDED.value_type, value = EXCLUDED.value",
            )
            .bind(job.instance_id)
            .bind(job.execution_id)
            .bind(&var.name)
            .bind(&var.value_type)
            .bind(&var.value)
            .execute(&mut *tx)
            .await?;
        }

        sqlx::query(
            "UPDATE execution_history SET left_at = NOW() \
             WHERE execution_id = $1 AND left_at IS NULL",
        )
        .bind(job.execution_id)
        .execute(&mut *tx)
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

        let next_ids: Vec<String> = graph.outgoing.get(&element_id).cloned().unwrap_or_default();

        for next_id in next_ids {
            Self::run_to_wait(&mut tx, job.instance_id, &next_id, &graph).await?;
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

    /// Advance the token starting from `element_id` until it reaches a wait state
    /// (UserTask, ServiceTask) or a terminal state (EndEvent).
    async fn run_to_wait(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        instance_id: Uuid,
        element_id: &str,
        graph: &ProcessGraph,
    ) -> Result<()> {
        let mut current_id = element_id.to_string();

        loop {
            let node = graph.nodes.get(&current_id).ok_or_else(|| {
                EngineError::Internal(format!("Element '{current_id}' not found in process graph"))
            })?;

            let execution = sqlx::query_as::<_, Execution>(
                "INSERT INTO executions (instance_id, element_id, state) \
                 VALUES ($1, $2, 'active') RETURNING *",
            )
            .bind(instance_id)
            .bind(&node.id)
            .fetch_one(&mut **tx)
            .await?;

            let element_type = Self::element_type_str(&node.kind);

            match &node.kind {
                FlowNodeKind::StartEvent | FlowNodeKind::EndEvent => {
                    // Instantaneous: enter and leave in the same instant.
                    sqlx::query(
                        "INSERT INTO execution_history \
                         (instance_id, execution_id, element_id, element_type, entered_at, left_at) \
                         VALUES ($1, $2, $3, $4, NOW(), NOW())",
                    )
                    .bind(instance_id)
                    .bind(execution.id)
                    .bind(&node.id)
                    .bind(element_type)
                    .execute(&mut **tx)
                    .await?;

                    sqlx::query("UPDATE executions SET state = 'completed' WHERE id = $1")
                        .bind(execution.id)
                        .execute(&mut **tx)
                        .await?;

                    if matches!(node.kind, FlowNodeKind::EndEvent) {
                        sqlx::query(
                            "UPDATE process_instances \
                             SET state = 'completed', ended_at = NOW() \
                             WHERE id = $1",
                        )
                        .bind(instance_id)
                        .execute(&mut **tx)
                        .await?;
                        return Ok(());
                    }

                    // StartEvent: continue to the next element.
                    let next_ids = graph.outgoing.get(&node.id).cloned().unwrap_or_default();
                    match next_ids.into_iter().next() {
                        Some(next_id) => {
                            current_id = next_id;
                        }
                        None => return Ok(()),
                    }
                }

                FlowNodeKind::UserTask => {
                    sqlx::query(
                        "INSERT INTO execution_history \
                         (instance_id, execution_id, element_id, element_type) \
                         VALUES ($1, $2, $3, $4)",
                    )
                    .bind(instance_id)
                    .bind(execution.id)
                    .bind(&node.id)
                    .bind(element_type)
                    .execute(&mut **tx)
                    .await?;

                    sqlx::query(
                        "INSERT INTO tasks \
                         (instance_id, execution_id, element_id, name, task_type, state) \
                         VALUES ($1, $2, $3, $4, $5, 'pending')",
                    )
                    .bind(instance_id)
                    .bind(execution.id)
                    .bind(&node.id)
                    .bind(node.name.as_deref())
                    .bind("user_task")
                    .execute(&mut **tx)
                    .await?;

                    // Insert boundary timer jobs for any boundary events attached to this task.
                    if let Some(boundary_ids) = graph.attached_to.get(&node.id) {
                        for boundary_id in boundary_ids {
                            let boundary_node =
                                graph.nodes.get(boundary_id.as_str()).ok_or_else(|| {
                                    EngineError::Internal(format!(
                                        "Boundary event '{boundary_id}' not found in graph"
                                    ))
                                })?;

                            if let FlowNodeKind::BoundaryTimerEvent { duration, .. } =
                                &boundary_node.kind
                            {
                                let dur = parse_duration(duration)?;
                                let due_date = chrono::Utc::now() + dur;

                                let boundary_exec = sqlx::query_as::<_, Execution>(
                                    "INSERT INTO executions (instance_id, element_id, state) \
                                     VALUES ($1, $2, 'active') RETURNING *",
                                )
                                .bind(instance_id)
                                .bind(boundary_id)
                                .fetch_one(&mut **tx)
                                .await?;

                                sqlx::query(
                                    "INSERT INTO jobs \
                                     (instance_id, execution_id, job_type, due_date, retries, state) \
                                     VALUES ($1, $2, 'timer', $3, 1, 'pending')",
                                )
                                .bind(instance_id)
                                .bind(boundary_exec.id)
                                .bind(due_date)
                                .execute(&mut **tx)
                                .await?;
                            }
                        }
                    }

                    return Ok(());
                }

                FlowNodeKind::ServiceTask { topic } => {
                    sqlx::query(
                        "INSERT INTO execution_history \
                         (instance_id, execution_id, element_id, element_type) \
                         VALUES ($1, $2, $3, $4)",
                    )
                    .bind(instance_id)
                    .bind(execution.id)
                    .bind(&node.id)
                    .bind(element_type)
                    .execute(&mut **tx)
                    .await?;

                    sqlx::query(
                        "INSERT INTO jobs \
                         (instance_id, execution_id, job_type, topic, due_date, retries, state) \
                         VALUES ($1, $2, 'external_task', $3, NOW(), 3, 'pending')",
                    )
                    .bind(instance_id)
                    .bind(execution.id)
                    .bind(topic.as_deref())
                    .execute(&mut **tx)
                    .await?;

                    return Ok(());
                }

                FlowNodeKind::IntermediateTimerCatchEvent { duration } => {
                    sqlx::query(
                        "INSERT INTO execution_history \
                         (instance_id, execution_id, element_id, element_type) \
                         VALUES ($1, $2, $3, $4)",
                    )
                    .bind(instance_id)
                    .bind(execution.id)
                    .bind(&node.id)
                    .bind(element_type)
                    .execute(&mut **tx)
                    .await?;

                    let dur = parse_duration(duration)?;
                    let due_date = chrono::Utc::now() + dur;

                    sqlx::query(
                        "INSERT INTO jobs \
                         (instance_id, execution_id, job_type, due_date, retries, state) \
                         VALUES ($1, $2, 'timer', $3, 1, 'pending')",
                    )
                    .bind(instance_id)
                    .bind(execution.id)
                    .bind(due_date)
                    .execute(&mut **tx)
                    .await?;

                    return Ok(());
                }

                FlowNodeKind::BoundaryTimerEvent { .. } => {
                    // Boundary events are never entered via run_to_wait directly;
                    // they are set up alongside the host UserTask.
                    return Ok(());
                }

                FlowNodeKind::ExclusiveGateway { default_flow } => {
                    sqlx::query(
                        "INSERT INTO execution_history \
                         (instance_id, execution_id, element_id, element_type, entered_at, left_at) \
                         VALUES ($1, $2, $3, $4, NOW(), NOW())",
                    )
                    .bind(instance_id)
                    .bind(execution.id)
                    .bind(&node.id)
                    .bind(element_type)
                    .execute(&mut **tx)
                    .await?;

                    sqlx::query("UPDATE executions SET state = 'completed' WHERE id = $1")
                        .bind(execution.id)
                        .execute(&mut **tx)
                        .await?;

                    // Load all instance variables visible within this transaction.
                    let vars: Vec<Variable> = sqlx::query_as::<_, Variable>(
                        "SELECT * FROM variables WHERE instance_id = $1",
                    )
                    .bind(instance_id)
                    .fetch_all(&mut **tx)
                    .await?;

                    let var_map: HashMap<String, JsonValue> =
                        vars.into_iter().map(|v| (v.name, v.value)).collect();

                    // Collect outgoing flows for this gateway (maintain declaration order).
                    let outgoing_flows: Vec<_> = graph
                        .flows
                        .iter()
                        .filter(|f| f.source_ref == node.id)
                        .collect();

                    let mut chosen: Option<String> = None;
                    let mut default_target: Option<String> = None;

                    for flow in &outgoing_flows {
                        if default_flow.as_deref() == Some(flow.id.as_str()) {
                            default_target = Some(flow.target_ref.clone());
                            continue;
                        }
                        if let Some(expr) = &flow.condition {
                            if evaluator::evaluate_condition(expr, &var_map)? {
                                chosen = Some(flow.target_ref.clone());
                                break;
                            }
                        }
                    }

                    match chosen.or(default_target) {
                        Some(target) => {
                            current_id = target;
                        }
                        None => {
                            sqlx::query(
                                "UPDATE process_instances \
                                 SET state = 'error', ended_at = NOW() WHERE id = $1",
                            )
                            .bind(instance_id)
                            .execute(&mut **tx)
                            .await?;
                            return Ok(());
                        }
                    }
                }
            }
        }
    }

    fn element_type_str(kind: &FlowNodeKind) -> &'static str {
        match kind {
            FlowNodeKind::StartEvent => "startEvent",
            FlowNodeKind::EndEvent => "endEvent",
            FlowNodeKind::UserTask => "userTask",
            FlowNodeKind::ServiceTask { .. } => "serviceTask",
            FlowNodeKind::ExclusiveGateway { .. } => "exclusiveGateway",
            FlowNodeKind::IntermediateTimerCatchEvent { .. } => "intermediateCatchEvent",
            FlowNodeKind::BoundaryTimerEvent { .. } => "boundaryEvent",
        }
    }

    /// Fire a specific timer job by ID, regardless of its due_date.
    /// Used by the background executor after it has already claimed the job via SKIP LOCKED.
    pub async fn fire_timer_job(&self, job_id: Uuid) -> Result<()> {
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

        let exec_row: (String,) = sqlx::query_as("SELECT element_id FROM executions WHERE id = $1")
            .bind(job.execution_id)
            .fetch_one(&self.pool)
            .await?;
        let element_id = exec_row.0;

        let node = graph.nodes.get(&element_id).ok_or_else(|| {
            EngineError::Internal(format!("Element '{element_id}' not found in process graph"))
        })?;

        let mut tx = self.pool.begin().await?;

        match &node.kind {
            FlowNodeKind::IntermediateTimerCatchEvent { .. } => {
                sqlx::query(
                    "UPDATE execution_history SET left_at = NOW() \
                     WHERE execution_id = $1 AND left_at IS NULL",
                )
                .bind(job.execution_id)
                .execute(&mut *tx)
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

                let next_ids: Vec<String> =
                    graph.outgoing.get(&element_id).cloned().unwrap_or_default();
                for next_id in next_ids {
                    Self::run_to_wait(&mut tx, job.instance_id, &next_id, &graph).await?;
                }
            }

            FlowNodeKind::BoundaryTimerEvent {
                attached_to,
                cancelling,
                ..
            } => {
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

                let next_ids: Vec<String> =
                    graph.outgoing.get(&element_id).cloned().unwrap_or_default();
                for next_id in next_ids {
                    Self::run_to_wait(&mut tx, job.instance_id, &next_id, &graph).await?;
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

        let count = jobs.len();
        for job in jobs {
            self.fire_timer_job(job.id).await?;
        }
        Ok(count)
    }
}
