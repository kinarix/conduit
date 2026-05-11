//! Phase 16 — HTTP service-task connector integration tests.
//!
//! Each test stands up:
//!   - a Postgres-backed test app (via spawn_test_app)
//!   - a wiremock server with declared expectations
//!   - an in-process Engine (the HTTP executor loop is not spawned in tests,
//!     so we drive it manually via `fire_job_for_instance()` to avoid
//!     races between parallel tests that share the same DB.
//!
//! The BPMN deployed in each test embeds the wiremock URL plus a
//! `<conduit:http>` extension element configured for the scenario under test.

mod common;

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use conduit::engine::Engine;
use conduit::state::GraphCache;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;
use wiremock::matchers::{header, method as m_method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TEST_KEY: [u8; 32] = [0xA5u8; 32];

fn engine_for(pool: PgPool) -> Engine {
    let cache: GraphCache = Arc::new(RwLock::new(HashMap::new()));
    Engine::new(pool, cache, TEST_KEY)
}

/// Build a BPMN document whose only service task targets `mock_url`. The
/// `<conduit:http>` block (already including the closing tag) is interpolated
/// verbatim into the extensionElements so tests control method/auth/transforms.
fn http_bpmn(mock_url: &str, http_block: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL"
             xmlns:conduit="http://conduit.io/ext"
             targetNamespace="http://conduit.io/test">
  <process id="http_test_proc" isExecutable="true">
    <startEvent id="start"/>
    <serviceTask id="call" name="Call" url="{mock_url}">
      <extensionElements>
{http_block}
      </extensionElements>
    </serviceTask>
    <endEvent id="end"/>
    <sequenceFlow id="f1" sourceRef="start" targetRef="call"/>
    <sequenceFlow id="f2" sourceRef="call" targetRef="end"/>
  </process>
</definitions>"#
    )
}

fn unique_key(prefix: &str) -> String {
    format!("{}-{}", prefix, Uuid::new_v4())
}

/// Convert a flat `{name: value}` JSON object into the engine's variable
/// input shape (an array of `{name, value_type, value}` records).
fn vars_from_object(obj: serde_json::Value) -> serde_json::Value {
    let map = obj.as_object().expect("variables must be a JSON object");
    let arr: Vec<serde_json::Value> = map
        .iter()
        .map(|(k, v)| {
            let value_type = match v {
                serde_json::Value::String(_) => "string",
                serde_json::Value::Number(n) if n.is_i64() || n.is_u64() => "integer",
                serde_json::Value::Bool(_) => "boolean",
                _ => "json",
            };
            json!({ "name": k, "value_type": value_type, "value": v })
        })
        .collect();
    serde_json::Value::Array(arr)
}

