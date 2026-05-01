use jsonschema::Validator;
use serde_json::Value as JsonValue;
use uuid::Uuid;

use tracing::{debug, info};

use crate::db::models::{Execution, ProcessInstance};
use crate::error::{EngineError, Result};
use crate::parser::FlowNodeKind;

use super::{Engine, VariableInput};

impl Engine {
    /// Create a new process instance and advance the token until it reaches
    /// a wait state (UserTask, ServiceTask) or completes (EndEvent).
    pub async fn start_instance(
        &self,
        definition_id: Uuid,
        org_id: Uuid,
        labels: &JsonValue,
        initial_variables: &[VariableInput],
    ) -> Result<ProcessInstance> {
        let graph = self.load_graph(definition_id).await?;

        if let Some(ref schema) = graph.input_schema {
            let vars_obj = JsonValue::Object(
                initial_variables
                    .iter()
                    .map(|v| (v.name.clone(), v.value.clone()))
                    .collect(),
            );
            let validator = Validator::new(schema).map_err(|e| {
                EngineError::Validation(format!("Invalid input schema in process definition: {e}"))
            })?;
            let msgs: Vec<String> = validator
                .iter_errors(&vars_obj)
                .map(|e| e.to_string())
                .collect();
            if !msgs.is_empty() {
                return Err(EngineError::Validation(format!(
                    "Input variables do not conform to process schema: {}",
                    msgs.join("; ")
                )));
            }
        }

        let start_node = graph
            .nodes
            .values()
            .find(|n| {
                matches!(
                    n.kind,
                    FlowNodeKind::StartEvent
                        | FlowNodeKind::TimerStartEvent { .. }
                        | FlowNodeKind::MessageStartEvent { .. }
                        | FlowNodeKind::SignalStartEvent { .. }
                )
            })
            .ok_or_else(|| EngineError::Internal("No start event in process graph".to_string()))?
            .clone();

        debug!(definition_id = %definition_id, variables = initial_variables.len(), "starting new process instance");

        let mut tx = self.pool.begin().await?;

        let instance = sqlx::query_as::<_, ProcessInstance>(
            "INSERT INTO process_instances (org_id, definition_id, state, labels) VALUES ($1, $2, 'running', $3) RETURNING *",
        )
        .bind(org_id)
        .bind(definition_id)
        .bind(labels)
        .fetch_one(&mut *tx)
        .await?;

        info!(instance_id = %instance.id, definition_id = %definition_id, "process instance created");

        // Write initial variables scoped to a synthetic "start" execution.
        if !initial_variables.is_empty() {
            let start_exec = sqlx::query_as::<_, Execution>(
                "INSERT INTO executions (instance_id, element_id, state) \
                 VALUES ($1, '__start__', 'completed') RETURNING *",
            )
            .bind(instance.id)
            .fetch_one(&mut *tx)
            .await?;

            for var in initial_variables {
                crate::db::variables::upsert_in_tx(
                    &mut tx,
                    instance.id,
                    start_exec.id,
                    None,
                    &var.name,
                    &var.value_type,
                    &var.value,
                )
                .await?;
            }
        }

        Self::run_to_wait(&mut tx, instance.id, &start_node.id, &graph, None).await?;

        // Re-fetch within the transaction to capture any state update (e.g. Start→End).
        let final_instance =
            sqlx::query_as::<_, ProcessInstance>("SELECT * FROM process_instances WHERE id = $1")
                .bind(instance.id)
                .fetch_one(&mut *tx)
                .await?;

        tx.commit().await?;
        Ok(final_instance)
    }
}
