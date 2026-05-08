//! Phase 16 — HTTP service-task connector.
//!
//! Lifecycle of a single fire:
//!   1. Load the job and its `config` JSONB (or fall back to legacy URL-in-`topic`).
//!   2. Pull current instance variables.
//!   3. Resolve the bound secret (if any) by name, at this exact moment, so
//!      rotation works without redeploying definitions.
//!   4. Build an input doc and run the request transform (jq) → request shape.
//!   5. Construct a `reqwest::RequestBuilder` honoring method/timeout/headers/
//!      query/path. Auth headers are layered last so a malicious or buggy
//!      transform cannot overwrite them.
//!   6. Send the request. Classify failures against the retry policy: retryable
//!      → bump `retry_count`, push `due_date`, leave the job pending.
//!   7. Build a response doc and run the response transform → flat var map →
//!      upsert into instance variables.
//!   8. Mark job + execution complete and advance tokens past the service task.

use std::collections::BTreeMap;
use std::time::Duration;

use serde_json::{Map, Value as JsonValue};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::db::models::{EventSubscription, Execution};
use crate::error::{EngineError, Result};
use crate::parser::types::{HttpAuth, HttpConfig, RetryPolicy};
use crate::parser::FlowNodeKind;

use super::{Engine, VariableInput};

impl Engine {
    pub async fn fire_http_task(&self, job_id: Uuid) -> Result<()> {
        debug!(job_id = %job_id, "firing HTTP task");
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

        // The retry policy applies even to pre-send failures (e.g. transform
        // errors). We hold a default policy to fall back on if `build_request_plan`
        // itself fails — that path is non-retryable since the job's snapshot is
        // unchanged across retries.
        let pre_send: std::result::Result<(RequestPlan, JsonValue, Option<String>), SendError> =
            self.prepare_send(&job).await;
        let (plan, request_shape, secret_value) = match pre_send {
            Ok(triple) => triple,
            Err(err) => {
                return self
                    .handle_failure(&job, &RetryPolicy::default(), err)
                    .await;
            }
        };

        // ---- Send loop with retry classification --------------------------------
        let send_result = self
            .send_with_request_shape(&plan, &request_shape, secret_value.as_deref())
            .await;
        let (status, response_body, response_headers) = match send_result {
            Ok(triple) => triple,
            Err(send_err) => {
                return self
                    .handle_failure(&job, &plan.config.retry, send_err)
                    .await;
            }
        };

        // Build response doc once — used by errorCodeExpression, responseTransform, and audit.
        let response_doc = serde_json::json!({
            "status": status,
            "headers": response_headers,
            "body": response_body,
        });

        // ---- errorCodeExpression: checked before success/failure routing --------
        // A non-empty string result overrides HTTP status and routes the token to
        // the matching BoundaryErrorEvent instead of the normal completion path.
        if let Some(expr) = plan.config.error_code_expression.as_deref() {
            match self.jq_cache.run(expr, response_doc.clone()) {
                Ok(JsonValue::String(code)) if !code.is_empty() => {
                    return self.route_http_bpmn_error(&job, &code, response_doc).await;
                }
                Ok(_) => {}
                Err(e) => {
                    return self
                        .handle_failure(&job, &RetryPolicy::default(), SendError::Internal(e))
                        .await;
                }
            }
        }

        if !is_success(status) {
            let class = if (400..500).contains(&status) {
                FailureClass::Status4xx
            } else {
                FailureClass::Status5xx
            };
            return self
                .handle_failure(
                    &job,
                    &plan.config.retry,
                    SendError::Status {
                        class,
                        status,
                        body: response_doc["body"].to_string(),
                    },
                )
                .await;
        }

        // ---- Success path -------------------------------------------------------
        // Any failure here (response transform, var write, token advancement)
        // is a permanent error — not retryable, since the upstream already
        // accepted the request. Route through handle_failure so the job state
        // doesn't end up stranded in 'locked'.
        if let Err(post_err) = self.complete_http_task(&job, &plan, response_doc).await {
            return self
                .handle_failure(&job, &RetryPolicy::default(), SendError::Internal(post_err))
                .await;
        }
        Ok(())
    }