async fn deploy_and_start(
    app: &common::TestApp,
    org_id: Uuid,
    process_group_id: Uuid,
    bpmn: &str,
    initial_vars: serde_json::Value,
) -> Uuid {
    let client = app.client.clone();
    let deploy_resp = client
        .post(format!(
            "{}/api/v1/orgs/{}/deployments",
            app.address, org_id
        ))
        .json(&json!({
            "process_group_id": process_group_id,
            "key": unique_key("http"),
            "bpmn_xml": bpmn,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        deploy_resp.status(),
        201,
        "deploy failed: {}",
        deploy_resp.text().await.unwrap_or_default()
    );
    let def: serde_json::Value = deploy_resp.json().await.unwrap();
    let def_id = Uuid::parse_str(def["id"].as_str().unwrap()).unwrap();

    let mut body = json!({
        "definition_id": def_id,
    });
    if !initial_vars.as_object().is_none_or(|o| o.is_empty()) {
        body["variables"] = vars_from_object(initial_vars);
    }
    let start_resp = client
        .post(format!(
            "{}/api/v1/orgs/{}/process-instances",
            app.address, org_id
        ))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(
        start_resp.status(),
        201,
        "start failed: {}",
        start_resp.text().await.unwrap_or_default()
    );
    let inst: serde_json::Value = start_resp.json().await.unwrap();
    Uuid::parse_str(inst["id"].as_str().unwrap()).unwrap()
}

async fn create_secret(app: &common::TestApp, org_id: Uuid, name: &str, value: &str) {
    let client = app.client.clone();
    let resp = client
        .post(format!("{}/api/v1/orgs/{}/secrets", app.address, org_id))
        .json(&json!({ "name": name, "value": value }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201, "secret create failed");
}

async fn read_var(pool: &PgPool, instance_id: Uuid, name: &str) -> Option<serde_json::Value> {
    sqlx::query_scalar::<_, Option<serde_json::Value>>(
        "SELECT value FROM variables WHERE instance_id = $1 AND name = $2",
    )
    .bind(instance_id)
    .bind(name)
    .fetch_optional(pool)
    .await
    .unwrap()
    .flatten()
}

async fn fetch_job_for(pool: &PgPool, instance_id: Uuid) -> Option<conduit::db::models::Job> {
    sqlx::query_as("SELECT * FROM jobs WHERE instance_id = $1 ORDER BY created_at DESC LIMIT 1")
        .bind(instance_id)
        .fetch_optional(pool)
        .await
        .unwrap()
}

/// Lock a specific job atomically and fire it. Replaces the global
/// `fire_due_http_tasks` for tests so concurrent tests sharing the DB don't
/// race for each other's jobs.
async fn fire_job_for_instance(engine: &Engine, pool: &PgPool, instance_id: Uuid) {
    // Bypass `fetch_and_lock_many` (which is global) and lock just this job.
    let job_id: Uuid = sqlx::query_scalar(
        "UPDATE jobs SET state = 'locked', \
                          locked_by = 'test', \
                          locked_until = NOW() + interval '60 seconds' \
         WHERE id = (SELECT id FROM jobs \
                     WHERE instance_id = $1 AND job_type = 'http_task' AND state = 'pending' \
                     ORDER BY created_at DESC LIMIT 1) \
         RETURNING id",
    )
    .bind(instance_id)
    .fetch_one(pool)
    .await
    .expect("no http_task job found for instance");
    let _ = engine.fire_http_task(job_id).await;
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn http_task_fires_and_extracts_response_into_vars() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 1).await;
    let mock = MockServer::start().await;

    Mock::given(m_method("POST"))
        .and(path("/charge"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({ "id": "ch_42", "status": "succeeded" })),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let url = format!("{}/charge", mock.uri());
    let block = r#"
        <conduit:http method="POST">
          <conduit:requestTransform><![CDATA[
            { body: { amount: .vars.amount } }
          ]]></conduit:requestTransform>
          <conduit:responseTransform><![CDATA[
            { charge_id: .body.id, charge_status: .body.status }
          ]]></conduit:responseTransform>
        </conduit:http>"#;
    let bpmn = http_bpmn(&url, block);
    let inst_id = deploy_and_start(&app, org_id, groups[0], &bpmn, json!({ "amount": 1000 })).await;

    fire_job_for_instance(&engine_for(app.pool.clone()), &app.pool, inst_id).await;

    assert_eq!(
        read_var(&app.pool, inst_id, "charge_id").await,
        Some(json!("ch_42"))
    );
    assert_eq!(
        read_var(&app.pool, inst_id, "charge_status").await,
        Some(json!("succeeded"))
    );

    // Job is terminal-completed.
    let job = fetch_job_for(&app.pool, inst_id).await.unwrap();
    assert_eq!(job.state, "completed");
}

#[tokio::test]
async fn request_transform_shapes_body_query_and_headers() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 1).await;
    let mock = MockServer::start().await;

    Mock::given(m_method("PUT"))
        .and(path("/things"))
        .and(header("X-Custom", "from-transform"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "ok": true })))
        .expect(1)
        .mount(&mock)
        .await;

    let url = format!("{}/things", mock.uri());
    let block = r#"
        <conduit:http method="PUT">
          <conduit:requestTransform><![CDATA[
            {
              body:    { name: .vars.name },
              query:   { tag: .vars.tag },
              headers: { "X-Custom": "from-transform" }
            }
          ]]></conduit:requestTransform>
        </conduit:http>"#;
    let bpmn = http_bpmn(&url, block);
    let inst_id = deploy_and_start(
        &app,
        org_id,
        groups[0],
        &bpmn,
        json!({ "name": "widget", "tag": "blue" }),
    )
    .await;

    fire_job_for_instance(&engine_for(app.pool.clone()), &app.pool, inst_id).await;

    let job = fetch_job_for(&app.pool, inst_id).await.unwrap();
    assert_eq!(job.state, "completed", "error={:?}", job.error_message);

    // wiremock's `expect(1)` and `header` matcher would have failed the test if
    // the request didn't carry the expected method/path/header.
    let received = mock.received_requests().await.unwrap();
    assert_eq!(received.len(), 1);
    let received = &received[0];
    assert_eq!(received.url.query(), Some("tag=blue"));
}

