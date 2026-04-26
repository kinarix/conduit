mod evaluator;

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use uuid::Uuid;

use crate::db::models::{Execution, ProcessInstance, Task, Variable};
use crate::error::{EngineError, Result};
use crate::parser::{FlowNodeKind, ProcessGraph};
use crate::state::GraphCache;

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

                FlowNodeKind::UserTask | FlowNodeKind::ServiceTask { .. } => {
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

                    let task_type = if matches!(node.kind, FlowNodeKind::UserTask) {
                        "user_task"
                    } else {
                        "service_task"
                    };

                    sqlx::query(
                        "INSERT INTO tasks \
                         (instance_id, execution_id, element_id, name, task_type, state) \
                         VALUES ($1, $2, $3, $4, $5, 'pending')",
                    )
                    .bind(instance_id)
                    .bind(execution.id)
                    .bind(&node.id)
                    .bind(node.name.as_deref())
                    .bind(task_type)
                    .execute(&mut **tx)
                    .await?;

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
        }
    }
}