    async fn complete_http_task(
        &self,
        job: &crate::db::models::Job,
        plan: &RequestPlan,
        response_doc: JsonValue,
    ) -> Result<()> {
        let output_vars = match plan.config.response_transform.as_deref() {
            Some(filter) => {
                let shaped = self.jq_cache.run(filter, response_doc.clone())?;
                flatten_response_to_vars(&shaped)
            }
            None => Self::parse_http_response_variables(&response_doc["body"]),
        };

        let status = response_doc["status"].as_u64().unwrap_or(0);
        info!(
            job_id = %job.id,
            instance_id = %job.instance_id,
            status,
            output_count = output_vars.len(),
            "HTTP service task completed"
        );

        // Advance the token: write vars, mark complete, walk forward.
        let def_row: (Uuid,) =
            sqlx::query_as("SELECT definition_id FROM process_instances WHERE id = $1")
                .bind(job.instance_id)
                .fetch_one(&self.pool)
                .await?;
        let graph = self.load_graph(def_row.0).await?;

        let http_exec: Execution = sqlx::query_as("SELECT * FROM executions WHERE id = $1")
            .bind(job.execution_id)
            .fetch_one(&self.pool)
            .await?;
        let element_id = http_exec.element_id.clone();
        let parallel_scope = http_exec.parent_id;

        let (current_graph, _) =
            Self::find_element_graph(&element_id, &graph).ok_or_else(|| {
                EngineError::Internal(format!("Element '{element_id}' not found in process graph"))
            })?;

        let mut tx = self.pool.begin().await?;

        for var in &output_vars {
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
        .bind(job.id)
        .execute(&mut *tx)
        .await?;
        crate::db::jobs::record_state_change(
            &mut tx,
            job.id,
            "job_completed",
            serde_json::json!({}),
        )
        .await?;

        // Cancel any boundary event executions and subscriptions for this task.
        // They are no longer needed once the host task completes normally.
        if let Some(boundaries) = current_graph.attached_to.get(&element_id).cloned() {
            for bid in &boundaries {
                let cancelled: Vec<Uuid> = sqlx::query_scalar(
                    "UPDATE executions SET state = 'cancelled' \
                     WHERE instance_id = $1 AND element_id = $2 AND state = 'active' \
                     RETURNING id",
                )
                .bind(job.instance_id)
                .bind(bid)
                .fetch_all(&mut *tx)
                .await?;
                if !cancelled.is_empty() {
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

    /// Route the HTTP task to the best-matching `BoundaryErrorEvent` when
    /// `errorCodeExpression` returns a non-empty error code. Mirrors the logic
    /// in `throw_bpmn_error` from the external-task path.
    async fn route_http_bpmn_error(
        &self,
        job: &crate::db::models::Job,
        error_code: &str,
        _response_doc: JsonValue,
    ) -> Result<()> {
        let def_row: (Uuid,) =
            sqlx::query_as("SELECT definition_id FROM process_instances WHERE id = $1")
                .bind(job.instance_id)
                .fetch_one(&self.pool)
                .await?;
        let graph = self.load_graph(def_row.0).await?;

        let mut tx = self.pool.begin().await?;

        crate::db::process_events::record_error(
            &mut *tx,
            job.instance_id,
            Some(job.execution_id),
            None,
            "error_raised",
            Some(error_code),
            &format!("HTTP connector errorCodeExpression matched '{error_code}'"),
        )
        .await?;

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

        let Some(sub) = sub else {
            warn!(
                job_id = %job.id,
                error_code,
                "no matching BoundaryErrorEvent for HTTP connector; terminating instance"
            );
            sqlx::query(
                "UPDATE jobs SET state = 'failed', \
                 error_message = $1, locked_by = NULL, locked_until = NULL WHERE id = $2",
            )
            .bind(format!(
                "BPMN error '{error_code}' has no matching boundary event"
            ))
            .bind(job.id)
            .execute(&mut *tx)
            .await?;
            crate::db::jobs::record_state_change(
                &mut tx,
                job.id,
                "job_failed",
                serde_json::json!({"error_code": error_code}),
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
        };

        crate::db::process_events::record_error(
            &mut *tx,
            job.instance_id,
            Some(sub.execution_id),
            Some(sub.element_id.as_str()),
            "error_caught",
            Some(error_code),
            &format!("HTTP connector errorCodeExpression matched '{error_code}'"),
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

        if cancelling {
            sqlx::query(
                "UPDATE jobs SET state = 'cancelled', locked_by = NULL, locked_until = NULL \
                 WHERE id = $1",
            )
            .bind(job.id)
            .execute(&mut *tx)
            .await?;
            crate::db::jobs::record_state_change(
                &mut tx,
                job.id,
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

            if let Some(all_boundaries) =
                current_graph.attached_to.get(attached_to.as_str()).cloned()
            {
                for bid in &all_boundaries {
                    if *bid != sub.element_id {
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
        info!(
            job_id = %job.id,
            boundary_id = %sub.element_id,
            error_code,
            "HTTP connector BPMN error routed to boundary event"
        );
        Ok(())
    }

    /// Read job config, instance/org, current variables, run the request
    /// transform, and resolve the secret. Any failure here is wrapped in
    /// [`SendError::Internal`] so it flows through `handle_failure` and the
    /// job state is updated rather than left in `locked`.
    async fn prepare_send(
        &self,
        job: &crate::db::models::Job,
    ) -> std::result::Result<(RequestPlan, JsonValue, Option<String>), SendError> {
        let plan = build_request_plan(job).map_err(SendError::Internal)?;

        let row: (Uuid, Uuid) = sqlx::query_as(
            "SELECT pi.id, COALESCE(pd.org_id, '00000000-0000-0000-0000-000000000000'::uuid) \
             FROM process_instances pi \
             JOIN process_definitions pd ON pd.id = pi.definition_id \
             WHERE pi.id = $1",
        )
        .bind(job.instance_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| SendError::Internal(EngineError::Database(e)))?;
        let org_id = row.1;

        let vars: Vec<crate::db::models::Variable> =
            sqlx::query_as("SELECT * FROM variables WHERE instance_id = $1")
                .bind(job.instance_id)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| SendError::Internal(EngineError::Database(e)))?;
        let mut vars_map = Map::new();
        for v in &vars {
            vars_map.insert(v.name.clone(), v.value.clone());
        }

        let request_input = serde_json::json!({
            "instance_id": job.instance_id.to_string(),
            "execution_id": job.execution_id.to_string(),
            "vars": vars_map,
        });
        let request_shape = match plan.config.request_transform.as_deref() {
            Some(filter) => self
                .jq_cache
                .run(filter, request_input)
                .map_err(SendError::Internal)?,
            None => legacy_request_envelope(job, &vars_map),
        };

        let secret_value = match (&plan.config.auth, plan.config.secret_ref.as_deref()) {
            (HttpAuth::None, _) => None,
            (_, None) => {
                return Err(SendError::Internal(EngineError::Validation(
                    "HTTP task auth requires a secretRef but none was set".into(),
                )));
            }
            (_, Some(name)) => Some(
                crate::db::secrets::reveal(&self.pool, &self.secrets_key, org_id, name)
                    .await
                    .map_err(SendError::Internal)?,
            ),
        };

        Ok((plan, request_shape, secret_value))
    }

    async fn send_with_request_shape(
        &self,
        plan: &RequestPlan,
        request_shape: &JsonValue,
        secret_value: Option<&str>,
    ) -> std::result::Result<(u16, JsonValue, JsonValue), SendError> {
        let mut url = plan.url.clone();
        let shape_obj = request_shape.as_object();

        // path substitutions: replace `:name` placeholders before query params attach.
        if let Some(path_map) = shape_obj
            .and_then(|o| o.get("path"))
            .and_then(|p| p.as_object())
        {
            for (k, v) in path_map {
                let needle = format!(":{k}");
                let val = json_value_as_string(v);
                url = url.replace(&needle, &urlencoding::encode(&val));
            }
        }

        let method = parse_method(&plan.config.method).map_err(SendError::Internal)?;
        let mut req = self.http_client.request(method.clone(), &url);

        if let Some(t) = plan.config.timeout_ms {
            req = req.timeout(Duration::from_millis(t));
        }

        // query params — built manually to keep reqwest's feature surface
        // narrow (we don't pull in serde_urlencoded).
        if let Some(q) = shape_obj
            .and_then(|o| o.get("query"))
            .and_then(|q| q.as_object())
        {
            if !q.is_empty() {
                let mut qs = String::new();
                for (k, v) in q {
                    if !qs.is_empty() {
                        qs.push('&');
                    }
                    qs.push_str(&urlencoding::encode(k));
                    qs.push('=');
                    qs.push_str(&urlencoding::encode(&json_value_as_string(v)));
                }
                let sep = if url.contains('?') { '&' } else { '?' };
                url.push(sep);
                url.push_str(&qs);
                req = self.http_client.request(method.clone(), &url);
                if let Some(t) = plan.config.timeout_ms {
                    req = req.timeout(Duration::from_millis(t));
                }
            }
        }

        // Merge user transform headers + auth headers into a single HeaderMap.
        // We use `insert` (not `append`) so auth wins outright on conflicts —
        // a malicious or buggy transform that sets `Authorization` cannot leak
        // its value alongside the real token.
        use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
        let mut headers: HeaderMap = HeaderMap::new();
        if let Some(h) = shape_obj
            .and_then(|o| o.get("headers"))
            .and_then(|h| h.as_object())
        {
            for (k, v) in h {
                let Ok(name) = k.parse::<HeaderName>() else {
                    continue;
                };
                let Ok(value) = json_value_as_string(v).parse::<HeaderValue>() else {
                    continue;
                };
                headers.insert(name, value);
            }
        }
        match (&plan.config.auth, secret_value) {
            (HttpAuth::None, _) => {}
            (HttpAuth::Bearer, Some(token)) => {
                let v: HeaderValue = format!("Bearer {token}").parse().map_err(|e| {
                    SendError::Internal(EngineError::Internal(format!(
                        "bearer header value rejected: {e}"
                    )))
                })?;
                headers.insert(reqwest::header::AUTHORIZATION, v);
            }
            (HttpAuth::Basic, Some(creds)) => {
                use base64::{engine::general_purpose::STANDARD, Engine as _};
                let v: HeaderValue = format!("Basic {}", STANDARD.encode(creds.as_bytes()))
                    .parse()
                    .map_err(|e| {
                        SendError::Internal(EngineError::Internal(format!(
                            "basic header value rejected: {e}"
                        )))
                    })?;
                headers.insert(reqwest::header::AUTHORIZATION, v);
            }
            (HttpAuth::ApiKey, Some(token)) => {
                let header = plan.config.api_key_header.as_deref().unwrap_or("X-API-Key");
                let name: HeaderName = header.parse().map_err(|e| {
                    SendError::Internal(EngineError::Validation(format!(
                        "invalid apiKey headerName '{header}': {e}"
                    )))
                })?;
                let v: HeaderValue = token.parse().map_err(|e| {
                    SendError::Internal(EngineError::Internal(format!(
                        "apiKey value rejected: {e}"
                    )))
                })?;
                headers.insert(name, v);
            }
            (_, None) => unreachable!("secret_value None handled above"),
        }
        req = req.headers(headers);

        // body — only for methods that send one. We attach as JSON when present.
        if let Some(body) = shape_obj.and_then(|o| o.get("body")) {
            if !matches!(plan.config.method.as_str(), "GET" | "HEAD" | "DELETE") && !body.is_null()
            {
                req = req.json(body);
            }
        }

        let resp = req.send().await.map_err(|e| {
            if e.is_timeout() {
                SendError::Timeout(e.to_string())
            } else {
                SendError::Network(e.to_string())
            }
        })?;
        let status = resp.status().as_u16();
        let headers_json = headers_to_lowercased_json(resp.headers());
        let body_text = resp.text().await.unwrap_or_default();
        // Try to parse as JSON; fall back to a plain string body so the response
        // doc shape is stable for jq filters either way.
        let body_json =
            serde_json::from_str::<JsonValue>(&body_text).unwrap_or(JsonValue::String(body_text));
        Ok((status, body_json, headers_json))
    }

    /// Apply the retry policy to a failed send. Marks the job either pending-with-
    /// future-due-date (retryable) or terminal-failed.
    async fn handle_failure(
        &self,
        job: &crate::db::models::Job,
        retry: &RetryPolicy,
        err: SendError,
    ) -> Result<()> {
        let class = err.class();
        let msg = err.into_message();

        let retryable = is_retryable(class, retry);
        let attempt_no = job.retry_count;
        let next_count = attempt_no + 1;

        if retryable && next_count <= retry.max as i32 {
            let backoff_ms =
                (retry.backoff_ms as f64 * retry.multiplier.powi(attempt_no.max(0))).round() as i64;
            let backoff_ms = backoff_ms.clamp(retry.backoff_ms as i64, 5 * 60 * 1000);
            sqlx::query(
                "UPDATE jobs SET \
                   state = 'pending', \
                   locked_by = NULL, \
                   locked_until = NULL, \
                   retry_count = $2, \
                   due_date = NOW() + ($3 || ' milliseconds')::interval, \
                   error_message = $4 \
                 WHERE id = $1",
            )
            .bind(job.id)
            .bind(next_count)
            .bind(backoff_ms.to_string())
            .bind(&msg)
            .execute(&self.pool)
            .await?;
            crate::db::jobs::record_state_change(
                &mut self.pool.begin().await?,
                job.id,
                "job_retry_scheduled",
                serde_json::json!({ "attempt": next_count, "delay_ms": backoff_ms, "error": msg }),
            )
            .await
            .ok();
            warn!(job_id = %job.id, attempt = next_count, backoff_ms, %msg, "HTTP task scheduled for retry");
            Ok(())
        } else {
            sqlx::query(
                "UPDATE jobs SET state = 'failed', locked_by = NULL, locked_until = NULL, \
                                  retry_count = $2, error_message = $3 \
                 WHERE id = $1",
            )
            .bind(job.id)
            .bind(next_count)
            .bind(&msg)
            .execute(&self.pool)
            .await?;
            crate::db::jobs::record_state_change(
                &mut self.pool.begin().await?,
                job.id,
                "job_failed",
                serde_json::json!({ "error": msg }),
            )
            .await
            .ok();
            warn!(job_id = %job.id, %msg, "HTTP task terminally failed");
            Err(EngineError::Internal(format!("HTTP task failed: {msg}")))
        }
    }

    fn parse_http_response_variables(body: &serde_json::Value) -> Vec<VariableInput> {
        let obj = match body.as_object() {
            Some(o) => o,
            None => return vec![],
        };

        // Conduit-native format: { "variables": [{ "name", "value_type", "value" }] }
        if let Some(arr) = obj.get("variables").and_then(|v| v.as_array()) {
            return arr
                .iter()
                .filter_map(|v| {
                    let name = v.get("name")?.as_str()?.to_string();
                    let value_type = v.get("value_type")?.as_str()?.to_string();
                    let value = v.get("value")?.clone();
                    Some(VariableInput {
                        name,
                        value_type,
                        value,
                    })
                })
                .collect();
        }

        // Flat format: every top-level key becomes a variable; type inferred from JSON type.
        obj.iter()
            .filter(|(k, _)| k.as_str() != "instance_id" && k.as_str() != "execution_id")
            .map(|(k, v)| VariableInput {
                name: k.clone(),
                value_type: infer_var_type(v).to_string(),
                value: v.clone(),
            })
            .collect()
    }

    pub async fn fire_due_http_tasks(&self) -> Result<usize> {
        let jobs = crate::db::jobs::fetch_and_lock_many(
            &self.pool,
            "conduit-http-executor",
            30,
            None,
            Some("http_task"),
            20,
        )
        .await?;

        let mut count = 0;
        for job in jobs {
            match self.fire_http_task(job.id).await {
                Ok(_) => count += 1,
                Err(crate::error::EngineError::Conflict(_)) => {}
                // Internal errors here are already persisted to the job row;
                // continuing keeps the executor healthy across batches.
                Err(crate::error::EngineError::Internal(_)) => {}
                Err(e) => return Err(e),
            }
        }
        if count > 0 {
            info!(count, "fired due HTTP tasks");
        }
        Ok(count)
    }
}

// --- helpers (free functions; no Engine state) ---------------------------------

struct RequestPlan {
    url: String,
    config: HttpConfig,
}

/// Build a [`RequestPlan`] from a job row. Prefers the modern `config` JSONB.
/// Falls back to the legacy "URL-in-topic, no extension elements" shape so jobs
/// enqueued before C2 still fire correctly during a rolling deploy.
fn build_request_plan(job: &crate::db::models::Job) -> Result<RequestPlan> {
    if let Some(cfg_value) = &job.config {
        let url = cfg_value
            .get("url")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| job.topic.clone())
            .ok_or_else(|| EngineError::Internal(format!("HTTP task job {} has no URL", job.id)))?;
        let config: HttpConfig = serde_json::from_value(cfg_value.clone()).map_err(|e| {
            EngineError::Internal(format!(
                "Job {} has invalid HttpConfig in jobs.config: {e}",
                job.id
            ))
        })?;
        Ok(RequestPlan { url, config })
    } else {
        let url = job
            .topic
            .clone()
            .ok_or_else(|| EngineError::Internal(format!("HTTP task job {} has no URL", job.id)))?;
        Ok(RequestPlan {
            url,
            config: HttpConfig {
                method: "POST".into(),
                timeout_ms: None,
                auth: HttpAuth::None,
                secret_ref: None,
                api_key_header: None,
                request_transform: None,
                response_transform: None,
                error_code_expression: None,
                retry: RetryPolicy::default(),
            },
        })
    }
}

/// Recreate the pre-Phase-16 envelope `{instance_id, execution_id, variables}`
/// as a body-only request shape. Used when no request transform is configured.
fn legacy_request_envelope(
    job: &crate::db::models::Job,
    vars_map: &serde_json::Map<String, JsonValue>,
) -> JsonValue {
    serde_json::json!({
        "body": {
            "instance_id": job.instance_id.to_string(),
            "execution_id": job.execution_id.to_string(),
            "variables": vars_map,
        }
    })
}

fn parse_method(s: &str) -> Result<reqwest::Method> {
    s.parse::<reqwest::Method>()
        .map_err(|e| EngineError::Validation(format!("invalid HTTP method '{s}': {e}")))
}

fn json_value_as_string(v: &JsonValue) -> String {
    match v {
        JsonValue::String(s) => s.clone(),
        JsonValue::Null => String::new(),
        _ => v.to_string(),
    }
}

fn headers_to_lowercased_json(headers: &reqwest::header::HeaderMap) -> JsonValue {
    let mut m: BTreeMap<String, String> = BTreeMap::new();
    for (name, value) in headers.iter() {
        if let Ok(s) = value.to_str() {
            // Stable keys for jq filters: lowercased header names.
            m.insert(name.as_str().to_ascii_lowercase(), s.to_string());
        }
    }
    serde_json::to_value(m).unwrap_or(JsonValue::Object(serde_json::Map::new()))
}

fn flatten_response_to_vars(shaped: &JsonValue) -> Vec<VariableInput> {
    let obj = match shaped.as_object() {
        Some(o) => o,
        None => return vec![],
    };
    obj.iter()
        // jq filters that resolve to `null` mean "do not set this variable".
        .filter(|(_, v)| !v.is_null())
        .map(|(k, v)| VariableInput {
            name: k.clone(),
            value_type: infer_var_type(v).to_string(),
            value: v.clone(),
        })
        .collect()
}

fn infer_var_type(v: &JsonValue) -> &'static str {
    // Must match the CHECK constraint on `variables.value_type`:
    // ('string', 'integer', 'boolean', 'json'). Floats / arrays / objects fall
    // through to 'json' rather than introducing a new variant.
    match v {
        JsonValue::String(_) => "string",
        JsonValue::Number(n) if n.is_i64() || n.is_u64() => "integer",
        JsonValue::Bool(_) => "boolean",
        _ => "json",
    }
}

fn is_success(status: u16) -> bool {
    (200..300).contains(&status)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FailureClass {
    Network,
    Timeout,
    Status4xx,
    Status5xx,
}

#[derive(Debug)]
enum SendError {
    Network(String),
    Timeout(String),
    Status {
        class: FailureClass,
        status: u16,
        body: String,
    },
    Internal(EngineError),
}

impl SendError {
    fn class(&self) -> FailureClass {
        match self {
            SendError::Network(_) => FailureClass::Network,
            SendError::Timeout(_) => FailureClass::Timeout,
            SendError::Status { class, .. } => *class,
            // Internal errors are not retryable by the policy — they're a code
            // bug, not a transient remote failure.
            SendError::Internal(_) => FailureClass::Status4xx,
        }
    }
    fn into_message(self) -> String {
        match self {
            SendError::Network(s) => format!("network error: {s}"),
            SendError::Timeout(s) => format!("timeout: {s}"),
            SendError::Status { status, body, .. } => format!(
                "HTTP {status}: {}",
                body.chars().take(500).collect::<String>()
            ),
            SendError::Internal(e) => format!("internal: {e}"),
        }
    }
}

fn is_retryable(class: FailureClass, retry: &RetryPolicy) -> bool {
    if retry.max == 0 {
        return false;
    }
    if retry.retry_on.is_empty() {
        // Safe defaults: only transient failures, not 4xx (which is "client bug").
        return matches!(
            class,
            FailureClass::Network | FailureClass::Timeout | FailureClass::Status5xx
        );
    }
    retry.retry_on.iter().any(|cond| match cond.as_str() {
        "network" => class == FailureClass::Network,
        "timeout" => class == FailureClass::Timeout,
        "4xx" => class == FailureClass::Status4xx,
        "5xx" => class == FailureClass::Status5xx,
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn flatten_drops_null_variables() {
        let shaped = json!({ "a": 1, "b": null, "c": "x" });
        let vars = flatten_response_to_vars(&shaped);
        let names: Vec<&str> = vars.iter().map(|v| v.name.as_str()).collect();
        assert_eq!(names, vec!["a", "c"]);
    }

    #[test]
    fn retry_policy_max_zero_never_retries() {
        let policy = RetryPolicy::default();
        assert!(!is_retryable(FailureClass::Status5xx, &policy));
    }

    #[test]
    fn retry_policy_default_excludes_4xx() {
        let policy = RetryPolicy {
            max: 3,
            ..RetryPolicy::default()
        };
        assert!(is_retryable(FailureClass::Status5xx, &policy));
        assert!(is_retryable(FailureClass::Timeout, &policy));
        assert!(is_retryable(FailureClass::Network, &policy));
        assert!(!is_retryable(FailureClass::Status4xx, &policy));
    }

    #[test]
    fn retry_on_explicitly_includes_4xx_when_configured() {
        let policy = RetryPolicy {
            max: 3,
            retry_on: vec!["4xx".into()],
            ..RetryPolicy::default()
        };
        assert!(is_retryable(FailureClass::Status4xx, &policy));
        assert!(!is_retryable(FailureClass::Status5xx, &policy));
    }

    #[test]
    fn parse_method_supports_all_standard_verbs() {
        for v in ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD"] {
            assert!(parse_method(v).is_ok(), "method {v} should parse");
        }
        assert!(parse_method("BOGUS@").is_err());
    }

    #[test]
    fn header_lowercasing_is_stable() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("X-Custom", "Value".parse().unwrap());
        let v = headers_to_lowercased_json(&headers);
        assert_eq!(v["x-custom"], "Value");
        assert!(v.get("X-Custom").is_none());
    }
}