#[tokio::test]
async fn bearer_auth_resolves_secret_at_fire_time() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 1).await;
    let mock = MockServer::start().await;

    create_secret(&app, org_id, "stripe_key", "sk_test_abc123").await;

    Mock::given(m_method("POST"))
        .and(path("/charge"))
        .and(header("Authorization", "Bearer sk_test_abc123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": "ch_x" })))
        .expect(1)
        .mount(&mock)
        .await;

    let url = format!("{}/charge", mock.uri());
    let block = r#"
        <conduit:http method="POST" authType="bearer" secretRef="stripe_key">
          <conduit:requestTransform><![CDATA[ { body: { x: 1 } } ]]></conduit:requestTransform>
        </conduit:http>"#;
    let bpmn = http_bpmn(&url, block);
    let inst_id = deploy_and_start(&app, org_id, groups[0], &bpmn, json!({})).await;

    fire_job_for_instance(&engine_for(app.pool.clone()), &app.pool, inst_id).await;
    let job = fetch_job_for(&app.pool, inst_id).await.unwrap();
    assert_eq!(job.state, "completed");
}

#[tokio::test]
async fn auth_header_cannot_be_overridden_by_transform() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 1).await;
    let mock = MockServer::start().await;

    create_secret(&app, org_id, "k", "real_token").await;

    Mock::given(m_method("POST"))
        .and(path("/x"))
        .and(header("Authorization", "Bearer real_token"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&mock)
        .await;

    let url = format!("{}/x", mock.uri());
    let block = r#"
        <conduit:http method="POST" authType="bearer" secretRef="k">
          <conduit:requestTransform><![CDATA[
            { headers: { "Authorization": "Bearer attacker_token" }, body: {} }
          ]]></conduit:requestTransform>
        </conduit:http>"#;
    let bpmn = http_bpmn(&url, block);
    let inst_id = deploy_and_start(&app, org_id, groups[0], &bpmn, json!({})).await;

    fire_job_for_instance(&engine_for(app.pool.clone()), &app.pool, inst_id).await;
    // wiremock's `expect(1)` + `header("Authorization", "Bearer real_token")`
    // would have made the test fail if the transform's value won.
}

#[tokio::test]
async fn five_xx_triggers_retry() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 1).await;
    let mock = MockServer::start().await;

    Mock::given(m_method("POST"))
        .and(path("/flaky"))
        .respond_with(ResponseTemplate::new(503))
        .expect(1)
        .mount(&mock)
        .await;

    let url = format!("{}/flaky", mock.uri());
    let block = r#"
        <conduit:http method="POST">
          <conduit:retry max="3" backoffMs="10" multiplier="1" retryOn="5xx"/>
        </conduit:http>"#;
    let bpmn = http_bpmn(&url, block);
    let inst_id = deploy_and_start(&app, org_id, groups[0], &bpmn, json!({})).await;

    fire_job_for_instance(&engine_for(app.pool.clone()), &app.pool, inst_id).await;
    let job = fetch_job_for(&app.pool, inst_id).await.unwrap();
    // Retryable failure: state is back to pending, retry_count=1, due_date pushed.
    assert_eq!(job.state, "pending");
    assert_eq!(job.retry_count, 1);
    assert!(job.error_message.as_deref().unwrap_or("").contains("503"));
}

