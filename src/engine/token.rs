use serde_json::Value as JsonValue;
use uuid::Uuid;

use tracing::{debug, info};

use crate::db::models::{DecisionDefinition, Execution};
use crate::error::{EngineError, Result};
use crate::parser::{FlowNodeKind, ProcessGraph};

use super::evaluator;
use super::Engine;

impl Engine {
    /// Advance tokens starting from `start_element_id` until all active paths reach
    /// a wait state (UserTask, ServiceTask) or a terminal state (EndEvent).
    /// `parallel_scope` carries the fork execution ID for tracking join synchronisation.
    pub(super) async fn run_to_wait(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        instance_id: Uuid,
        start_element_id: &str,
        graph: &ProcessGraph,
        parallel_scope: Option<Uuid>,
    ) -> Result<()> {
        // Work-stack: (element_id, scope). Avoids recursive async functions.
        let mut stack: Vec<(String, Option<Uuid>)> =
            vec![(start_element_id.to_string(), parallel_scope)];

        while let Some((current_id, scope)) = stack.pop() {
            let (current_graph, outer_chain) = Self::find_element_graph(&current_id, graph)
                .ok_or_else(|| {
                    EngineError::Internal(format!(
                        "Element '{current_id}' not found in process graph"
                    ))
                })?;
            let node = current_graph
                .nodes
                .get(&current_id)
                .expect("find_element_graph contract: element exists in returned graph");

            debug!(
                instance_id = %instance_id,
                element_id = %current_id,
                element_type = Self::element_type_str(&node.kind),
                "token advancing to element"
            );

            let execution = sqlx::query_as::<_, Execution>(
                "INSERT INTO executions (instance_id, element_id, state, parent_id) \
                 VALUES ($1, $2, 'active', $3) RETURNING *",
            )
            .bind(instance_id)
            .bind(&node.id)
            .bind(scope)
            .fetch_one(&mut **tx)
            .await?;

            let element_type = Self::element_type_str(&node.kind);

            match &node.kind {
                FlowNodeKind::StartEvent | FlowNodeKind::EndEvent => {
                    crate::db::execution_history::record_entry(
                        tx,
                        instance_id,
                        execution.id,
                        &node.id,
                        element_type,
                        true,
                    )
                    .await?;

                    sqlx::query("UPDATE executions SET state = 'completed' WHERE id = $1")
                        .bind(execution.id)
                        .execute(&mut **tx)
                        .await?;

                    if matches!(node.kind, FlowNodeKind::EndEvent) {
                        if let Some(&outer_graph) = outer_chain.last() {
                            // Inner EndEvent of a subprocess: check if all inner paths finished.
                            let sp_exec_id = scope.ok_or_else(|| {
                                EngineError::Internal(format!(
                                    "Subprocess EndEvent '{}' reached with no scope",
                                    node.id
                                ))
                            })?;

                            let (active_inner,): (i64,) = sqlx::query_as(
                                "SELECT COUNT(*) FROM executions \
                                 WHERE parent_id = $1 AND state = 'active'",
                            )
                            .bind(sp_exec_id)
                            .fetch_one(&mut **tx)
                            .await?;

                            if active_inner == 0 {
                                sqlx::query(
                                    "UPDATE execution_history SET left_at = NOW() \
                                     WHERE execution_id = $1 AND left_at IS NULL",
                                )
                                .bind(sp_exec_id)
                                .execute(&mut **tx)
                                .await?;

                                sqlx::query(
                                    "UPDATE executions SET state = 'completed' WHERE id = $1",
                                )
                                .bind(sp_exec_id)
                                .execute(&mut **tx)
                                .await?;

                                let (sp_element_id, outer_scope): (String, Option<Uuid>) =
                                    sqlx::query_as(
                                        "SELECT element_id, parent_id FROM executions WHERE id = $1",
                                    )
                                    .bind(sp_exec_id)
                                    .fetch_one(&mut **tx)
                                    .await?;

                                for next_id in outer_graph
                                    .outgoing
                                    .get(&sp_element_id)
                                    .cloned()
                                    .unwrap_or_default()
                                {
                                    stack.push((next_id, outer_scope));
                                }
                            }
                            // else: other inner paths still active — don't advance yet.
                        } else {
                            // Top-level EndEvent: complete instance when nothing else is running.
                            // This preserves non-interrupting boundary event paths: the boundary
                            // path can reach an EndEvent while the host task is still active.
                            let (active_count,): (i64,) = sqlx::query_as(
                                "SELECT COUNT(*) FROM executions \
                                 WHERE instance_id = $1 AND state = 'active'",
                            )
                            .bind(instance_id)
                            .fetch_one(&mut **tx)
                            .await?;

                            if active_count == 0 {
                                info!(instance_id = %instance_id, "process instance completed");
                                sqlx::query(
                                    "UPDATE process_instances \
                                     SET state = 'completed', ended_at = NOW() \
                                     WHERE id = $1",
                                )
                                .bind(instance_id)
                                .execute(&mut **tx)
                                .await?;
                            }
                        }
                        // Don't push anything — this token is done.
                    } else {
                        for next_id in current_graph
                            .outgoing
                            .get(&node.id)
                            .cloned()
                            .unwrap_or_default()
                        {
                            stack.push((next_id, scope));
                        }
                    }
                }

                FlowNodeKind::UserTask => {
                    crate::db::execution_history::record_entry(
                        tx,
                        instance_id,
                        execution.id,
                        &node.id,
                        element_type,
                        false,
                    )
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

                    if let Some(boundary_ids) = current_graph.attached_to.get(&node.id) {
                        for boundary_id in boundary_ids {
                            let boundary_node = current_graph
                                .nodes
                                .get(boundary_id.as_str())
                                .ok_or_else(|| {
                                    EngineError::Internal(format!(
                                        "Boundary event '{boundary_id}' not found in graph"
                                    ))
                                })?;

                            if let FlowNodeKind::BoundaryTimerEvent { timer, .. } =
                                &boundary_node.kind
                            {
                                let (due_date, timer_expression, repetitions_remaining) =
                                    super::helpers::timer_spec_schedule(timer)?;

                                let boundary_exec = sqlx::query_as::<_, Execution>(
                                    "INSERT INTO executions (instance_id, element_id, state) \
                                     VALUES ($1, $2, 'active') RETURNING *",
                                )
                                .bind(instance_id)
                                .bind(boundary_id)
                                .fetch_one(&mut **tx)
                                .await?;

                                let job_id: Uuid = sqlx::query_scalar(
                                    "INSERT INTO jobs \
                                     (instance_id, execution_id, job_type, due_date, \
                                      timer_expression, repetitions_remaining, retries, state) \
                                     VALUES ($1, $2, 'timer', $3, $4, $5, 1, 'pending') \
                                     RETURNING id",
                                )
                                .bind(instance_id)
                                .bind(boundary_exec.id)
                                .bind(due_date)
                                .bind(timer_expression)
                                .bind(repetitions_remaining)
                                .fetch_one(&mut **tx)
                                .await?;
                                crate::db::process_events::record_job(
                                    &mut **tx,
                                    instance_id,
                                    Some(boundary_exec.id),
                                    Some(boundary_id.as_str()),
                                    Some(job_id),
                                    "timer",
                                    "job_created",
                                    serde_json::json!({}),
                                )
                                .await?;
                            } else if let FlowNodeKind::BoundarySignalEvent {
                                signal_name, ..
                            } = &boundary_node.kind
                            {
                                let boundary_exec = sqlx::query_as::<_, Execution>(
                                    "INSERT INTO executions (instance_id, element_id, state) \
                                     VALUES ($1, $2, 'active') RETURNING *",
                                )
                                .bind(instance_id)
                                .bind(boundary_id)
                                .fetch_one(&mut **tx)
                                .await?;

                                sqlx::query(
                                    "INSERT INTO event_subscriptions \
                                     (instance_id, execution_id, event_type, event_name, element_id) \
                                     VALUES ($1, $2, 'signal', $3, $4)",
                                )
                                .bind(instance_id)
                                .bind(boundary_exec.id)
                                .bind(signal_name)
                                .bind(boundary_id)
                                .execute(&mut **tx)
                                .await?;
                            }
                        }
                    }
                    info!(instance_id = %instance_id, element_id = %node.id, "user task created");
                    // Stop — task is waiting for human input.
                }

                FlowNodeKind::ServiceTask { topic, url, http } => {
                    crate::db::execution_history::record_entry(
                        tx,
                        instance_id,
                        execution.id,
                        &node.id,
                        element_type,
                        false,
                    )
                    .await?;

                    // Routing rules:
                    //   url present (with or without <conduit:http>) → http_task
                    //   <conduit:http> alone (no url) → http_task with URL inside config (future)
                    //   topic present, no url → external_task (worker pattern)
                    // The job carries:
                    //   - `topic` column: legacy URL for http_task without HttpConfig (backwards compat)
                    //                     OR worker topic for external_task
                    //   - `config` column (JSONB): HttpConfig snapshot when http is Some
                    let (job_type, stored_topic, config_json): (
                        &str,
                        Option<&str>,
                        Option<serde_json::Value>,
                    ) = if let Some(u) = url {
                        // Snapshot the URL inside config so the engine has a single
                        // source of truth once C3 lands. For now, also write to
                        // `topic` column so the existing fire_http_task read path
                        // (which reads URL from `topic`) keeps working.
                        let cfg = http.as_ref().map(|h| {
                            let mut v = serde_json::to_value(h)
                                .expect("HttpConfig serialization is infallible");
                            v["url"] = serde_json::Value::String(u.clone());
                            v
                        });
                        ("http_task", Some(u.as_str()), cfg)
                    } else if http.is_some() {
                        // <conduit:http> without a url attribute is currently a
                        // parse-time error path — we don't reach here in practice,
                        // but be defensive.
                        (
                            "http_task",
                            None,
                            http.as_ref().map(|h| {
                                serde_json::to_value(h)
                                    .expect("HttpConfig serialization is infallible")
                            }),
                        )
                    } else {
                        ("external_task", topic.as_deref(), None)
                    };

                    let job_id: Uuid = sqlx::query_scalar(
                        "INSERT INTO jobs \
                         (instance_id, execution_id, job_type, topic, config, due_date, retries, state) \
                         VALUES ($1, $2, $3, $4, $5, NOW(), 3, 'pending') \
                         RETURNING id",
                    )
                    .bind(instance_id)
                    .bind(execution.id)
                    .bind(job_type)
                    .bind(stored_topic)
                    .bind(&config_json)
                    .fetch_one(&mut **tx)
                    .await?;
                    crate::db::process_events::record_job(
                        &mut **tx,
                        instance_id,
                        Some(execution.id),
                        Some(node.id.as_str()),
                        Some(job_id),
                        job_type,
                        "job_created",
                        serde_json::json!({"topic": stored_topic}),
                    )
                    .await?;

                    if let Some(boundary_ids) = current_graph.attached_to.get(&node.id) {
                        for boundary_id in boundary_ids {
                            let boundary_node = current_graph
                                .nodes
                                .get(boundary_id.as_str())
                                .ok_or_else(|| {
                                    EngineError::Internal(format!(
                                        "Boundary event '{boundary_id}' not found in graph"
                                    ))
                                })?;

                            if let FlowNodeKind::BoundaryErrorEvent { error_code, .. } =
                                &boundary_node.kind
                            {
                                let boundary_exec = sqlx::query_as::<_, Execution>(
                                    "INSERT INTO executions (instance_id, element_id, state) \
                                     VALUES ($1, $2, 'active') RETURNING *",
                                )
                                .bind(instance_id)
                                .bind(boundary_id)
                                .fetch_one(&mut **tx)
                                .await?;

                                sqlx::query(
                                    "INSERT INTO event_subscriptions \
                                     (instance_id, execution_id, event_type, event_name, element_id) \
                                     VALUES ($1, $2, 'error', $3, $4)",
                                )
                                .bind(instance_id)
                                .bind(boundary_exec.id)
                                .bind(error_code.as_deref().unwrap_or(""))
                                .bind(boundary_id)
                                .execute(&mut **tx)
                                .await?;
                            }
                        }
                    }
                    info!(instance_id = %instance_id, element_id = %node.id, job_type, "service task queued");
                    // Stop — waiting for HTTP response / external worker.
                }

                FlowNodeKind::SendTask { message_name } => {
                    crate::db::execution_history::record_entry(
                        tx,
                        instance_id,
                        execution.id,
                        &node.id,
                        element_type,
                        false,
                    )
                    .await?;

                    // Queue a send_message job; fire-and-continue once delivered.
                    let job_id: Uuid = sqlx::query_scalar(
                        "INSERT INTO jobs \
                         (instance_id, execution_id, job_type, topic, due_date, retries, state) \
                         VALUES ($1, $2, 'send_message', $3, NOW(), 3, 'pending') \
                         RETURNING id",
                    )
                    .bind(instance_id)
                    .bind(execution.id)
                    .bind(message_name.as_str())
                    .fetch_one(&mut **tx)
                    .await?;
                    crate::db::process_events::record_job(
                        &mut **tx,
                        instance_id,
                        Some(execution.id),
                        Some(node.id.as_str()),
                        Some(job_id),
                        "send_message",
                        "job_created",
                        serde_json::json!({"message_name": message_name}),
                    )
                    .await?;

                    info!(instance_id = %instance_id, element_id = %node.id, message = %message_name, "send task queued");
                    // Stop — waiting for message delivery job.
                }

                FlowNodeKind::IntermediateTimerCatchEvent { timer } => {
                    crate::db::execution_history::record_entry(
                        tx,
                        instance_id,
                        execution.id,
                        &node.id,
                        element_type,
                        false,
                    )
                    .await?;

                    let (due_date, timer_expression, repetitions_remaining) =
                        super::helpers::timer_spec_schedule(timer)?;

                    let job_id: Uuid = sqlx::query_scalar(
                        "INSERT INTO jobs \
                         (instance_id, execution_id, job_type, due_date, \
                          timer_expression, repetitions_remaining, retries, state) \
                         VALUES ($1, $2, 'timer', $3, $4, $5, 1, 'pending') \
                         RETURNING id",
                    )
                    .bind(instance_id)
                    .bind(execution.id)
                    .bind(due_date)
                    .bind(timer_expression)
                    .bind(repetitions_remaining)
                    .fetch_one(&mut **tx)
                    .await?;
                    crate::db::process_events::record_job(
                        &mut **tx,
                        instance_id,
                        Some(execution.id),
                        Some(node.id.as_str()),
                        Some(job_id),
                        "timer",
                        "job_created",
                        serde_json::json!({"due_date": due_date}),
                    )
                    .await?;
                    debug!(instance_id = %instance_id, element_id = %node.id, %due_date, "timer catch event scheduled");
                    // Stop — waiting for timer.
                }

                FlowNodeKind::MessageStartEvent { .. } => {
                    // Entered when a MessageStartEvent triggers a new instance.
                    // Treat like a plain StartEvent — pass through immediately.
                    crate::db::execution_history::record_entry(
                        tx,
                        instance_id,
                        execution.id,
                        &node.id,
                        element_type,
                        true,
                    )
                    .await?;

                    sqlx::query("UPDATE executions SET state = 'completed' WHERE id = $1")
                        .bind(execution.id)
                        .execute(&mut **tx)
                        .await?;

                    for next_id in current_graph
                        .outgoing
                        .get(&node.id)
                        .cloned()
                        .unwrap_or_default()
                    {
                        stack.push((next_id, scope));
                    }
                }

                FlowNodeKind::IntermediateMessageCatchEvent {
                    message_name,
                    correlation_key_expr,
                }
                | FlowNodeKind::ReceiveTask {
                    message_name,
                    correlation_key_expr,
                } => {
                    crate::db::execution_history::record_entry(
                        tx,
                        instance_id,
                        execution.id,
                        &node.id,
                        element_type,
                        false,
                    )
                    .await?;

                    // Resolve correlation key: evaluate simple ${varName} expressions
                    // against current instance variables; fall back to literal value.
                    let resolved_key = if let Some(expr) = correlation_key_expr {
                        Self::resolve_correlation_key(expr, instance_id, tx).await?
                    } else {
                        None
                    };

                    sqlx::query(
                        "INSERT INTO event_subscriptions \
                         (instance_id, execution_id, event_type, event_name, correlation_key, element_id) \
                         VALUES ($1, $2, 'message', $3, $4, $5)",
                    )
                    .bind(instance_id)
                    .bind(execution.id)
                    .bind(message_name)
                    .bind(resolved_key.as_deref())
                    .bind(&node.id)
                    .execute(&mut **tx)
                    .await?;
                    debug!(instance_id = %instance_id, element_id = %node.id, message_name = %message_name, "message catch event waiting for correlation");
                    // Stop — waiting for message correlation.
                }

                FlowNodeKind::BoundaryTimerEvent { .. }
                | FlowNodeKind::BoundarySignalEvent { .. }
                | FlowNodeKind::BoundaryErrorEvent { .. } => {
                    // Never entered via run_to_wait directly; set up alongside the host task.
                }

                FlowNodeKind::SubProcess { sub_graph } => {
                    crate::db::execution_history::record_entry(
                        tx,
                        instance_id,
                        execution.id,
                        &node.id,
                        element_type,
                        false,
                    )
                    .await?;

                    let inner_start = sub_graph
                        .nodes
                        .values()
                        .find(|n| matches!(n.kind, FlowNodeKind::StartEvent))
                        .ok_or_else(|| {
                            EngineError::Internal(format!(
                                "SubProcess '{}' has no StartEvent",
                                node.id
                            ))
                        })?;
                    debug!(instance_id = %instance_id, element_id = %node.id, inner_start = %inner_start.id, "entering subprocess");
                    stack.push((inner_start.id.clone(), Some(execution.id)));
                }

                FlowNodeKind::BusinessRuleTask { decision_ref, decision_version } => {
                    crate::db::execution_history::record_entry(
                        tx,
                        instance_id,
                        execution.id,
                        &node.id,
                        element_type,
                        true,
                    )
                    .await?;

                    let org_id: Uuid =
                        sqlx::query_scalar("SELECT org_id FROM process_instances WHERE id = $1")
                            .bind(instance_id)
                            .fetch_one(&mut **tx)
                            .await?;

                    let var_map = crate::engine::helpers::load_instance_var_context(tx, instance_id).await?;

                    let def_opt = if let Some(pinned) = decision_version {
                        sqlx::query_as::<_, DecisionDefinition>(
                            "SELECT * FROM decision_definitions \
                             WHERE org_id = $1 AND decision_key = $2 AND version = $3",
                        )
                        .bind(org_id)
                        .bind(decision_ref)
                        .bind(*pinned)
                        .fetch_optional(&mut **tx)
                        .await?
                    } else {
                        sqlx::query_as::<_, DecisionDefinition>(
                            "SELECT * FROM decision_definitions \
                             WHERE org_id = $1 AND decision_key = $2 \
                             ORDER BY version DESC LIMIT 1",
                        )
                        .bind(org_id)
                        .bind(decision_ref)
                        .fetch_optional(&mut **tx)
                        .await?
                    };

                    let dmn_def = match def_opt {
                        Some(d) => d,
                        None => {
                            crate::db::process_events::record_error(
                                &mut **tx,
                                instance_id,
                                Some(execution.id),
                                Some(node.id.as_str()),
                                "error_raised",
                                None,
                                &match decision_version {
                                    Some(v) => format!("decision definition '{}' v{} not found", decision_ref, v),
                                    None => format!("decision definition '{}' not found", decision_ref),
                                },
                            )
                            .await?;
                            sqlx::query(
                                "UPDATE process_instances \
                                 SET state = 'error', ended_at = NOW() WHERE id = $1",
                            )
                            .bind(instance_id)
                            .execute(&mut **tx)
                            .await?;
                            continue;
                        }
                    };

                    let tables = match crate::dmn::parse(&dmn_def.dmn_xml) {
                        Ok(t) => t,
                        Err(e) => {
                            crate::db::process_events::record_error(
                                &mut **tx,
                                instance_id,
                                Some(execution.id),
                                Some(node.id.as_str()),
                                "error_raised",
                                None,
                                &format!("DMN parse error: {e}"),
                            )
                            .await?;
                            sqlx::query(
                                "UPDATE process_instances \
                                 SET state = 'error', ended_at = NOW() WHERE id = $1",
                            )
                            .bind(instance_id)
                            .execute(&mut **tx)
                            .await?;
                            continue;
                        }
                    };

                    let table = match tables.iter().find(|t| t.decision_key == *decision_ref) {
                        Some(t) => t,
                        None => {
                            crate::db::process_events::record_error(
                                &mut **tx,
                                instance_id,
                                Some(execution.id),
                                Some(node.id.as_str()),
                                "error_raised",
                                None,
                                &format!(
                                    "decision key '{}' not found in DMN definition",
                                    decision_ref
                                ),
                            )
                            .await?;
                            sqlx::query(
                                "UPDATE process_instances \
                                 SET state = 'error', ended_at = NOW() WHERE id = $1",
                            )
                            .bind(instance_id)
                            .execute(&mut **tx)
                            .await?;
                            continue;
                        }
                    };

                    let outputs = match crate::dmn::evaluate(table, &var_map) {
                        Ok(o) => o,
                        Err(e) => {
                            crate::db::process_events::record_error(
                                &mut **tx,
                                instance_id,
                                Some(execution.id),
                                Some(node.id.as_str()),
                                "error_raised",
                                None,
                                &format!("DMN evaluation error: {e}"),
                            )
                            .await?;
                            sqlx::query(
                                "UPDATE process_instances \
                                 SET state = 'error', ended_at = NOW() WHERE id = $1",
                            )
                            .bind(instance_id)
                            .execute(&mut **tx)
                            .await?;
                            continue;
                        }
                    };

                    debug!(instance_id = %instance_id, element_id = %node.id, decision_ref = %decision_ref, outputs = outputs.len(), "business rule task evaluated");
                    for (name, value) in &outputs {
                        let value_type = match value {
                            JsonValue::String(_) => "string",
                            JsonValue::Number(_) => "number",
                            JsonValue::Bool(_) => "boolean",
                            _ => "json",
                        };
                        crate::db::variables::upsert_in_tx(
                            &mut *tx,
                            instance_id,
                            execution.id,
                            Some(&node.id),
                            name,
                            value_type,
                            value,
                        )
                        .await?;
                    }

                    sqlx::query("UPDATE executions SET state = 'completed' WHERE id = $1")
                        .bind(execution.id)
                        .execute(&mut **tx)
                        .await?;

                    for next_id in current_graph
                        .outgoing
                        .get(&node.id)
                        .cloned()
                        .unwrap_or_default()
                    {
                        stack.push((next_id, scope));
                    }
                }

                FlowNodeKind::ScriptTask {
                    script,
                    result_variable,
                } => {
                    crate::db::execution_history::record_entry(
                        tx,
                        instance_id,
                        execution.id,
                        &node.id,
                        element_type,
                        true,
                    )
                    .await?;

                    let var_map = crate::engine::helpers::load_instance_var_context(tx, instance_id).await?;

                    let output =
                        match crate::engine::evaluator::evaluate_expression(script, &var_map) {
                            Ok(v) => v,
                            Err(e) => {
                                crate::db::process_events::record_error(
                                    &mut **tx,
                                    instance_id,
                                    Some(execution.id),
                                    Some(node.id.as_str()),
                                    "error_raised",
                                    None,
                                    &format!("script evaluation failed: {e}"),
                                )
                                .await?;
                                sqlx::query(
                                    "UPDATE process_instances \
                                     SET state = 'error', ended_at = NOW() WHERE id = $1",
                                )
                                .bind(instance_id)
                                .execute(&mut **tx)
                                .await?;
                                continue;
                            }
                        };

                    match output {
                        JsonValue::Object(map) => {
                            for (name, value) in &map {
                                let value_type = match value {
                                    JsonValue::String(_) => "string",
                                    JsonValue::Bool(_) => "boolean",
                                    _ => "json",
                                };
                                crate::db::variables::upsert_in_tx(
                                    &mut *tx,
                                    instance_id,
                                    execution.id,
                                    Some(&node.id),
                                    name,
                                    value_type,
                                    value,
                                )
                                .await?;
                            }
                        }
                        scalar => {
                            if let Some(var_name) = result_variable {
                                let value_type = match &scalar {
                                    JsonValue::String(_) => "string",
                                    JsonValue::Bool(_) => "boolean",
                                    _ => "json",
                                };
                                crate::db::variables::upsert_in_tx(
                                    &mut *tx,
                                    instance_id,
                                    execution.id,
                                    Some(&node.id),
                                    var_name,
                                    value_type,
                                    &scalar,
                                )
                                .await?;
                            } else {
                                crate::db::process_events::record_error(
                                    &mut **tx,
                                    instance_id,
                                    Some(execution.id),
                                    Some(node.id.as_str()),
                                    "error_raised",
                                    None,
                                    "script returned a non-object value but no resultVariable is configured",
                                )
                                .await?;
                                sqlx::query(
                                    "UPDATE process_instances \
                                     SET state = 'error', ended_at = NOW() WHERE id = $1",
                                )
                                .bind(instance_id)
                                .execute(&mut **tx)
                                .await?;
                                continue;
                            }
                        }
                    }

                    sqlx::query("UPDATE executions SET state = 'completed' WHERE id = $1")
                        .bind(execution.id)
                        .execute(&mut **tx)
                        .await?;

                    for next_id in current_graph
                        .outgoing
                        .get(&node.id)
                        .cloned()
                        .unwrap_or_default()
                    {
                        stack.push((next_id, scope));
                    }
                }

                FlowNodeKind::SignalStartEvent { .. } => {
                    // Entered when a SignalStartEvent triggers a new instance.
                    // Treat like a plain StartEvent — pass through immediately.
                    crate::db::execution_history::record_entry(
                        tx,
                        instance_id,
                        execution.id,
                        &node.id,
                        element_type,
                        true,
                    )
                    .await?;

                    sqlx::query("UPDATE executions SET state = 'completed' WHERE id = $1")
                        .bind(execution.id)
                        .execute(&mut **tx)
                        .await?;

                    for next_id in current_graph
                        .outgoing
                        .get(&node.id)
                        .cloned()
                        .unwrap_or_default()
                    {
                        stack.push((next_id, scope));
                    }
                }

                FlowNodeKind::TimerStartEvent { .. } => {
                    // Entered when a TimerStartEvent triggers a new instance.
                    // Treat like a plain StartEvent — pass through immediately.
                    crate::db::execution_history::record_entry(
                        tx,
                        instance_id,
                        execution.id,
                        &node.id,
                        element_type,
                        true,
                    )
                    .await?;

                    sqlx::query("UPDATE executions SET state = 'completed' WHERE id = $1")
                        .bind(execution.id)
                        .execute(&mut **tx)
                        .await?;

                    for next_id in current_graph
                        .outgoing
                        .get(&node.id)
                        .cloned()
                        .unwrap_or_default()
                    {
                        stack.push((next_id, scope));
                    }
                }

                FlowNodeKind::IntermediateSignalCatchEvent { signal_name } => {
                    crate::db::execution_history::record_entry(
                        tx,
                        instance_id,
                        execution.id,
                        &node.id,
                        element_type,
                        false,
                    )
                    .await?;

                    sqlx::query(
                        "INSERT INTO event_subscriptions \
                         (instance_id, execution_id, event_type, event_name, element_id) \
                         VALUES ($1, $2, 'signal', $3, $4)",
                    )
                    .bind(instance_id)
                    .bind(execution.id)
                    .bind(signal_name)
                    .bind(&node.id)
                    .execute(&mut **tx)
                    .await?;
                    debug!(instance_id = %instance_id, element_id = %node.id, signal_name = %signal_name, "signal catch event waiting for broadcast");
                    // Stop — waiting for signal broadcast.
                }

                FlowNodeKind::ExclusiveGateway { default_flow } => {
                    crate::db::execution_history::record_entry(
                        tx,
                        instance_id,
                        execution.id,
                        &node.id,
                        element_type,
                        true,
                    )
                    .await?;

                    sqlx::query("UPDATE executions SET state = 'completed' WHERE id = $1")
                        .bind(execution.id)
                        .execute(&mut **tx)
                        .await?;

                    // Scope: instance_id (not execution_id) so a gateway inside a
                    // subprocess sees variables written at the parent/instance level. BPMN
                    // visibility lets nested scopes read enclosing-scope variables.
                    let var_map = crate::engine::helpers::load_instance_var_context(tx, instance_id).await?;

                    let outgoing_flows: Vec<_> = current_graph
                        .flows
                        .iter()
                        .filter(|f| f.source_ref == node.id)
                        .collect();

                    let mut chosen: Option<String> = None;
                    let mut chosen_flow_id: Option<String> = None;
                    let mut default_target: Option<String> = None;
                    let mut eval_failure: Option<(String, String, String)> = None;

                    for flow in &outgoing_flows {
                        if default_flow.as_deref() == Some(flow.id.as_str()) {
                            default_target = Some(flow.target_ref.clone());
                            continue;
                        }
                        if let Some(expr) = &flow.condition {
                            match evaluator::evaluate_condition(expr, &var_map) {
                                Ok(true) => {
                                    chosen = Some(flow.target_ref.clone());
                                    chosen_flow_id = Some(flow.id.clone());
                                    break;
                                }
                                Ok(false) => continue,
                                Err(e) => {
                                    eval_failure =
                                        Some((flow.id.clone(), expr.clone(), e.to_string()));
                                    break;
                                }
                            }
                        }
                    }

                    if let Some((flow_id, expr, err)) = eval_failure {
                        tracing::error!(
                            instance_id = %instance_id,
                            element_id = %node.id,
                            flow_id = %flow_id,
                            condition = %expr,
                            error = %err,
                            "exclusive gateway condition evaluation failed; marking instance error"
                        );
                        crate::db::process_events::record_error(
                            &mut **tx,
                            instance_id,
                            Some(execution.id),
                            Some(node.id.as_str()),
                            "error_raised",
                            None,
                            &format!("exclusive gateway condition evaluation failed on flow '{flow_id}': {err}"),
                        )
                        .await?;
                        sqlx::query(
                            "UPDATE process_instances \
                             SET state = 'error', ended_at = NOW() WHERE id = $1",
                        )
                        .bind(instance_id)
                        .execute(&mut **tx)
                        .await?;
                        continue;
                    }

                    debug!(instance_id = %instance_id, element_id = %node.id, chosen_flow = chosen_flow_id.as_deref(), "exclusive gateway routing");
                    match chosen.or(default_target) {
                        Some(target) => stack.push((target, scope)),
                        None => {
                            crate::db::process_events::record_error(
                                &mut **tx,
                                instance_id,
                                Some(execution.id),
                                Some(node.id.as_str()),
                                "error_raised",
                                None,
                                "exclusive gateway: no matching condition and no default flow",
                            )
                            .await?;
                            sqlx::query(
                                "UPDATE process_instances \
                                 SET state = 'error', ended_at = NOW() WHERE id = $1",
                            )
                            .bind(instance_id)
                            .execute(&mut **tx)
                            .await?;
                        }
                    }
                }

                FlowNodeKind::ParallelGateway => {
                    let incoming_count = current_graph
                        .incoming
                        .get(&node.id)
                        .map(|v| v.len())
                        .unwrap_or(0);
                    let outgoing_ids: Vec<String> = current_graph
                        .outgoing
                        .get(&node.id)
                        .cloned()
                        .unwrap_or_default();

                    crate::db::execution_history::record_entry(
                        tx,
                        instance_id,
                        execution.id,
                        &node.id,
                        element_type,
                        true,
                    )
                    .await?;

                    sqlx::query("UPDATE executions SET state = 'completed' WHERE id = $1")
                        .bind(execution.id)
                        .execute(&mut **tx)
                        .await?;

                    if incoming_count <= 1 {
                        // Fork: create join state and push all branch starts.
                        let expected = outgoing_ids.len() as i32;
                        debug!(instance_id = %instance_id, element_id = %node.id, branches = expected, "parallel gateway fork");
                        sqlx::query(
                            "INSERT INTO parallel_join_state \
                             (instance_id, fork_execution_id, expected_count) \
                             VALUES ($1, $2, $3)",
                        )
                        .bind(instance_id)
                        .bind(execution.id)
                        .bind(expected)
                        .execute(&mut **tx)
                        .await?;

                        for target_id in outgoing_ids {
                            stack.push((target_id, Some(execution.id)));
                        }
                    } else {
                        // Join: atomically increment arrived count.
                        let fork_exec_id = scope.ok_or_else(|| {
                            EngineError::Internal(
                                "Parallel join reached with no scope — fork execution ID unknown"
                                    .to_string(),
                            )
                        })?;

                        let (arrived, expected): (i32, i32) = sqlx::query_as(
                            "UPDATE parallel_join_state \
                             SET arrived_count = arrived_count + 1 \
                             WHERE fork_execution_id = $1 \
                             RETURNING arrived_count, expected_count",
                        )
                        .bind(fork_exec_id)
                        .fetch_one(&mut **tx)
                        .await?;

                        debug!(instance_id = %instance_id, element_id = %node.id, arrived, expected, "parallel gateway join");
                        if arrived >= expected {
                            // All branches arrived — query the fork's parent scope for continuation.
                            let outer_scope: Option<Uuid> = sqlx::query_scalar(
                                "SELECT parent_id FROM executions WHERE id = $1",
                            )
                            .bind(fork_exec_id)
                            .fetch_one(&mut **tx)
                            .await?;

                            for target_id in outgoing_ids {
                                stack.push((target_id, outer_scope));
                            }
                        }
                        // else: not all branches arrived — this branch stops here.
                    }
                }

                FlowNodeKind::InclusiveGateway { default_flow } => {
                    let incoming_count = current_graph
                        .incoming
                        .get(&node.id)
                        .map(|v| v.len())
                        .unwrap_or(0);

                    crate::db::execution_history::record_entry(
                        tx,
                        instance_id,
                        execution.id,
                        &node.id,
                        element_type,
                        true,
                    )
                    .await?;

                    sqlx::query("UPDATE executions SET state = 'completed' WHERE id = $1")
                        .bind(execution.id)
                        .execute(&mut **tx)
                        .await?;

                    if incoming_count <= 1 {
                        // Fork: evaluate all conditions, activate every matching path.
                        let var_map = crate::engine::helpers::load_instance_var_context(tx, instance_id).await?;

                        let outgoing_flows: Vec<_> = current_graph
                            .flows
                            .iter()
                            .filter(|f| f.source_ref == node.id)
                            .collect();

                        let mut matched: Vec<String> = Vec::new();
                        let mut default_target: Option<String> = None;
                        let mut eval_failure: Option<(String, String, String)> = None;

                        for flow in &outgoing_flows {
                            if default_flow.as_deref() == Some(flow.id.as_str()) {
                                default_target = Some(flow.target_ref.clone());
                                continue;
                            }
                            if let Some(expr) = &flow.condition {
                                match evaluator::evaluate_condition(expr, &var_map) {
                                    Ok(true) => matched.push(flow.target_ref.clone()),
                                    Ok(false) => {}
                                    Err(e) => {
                                        eval_failure =
                                            Some((flow.id.clone(), expr.clone(), e.to_string()));
                                        break;
                                    }
                                }
                            } else {
                                // Unconditional outgoing (no condition expression) — always taken.
                                matched.push(flow.target_ref.clone());
                            }
                        }

                        if let Some((flow_id, expr, err)) = eval_failure {
                            tracing::error!(
                                instance_id = %instance_id,
                                element_id = %node.id,
                                flow_id = %flow_id,
                                condition = %expr,
                                error = %err,
                                "inclusive gateway condition evaluation failed; marking instance error"
                            );
                            crate::db::process_events::record_error(
                                &mut **tx,
                                instance_id,
                                Some(execution.id),
                                Some(node.id.as_str()),
                                "error_raised",
                                None,
                                &format!("inclusive gateway condition evaluation failed on flow '{flow_id}': {err}"),
                            )
                            .await?;
                            sqlx::query(
                                "UPDATE process_instances \
                                 SET state = 'error', ended_at = NOW() WHERE id = $1",
                            )
                            .bind(instance_id)
                            .execute(&mut **tx)
                            .await?;
                            continue;
                        }

                        if matched.is_empty() {
                            if let Some(target) = default_target {
                                matched.push(target);
                            } else {
                                crate::db::process_events::record_error(
                                    &mut **tx,
                                    instance_id,
                                    Some(execution.id),
                                    Some(node.id.as_str()),
                                    "error_raised",
                                    None,
                                    "inclusive gateway: no matching condition and no default flow",
                                )
                                .await?;
                                sqlx::query(
                                    "UPDATE process_instances \
                                     SET state = 'error', ended_at = NOW() WHERE id = $1",
                                )
                                .bind(instance_id)
                                .execute(&mut **tx)
                                .await?;
                                continue;
                            }
                        }

                        let expected = matched.len() as i32;
                        debug!(instance_id = %instance_id, element_id = %node.id, branches = expected, "inclusive gateway fork");
                        sqlx::query(
                            "INSERT INTO parallel_join_state \
                             (instance_id, fork_execution_id, expected_count) \
                             VALUES ($1, $2, $3)",
                        )
                        .bind(instance_id)
                        .bind(execution.id)
                        .bind(expected)
                        .execute(&mut **tx)
                        .await?;

                        for target_id in matched {
                            stack.push((target_id, Some(execution.id)));
                        }
                    } else {
                        // Join: identical to ParallelGateway join.
                        let fork_exec_id = scope.ok_or_else(|| {
                            EngineError::Internal(
                                "Inclusive join reached with no scope — fork execution ID unknown"
                                    .to_string(),
                            )
                        })?;

                        let (arrived, expected): (i32, i32) = sqlx::query_as(
                            "UPDATE parallel_join_state \
                             SET arrived_count = arrived_count + 1 \
                             WHERE fork_execution_id = $1 \
                             RETURNING arrived_count, expected_count",
                        )
                        .bind(fork_exec_id)
                        .fetch_one(&mut **tx)
                        .await?;

                        debug!(instance_id = %instance_id, element_id = %node.id, arrived, expected, "inclusive gateway join");
                        if arrived >= expected {
                            let outer_scope: Option<Uuid> = sqlx::query_scalar(
                                "SELECT parent_id FROM executions WHERE id = $1",
                            )
                            .bind(fork_exec_id)
                            .fetch_one(&mut **tx)
                            .await?;

                            let outgoing_ids: Vec<String> = current_graph
                                .outgoing
                                .get(&node.id)
                                .cloned()
                                .unwrap_or_default();

                            for target_id in outgoing_ids {
                                stack.push((target_id, outer_scope));
                            }
                        }
                        // else: not all activated branches arrived — this branch stops here.
                    }
                }
            }
        }

        Ok(())
    }
}
