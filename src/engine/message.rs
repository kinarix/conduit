use uuid::Uuid;

use tracing::{debug, info};

use crate::db::models::{EventSubscription, Execution, ProcessInstance};
use crate::error::{EngineError, Result};
use crate::parser::FlowNodeKind;

use super::{Engine, VariableInput};

impl Engine {
    /// Correlate an inbound message to a waiting instance and advance its token.
    ///
    /// Lookup order:
    /// 1. Find an `event_subscriptions` row matching `message_name` + `correlation_key` and
    ///    atomically claim it (DELETE … RETURNING inside a transaction).
    /// 2. If no subscription found, scan deployed process definitions for one whose start event is
    ///    a `MessageStartEvent` with a matching name, then create a new instance.
    /// 3. If neither matches, return `NotFound`.
    pub async fn correlate_message(
        &self,
        message_name: &str,
        correlation_key: Option<&str>,
        variables: &[VariableInput],
        org_id: Uuid,
    ) -> Result<()> {
        debug!(message_name, ?correlation_key, %org_id, "correlating message");
        // Attempt 1: find a waiting subscription and claim it atomically.
        let mut tx = self.pool.begin().await?;

        let sub: Option<EventSubscription> = sqlx::query_as(
            "DELETE FROM event_subscriptions \
             WHERE id = ( \
                 SELECT es.id FROM event_subscriptions es \
                 JOIN process_instances pi ON pi.id = es.instance_id \
                 WHERE es.event_type = 'message' \
                   AND es.event_name = $1 \
                   AND ($2::text IS NULL OR es.correlation_key = $2) \
                   AND pi.org_id = $3 \
                   AND pi.state = 'running' \
                 ORDER BY es.created_at ASC \
                 LIMIT 1 \
                 FOR UPDATE OF es SKIP LOCKED \
             ) RETURNING *",
        )
        .bind(message_name)
        .bind(correlation_key)
        .bind(org_id)
        .fetch_optional(&mut *tx)
        .await?;

        if let Some(sub) = sub {
            info!(instance_id = %sub.instance_id, element_id = %sub.element_id, message_name, "message correlated to waiting subscription");
            let payload_json = serde_json::to_value(variables).unwrap_or(serde_json::json!([]));
            crate::db::process_events::record_correlation(
                &mut *tx,
                sub.instance_id,
                Some(sub.execution_id),
                Some(&sub.element_id),
                "message_received",
                message_name,
                correlation_key,
                payload_json,
            )
            .await?;
            let def_row: (Uuid,) =
                sqlx::query_as("SELECT definition_id FROM process_instances WHERE id = $1")
                    .bind(sub.instance_id)
                    .fetch_one(&mut *tx)
                    .await?;

            let graph = self.load_graph(def_row.0).await?;

            let wait_exec: Execution = sqlx::query_as("SELECT * FROM executions WHERE id = $1")
                .bind(sub.execution_id)
                .fetch_one(&mut *tx)
                .await?;
            let parallel_scope = wait_exec.parent_id;

            for var in variables {
                crate::db::variables::upsert_in_tx(
                    &mut tx,
                    sub.instance_id,
                    sub.execution_id,
                    Some(&sub.element_id),
                    &var.name,
                    &var.value_type,
                    &var.value,
                )
                .await?;
            }

            crate::db::execution_history::record_exit(
                &mut tx,
                sub.instance_id,
                sub.execution_id,
                &sub.element_id,
                "intermediateCatchEvent",
            )
            .await?;

            sqlx::query("UPDATE executions SET state = 'completed' WHERE id = $1")
                .bind(sub.execution_id)
                .execute(&mut *tx)
                .await?;

            let (current_graph, _) =
                Self::find_element_graph(&sub.element_id, &graph).ok_or_else(|| {
                    EngineError::Internal(format!(
                        "Element '{}' not found in process graph",
                        sub.element_id
                    ))
                })?;
            let next_ids: Vec<String> = current_graph
                .outgoing
                .get(&sub.element_id)
                .cloned()
                .unwrap_or_default();

            for next_id in next_ids {
                Self::run_to_wait(&mut tx, sub.instance_id, &next_id, &graph, parallel_scope)
                    .await?;
            }

            tx.commit().await?;
            return Ok(());
        }

        tx.rollback().await.ok();

        // Attempt 2: MessageStartEvent — scan deployed, enabled definitions for the org.
        let defs: Vec<(Uuid, String)> = sqlx::query_as(
            "SELECT id, bpmn_xml FROM process_definitions \
             WHERE org_id = $1 AND status = 'deployed' AND disabled_at IS NULL",
        )
        .bind(org_id)
        .fetch_all(&self.pool)
        .await?;

        for (def_id, bpmn_xml) in &defs {
            let graph = match crate::parser::parse(bpmn_xml) {
                Ok(g) => g,
                Err(_) => continue,
            };

            let start_node = graph.nodes.values().find(|n| {
                matches!(
                    &n.kind,
                    FlowNodeKind::MessageStartEvent { message_name: mn }
                        if mn == message_name
                )
            });

            if let Some(start_node) = start_node {
                let start_id = start_node.id.clone();
                let labels = serde_json::json!({});
                let instance = sqlx::query_as::<_, ProcessInstance>(
                    "INSERT INTO process_instances \
                     (org_id, definition_id, state, labels) \
                     VALUES ($1, $2, 'running', $3) RETURNING *",
                )
                .bind(org_id)
                .bind(def_id)
                .bind(&labels)
                .fetch_one(&self.pool)
                .await?;

                let mut tx2 = self.pool.begin().await?;

                // Write the message variables into the start execution context.
                // We need the start execution id — it gets created in run_to_wait, so write
                // variables after the run using the instance id scope.
                info!(definition_id = %def_id, instance_id = %instance.id, message_name, "message start event triggered new instance");
                let payload_json = serde_json::to_value(variables).unwrap_or(serde_json::json!([]));
                crate::db::process_events::record_correlation(
                    &mut *tx2,
                    instance.id,
                    None,
                    Some(&start_id),
                    "message_received",
                    message_name,
                    correlation_key,
                    payload_json,
                )
                .await?;
                Self::run_to_wait(&mut tx2, instance.id, &start_id, &graph, None).await?;

                // Write message variables at instance scope (execution_id = first execution).
                let first_exec_id: Uuid = sqlx::query_scalar(
                    "SELECT id FROM executions WHERE instance_id = $1 \
                     ORDER BY created_at ASC LIMIT 1",
                )
                .bind(instance.id)
                .fetch_one(&mut *tx2)
                .await?;

                for var in variables {
                    crate::db::variables::upsert_in_tx(
                        &mut tx2,
                        instance.id,
                        first_exec_id,
                        Some(&start_id),
                        &var.name,
                        &var.value_type,
                        &var.value,
                    )
                    .await?;
                }

                tx2.commit().await?;
                return Ok(());
            }
        }

        Err(EngineError::NotFound(format!(
            "No waiting subscription or MessageStartEvent found for message '{message_name}'"
        )))
    }
}