#[tokio::test]
async fn four_xx_does_not_retry_by_default() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 1).await;
    let mock = MockServer::start().await;

    Mock::given(m_method("POST"))
        .and(path("/bad"))
        .respond_with(ResponseTemplate::new(400))
        .mount(&mock)
        .await;

    let url = format!("{}/bad", mock.uri());
    let block = r#"
        <conduit:http method="POST">
          <conduit:retry max="3" backoffMs="10" multiplier="1"/>
        </conduit:http>"#;
    let bpmn = http_bpmn(&url, block);
    let inst_id = deploy_and_start(&app, org_id, groups[0], &bpmn, json!({})).await;

    fire_job_for_instance(&engine_for(app.pool.clone()), &app.pool, inst_id).await;
    let job = fetch_job_for(&app.pool, inst_id).await.unwrap();
    assert_eq!(job.state, "failed", "default policy should not retry 4xx");
    assert_eq!(job.retry_count, 1);
}

#[tokio::test]
async fn timeout_classified_as_retryable() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 1).await;
    let mock = MockServer::start().await;

    Mock::given(m_method("POST"))
        .and(path("/slow"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_millis(500)))
        .mount(&mock)
        .await;

    let url = format!("{}/slow", mock.uri());
    let block = r#"
        <conduit:http method="POST" timeoutMs="50">
          <conduit:retry max="2" backoffMs="10" multiplier="1" retryOn="timeout"/>
        </conduit:http>"#;
    let bpmn = http_bpmn(&url, block);
    let inst_id = deploy_and_start(&app, org_id, groups[0], &bpmn, json!({})).await;

    fire_job_for_instance(&engine_for(app.pool.clone()), &app.pool, inst_id).await;
    let job = fetch_job_for(&app.pool, inst_id).await.unwrap();
    assert_eq!(job.state, "pending", "timeout should retry");
    assert!(job
        .error_message
        .as_deref()
        .unwrap_or("")
        .contains("timeout"));
}

#[tokio::test]
async fn missing_secret_fails_task() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 1).await;
    let mock = MockServer::start().await;

    Mock::given(m_method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock)
        .await;

    let url = format!("{}/whatever", mock.uri());
    let block = r#"
        <conduit:http method="POST" authType="bearer" secretRef="not_created"/>
    "#;
    let bpmn = http_bpmn(&url, block);
    let inst_id = deploy_and_start(&app, org_id, groups[0], &bpmn, json!({})).await;

    fire_job_for_instance(&engine_for(app.pool.clone()), &app.pool, inst_id).await;
    let job = fetch_job_for(&app.pool, inst_id).await.unwrap();
    assert_eq!(job.state, "failed");
    assert!(job
        .error_message
        .as_deref()
        .unwrap_or("")
        .contains("not_created"));
}

#[tokio::test]
async fn legacy_url_only_service_task_still_fires() {
    // Regression guard: a serviceTask with just `url=...` and no <conduit:http>
    // must continue to POST the legacy envelope and complete the job.
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 1).await;
    let mock = MockServer::start().await;

    Mock::given(m_method("POST"))
        .and(path("/legacy"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "variables": [
                { "name": "result", "value_type": "string", "value": "ok" }
            ]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let url = format!("{}/legacy", mock.uri());
    let bpmn = format!(
        r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL">
  <process id="legacy" isExecutable="true">
    <startEvent id="s"/>
    <serviceTask id="call" url="{url}"/>
    <endEvent id="e"/>
    <sequenceFlow id="f1" sourceRef="s" targetRef="call"/>
    <sequenceFlow id="f2" sourceRef="call" targetRef="e"/>
  </process>
</definitions>"#
    );
    let inst_id = deploy_and_start(&app, org_id, groups[0], &bpmn, json!({})).await;

    fire_job_for_instance(&engine_for(app.pool.clone()), &app.pool, inst_id).await;
    let job = fetch_job_for(&app.pool, inst_id).await.unwrap();
    assert_eq!(job.state, "completed");
    assert_eq!(
        read_var(&app.pool, inst_id, "result").await,
        Some(json!("ok"))
    );
}

#[tokio::test]
async fn get_method_dispatches_correctly() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 1).await;
    let mock = MockServer::start().await;

    Mock::given(m_method("GET"))
        .and(path("/items"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "n": 7 })))
        .expect(1)
        .mount(&mock)
        .await;

    let url = format!("{}/items", mock.uri());
    let block = r#"
        <conduit:http method="GET">
          <conduit:responseTransform><![CDATA[ { count: .body.n } ]]></conduit:responseTransform>
        </conduit:http>"#;
    let bpmn = http_bpmn(&url, block);
    let inst_id = deploy_and_start(&app, org_id, groups[0], &bpmn, json!({})).await;

    fire_job_for_instance(&engine_for(app.pool.clone()), &app.pool, inst_id).await;
    assert_eq!(read_var(&app.pool, inst_id, "count").await, Some(json!(7)));
}

