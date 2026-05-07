use uuid::Uuid;

use tracing::{debug, info};

use crate::db::models::{EventSubscription, Execution, ProcessInstance};
use crate::error::{EngineError, Result};
use crate::parser::FlowNodeKind;

use super::{Engine, VariableInput};

impl Engine {
    /// Broadcast a signal to all waiting instances within the org.
    ///
    /// Unlike messages (exclusive), signals go to EVERY matching subscription AND start every
    /// matching `SignalStartEvent` process. Always returns `Ok` — no error if nobody is listening.
    pub async fn broadcast_signal(
        &self,
        signal_name: &str,
        variables: &[VariableInput],
        org_id: Uuid,
    ) -> Result<()> {
        debug!(signal_name, %org_id, "broadcasting signal");
        // Phase 1: drain all waiting subscriptions one at a time, each in its own transaction.
        loop {
            let mut tx = self.pool.begin().await?;

            let sub: Option<EventSubscription> = sqlx::query_as(
                "DELETE FROM event_subscriptions \
                 WHERE id = ( \
                     SELECT es.id FROM event_subscriptions es \
                     JOIN process_instances pi ON pi.id = es.instance_id \
                     WHERE es.event_type = 'signal' \
                       AND es.event_name = $1 \
                       AND pi.org_id = $2 \
                       AND pi.state = 'running' \
                     ORDER BY es.created_at ASC \
                     LIMIT 1 \
                     FOR UPDATE OF es SKIP LOCKED \
                 ) RETURNING *",
            )
            .bind(signal_name)
            .bind(org_id)
            .fetch_optional(&mut *tx)
            .await?;

            let sub = match sub {
                Some(s) => s,
                None => {
                    tx.rollback().await.ok();
                    break;
                }
            };

            info!(instance_id = %sub.instance_id, element_id = %sub.element_id, signal_name, "signal received by waiting subscription");
            let payload_json = serde_json::to_value(variables).unwrap_or(serde_json::json!([]));
            crate::db::process_events::record_correlation(
                &mut *tx,
                sub.instance_id,
                Some(sub.execution_id),
                Some(sub.element_id.as_str()),
                "signal_received",
                signal_name,
                None,
                payload_json,
            )
            .await?;

            let def_row: (Uuid,) =
                sqlx::query_as("SELECT definition_id FROM process_instances WHERE id = $1")
                    .bind(sub.instance_id)
                    .fetch_one(&mut *tx)
                    .await?;

            let graph = self.load_graph(def_row.0).await?;

            let (current_graph, _) = Self::find_element_graph(sub.element_id.as_str(), &graph)
                .ok_or_else(|| {
                    EngineError::Internal(format!(
                        "Element '{}' not found in graph",
                        sub.element_id
                    ))
                })?;
            let node = current_graph.nodes.get(sub.element_id.as_str()).unwrap();

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

            match &node.kind {
                FlowNodeKind::BoundarySignalEvent {
                    attached_to,
                    cancelling,
                    ..
                } => {
                    let attached_to = attached_to.clone();
                    if *cancelling {
                        sqlx::query(
                            "UPDATE tasks SET state = 'cancelled' \
                             WHERE execution_id = (SELECT id FROM executions \
                                 WHERE instance_id = $1 AND element_id = $2 AND state = 'active' \
                                 LIMIT 1)",
                        )
                        .bind(sub.instance_id)
                        .bind(&attached_to)
                        .execute(&mut *tx)
                        .await?;

                        sqlx::query(
                            "UPDATE execution_history SET left_at = NOW() \
                             WHERE execution_id = (SELECT id FROM executions \
                                 WHERE instance_id = $1 AND element_id = $2 AND state = 'active' \
                                 LIMIT 1) AND left_at IS NULL",
                        )
                        .bind(sub.instance_id)
                        .bind(&attached_to)
                        .execute(&mut *tx)
                        .await?;

                        sqlx::query(
                            "UPDATE executions SET state = 'cancelled' \
                             WHERE instance_id = $1 AND element_id = $2 AND state = 'active'",
                        )
                        .bind(sub.instance_id)
                        .bind(&attached_to)
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
                                    .bind(sub.instance_id)
                                    .bind(bid)
                                    .fetch_all(&mut *tx)
                                    .await?;
                                    crate::db::jobs::record_bulk_cancelled(&mut tx, &cancelled_ids)
                                        .await?;

                                    sqlx::query(
                                        "DELETE FROM event_subscriptions \
                                         WHERE instance_id = $1 AND element_id = $2",
                                    )
                                    .bind(sub.instance_id)
                                    .bind(bid)
                                    .execute(&mut *tx)
                                    .await?;
                                }
                            }
                        }
                    }

                    crate::db::execution_history::record_exit(
                        &mut tx,
                        sub.instance_id,
                        sub.execution_id,
                        sub.element_id.as_str(),
                        "signalEvent",
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
                        Self::run_to_wait(&mut tx, sub.instance_id, &next_id, &graph, None).await?;
                    }
                }

                _ => {
                    // IntermediateSignalCatchEvent: close history, complete execution, advance.
                    let wait_exec: Execution =
                        sqlx::query_as("SELECT * FROM executions WHERE id = $1")
                            .bind(sub.execution_id)
                            .fetch_one(&mut *tx)
                            .await?;
                    let parallel_scope = wait_exec.parent_id;

                    crate::db::execution_history::record_exit(
                        &mut tx,
                        sub.instance_id,
                        sub.execution_id,
                        sub.element_id.as_str(),
                        "signalEvent",
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
                        Self::run_to_wait(
                            &mut tx,
                            sub.instance_id,
                            &next_id,
                            &graph,
                            parallel_scope,
                        )
                        .await?;
                    }
                }
            }

            tx.commit().await?;
        }

        // Phase 2: start new instances from matching SignalStartEvent definitions
        // (deployed and not disabled).
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
                    FlowNodeKind::SignalStartEvent { signal_name: sn } if sn == signal_name
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

                info!(definition_id = %def_id, instance_id = %instance.id, signal_name, "signal start event triggered new instance");

                let mut tx2 = self.pool.begin().await?;

                let payload_json = serde_json::to_value(variables).unwrap_or(serde_json::json!([]));
                crate::db::process_events::record_correlation(
                    &mut *tx2,
                    instance.id,
                    None,
                    Some(&start_id),
                    "signal_received",
                    signal_name,
                    None,
                    payload_json,
                )
                .await?;
                Self::run_to_wait(&mut tx2, instance.id, &start_id, &graph, None).await?;

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
            }
        }

        Ok(())
    }
}