#[tokio::test]
async fn jq_runtime_error_fails_task() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 1).await;
    let mock = MockServer::start().await;

    Mock::given(m_method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "x": "not_a_number" })))
        .mount(&mock)
        .await;

    let url = format!("{}/x", mock.uri());
    // tonumber on a non-numeric string with no `?` raises.
    let block = r#"
        <conduit:http method="POST">
          <conduit:responseTransform><![CDATA[ { v: (.body.x | tonumber) } ]]></conduit:responseTransform>
        </conduit:http>"#;
    let bpmn = http_bpmn(&url, block);
    let inst_id = deploy_and_start(&app, org_id, groups[0], &bpmn, json!({})).await;

    fire_job_for_instance(&engine_for(app.pool.clone()), &app.pool, inst_id).await;
    let job = fetch_job_for(&app.pool, inst_id).await.unwrap();
    assert_eq!(job.state, "failed");
}

// ─── errorCodeExpression tests ────────────────────────────────────────────────

/// Build a BPMN with a serviceTask that has a `<conduit:http>` block plus an
/// interrupting boundary error event that catches `error_code` (empty string
/// = catch-all). The normal path leads to `end`; the error path to `end_error`.
fn http_bpmn_with_boundary(mock_url: &str, http_block: &str, error_code: &str) -> String {
    let error_ref = if error_code.is_empty() {
        // catch-all: no errorRef
        "".to_string()
    } else {
        r#" errorRef="err1""#.to_string()
    };
    let error_def = if error_code.is_empty() {
        String::new()
    } else {
        format!(r#"  <error id="err1" name="PaymentFailed" errorCode="{error_code}"/>"#)
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL"
             xmlns:conduit="http://conduit.io/ext"
             targetNamespace="http://conduit.io/test">
{error_def}
  <process id="http_boundary_proc" isExecutable="true">
    <startEvent id="start"/>
    <serviceTask id="call" name="Call" url="{mock_url}">
      <extensionElements>
{http_block}
      </extensionElements>
    </serviceTask>
    <boundaryEvent id="on_error" attachedToRef="call" cancelActivity="true">
      <errorEventDefinition{error_ref}/>
    </boundaryEvent>
    <endEvent id="end"/>
    <endEvent id="end_error"/>
    <sequenceFlow id="f1" sourceRef="start" targetRef="call"/>
    <sequenceFlow id="f2" sourceRef="call" targetRef="end"/>
    <sequenceFlow id="f3" sourceRef="on_error" targetRef="end_error"/>
  </process>
</definitions>"#
    )
}

async fn instance_state(pool: &PgPool, instance_id: Uuid) -> String {
    sqlx::query_scalar::<_, String>("SELECT state FROM process_instances WHERE id = $1")
        .bind(instance_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn error_code_expression_on_2xx_routes_to_boundary() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 1).await;
    let mock = MockServer::start().await;

    Mock::given(m_method("POST"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({ "error_code": "PAYMENT_FAILED", "msg": "card declined" })),
        )
        .mount(&mock)
        .await;

    let url = format!("{}/pay", mock.uri());
    let block = r#"
        <conduit:http method="POST">
          <conduit:errorCodeExpression><![CDATA[ .body.error_code // "" ]]></conduit:errorCodeExpression>
        </conduit:http>"#;
    let bpmn = http_bpmn_with_boundary(&url, block, "PAYMENT_FAILED");
    let inst_id = deploy_and_start(&app, org_id, groups[0], &bpmn, json!({})).await;

    fire_job_for_instance(&engine_for(app.pool.clone()), &app.pool, inst_id).await;

    // Job should be cancelled (interrupting boundary cancelled the service task job).
    let job = fetch_job_for(&app.pool, inst_id).await.unwrap();
    assert_eq!(
        job.state, "cancelled",
        "job should be cancelled by interrupting boundary"
    );

    // Instance should complete via the error path.
    assert_eq!(instance_state(&app.pool, inst_id).await, "completed");
}

#[tokio::test]
async fn error_code_expression_null_follows_normal_path() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 1).await;
    let mock = MockServer::start().await;

    // Body has no `error_code` field — expression returns null/empty.
    Mock::given(m_method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "result": "ok" })))
        .mount(&mock)
        .await;

    let url = format!("{}/pay", mock.uri());
    let block = r#"
        <conduit:http method="POST">
          <conduit:errorCodeExpression><![CDATA[ .body.error_code // "" ]]></conduit:errorCodeExpression>
        </conduit:http>"#;
    let bpmn = http_bpmn_with_boundary(&url, block, "PAYMENT_FAILED");
    let inst_id = deploy_and_start(&app, org_id, groups[0], &bpmn, json!({})).await;

    fire_job_for_instance(&engine_for(app.pool.clone()), &app.pool, inst_id).await;

    // Normal path: job completed, instance completed.
    let job = fetch_job_for(&app.pool, inst_id).await.unwrap();
    assert_eq!(
        job.state, "completed",
        "expression returned empty — normal completion expected"
    );
    assert_eq!(instance_state(&app.pool, inst_id).await, "completed");
}

#[tokio::test]
async fn error_code_expression_no_boundary_terminates_instance() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 1).await;
    let mock = MockServer::start().await;

    Mock::given(m_method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "error_code": "UNKNOWN_CODE" })),
        )
        .mount(&mock)
        .await;

    let url = format!("{}/pay", mock.uri());
    // errorCodeExpression on a process with NO boundary event at all.
    let block = r#"
        <conduit:http method="POST">
          <conduit:errorCodeExpression><![CDATA[ .body.error_code // "" ]]></conduit:errorCodeExpression>
        </conduit:http>"#;
    // Use http_bpmn (no boundary) so there's nowhere to route the error.
    let bpmn = http_bpmn(&url, block);
    let inst_id = deploy_and_start(&app, org_id, groups[0], &bpmn, json!({})).await;

    fire_job_for_instance(&engine_for(app.pool.clone()), &app.pool, inst_id).await;

    let job = fetch_job_for(&app.pool, inst_id).await.unwrap();
    assert_eq!(job.state, "failed", "no boundary → job must be failed");
    assert_eq!(instance_state(&app.pool, inst_id).await, "error");
}

#[tokio::test]
async fn error_code_expression_on_4xx_intercepts_failure() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 1).await;
    let mock = MockServer::start().await;

    // 422 response with a structured error body.
    Mock::given(m_method("POST"))
        .respond_with(
            ResponseTemplate::new(422)
                .set_body_json(json!({ "error_code": "VALIDATION_ERROR", "detail": "bad input" })),
        )
        .mount(&mock)
        .await;

    let url = format!("{}/pay", mock.uri());
    let block = r#"
        <conduit:http method="POST">
          <conduit:errorCodeExpression><![CDATA[ .body.error_code // "" ]]></conduit:errorCodeExpression>
        </conduit:http>"#;
    // Catch-all boundary (empty errorCode) — catches any BPMN error.
    let bpmn = http_bpmn_with_boundary(&url, block, "");
    let inst_id = deploy_and_start(&app, org_id, groups[0], &bpmn, json!({})).await;

    fire_job_for_instance(&engine_for(app.pool.clone()), &app.pool, inst_id).await;

    // Expression intercepted the 4xx before the failure path.
    let job = fetch_job_for(&app.pool, inst_id).await.unwrap();
    assert_eq!(
        job.state, "cancelled",
        "errorCodeExpression on 4xx should route to boundary, not retry/fail"
    );
    assert_eq!(instance_state(&app.pool, inst_id).await, "completed");
}
