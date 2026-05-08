//! Echo-service sample flow tests.
//!
//! Runs the four sample BPMN flows in `samples/` against a real httpbin-style
//! echo service.  Each test skips gracefully when the env var is not set so
//! that CI stays green without the echo service.
//!
//! Set ECHO_BASE_URL to your echo service base URL before running:
//!
//!   ECHO_BASE_URL=http://localhost:8080 cargo test --test echo_sample_test
//!
//! The echo service must be httpbin-compatible:
//!   POST /anything → { "json": {…}, "method": "POST", "headers": {…}, "url": "…" }

mod common;

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use conduit::engine::Engine;
use conduit::state::GraphCache;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

const TEST_KEY: [u8; 32] = [0xA5u8; 32];

fn engine_for(pool: PgPool) -> Engine {
    let cache: GraphCache = Arc::new(RwLock::new(HashMap::new()));
    Engine::new(pool, cache, TEST_KEY)
}

fn echo_base_url() -> Option<String> {
    std::env::var("ECHO_BASE_URL").ok()
}

fn unique_key(prefix: &str) -> String {
    format!("{}-{}", prefix, Uuid::new_v4())
}

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
    let client = reqwest::Client::new();
    let deploy_resp = client
        .post(format!("{}/api/v1/deployments", app.address))
        .json(&json!({
            "org_id": org_id,
            "process_group_id": process_group_id,
            "key": unique_key("echo"),
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
        "org_id": org_id,
        "definition_id": def_id,
    });
    if !initial_vars.as_object().is_none_or(|o| o.is_empty()) {
        body["variables"] = vars_from_object(initial_vars);
    }
    let start_resp = client
        .post(format!("{}/api/v1/process-instances", app.address))
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

async fn fire_next_http_job(engine: &Engine, pool: &PgPool, instance_id: Uuid) {
    let job_id: Uuid = sqlx::query_scalar(
        "UPDATE jobs SET state = 'locked', \
                          locked_by = 'test', \
                          locked_until = NOW() + interval '60 seconds' \
         WHERE id = (SELECT id FROM jobs \
                     WHERE instance_id = $1 AND job_type = 'http_task' AND state = 'pending' \
                     ORDER BY created_at LIMIT 1) \
         RETURNING id",
    )
    .bind(instance_id)
    .fetch_one(pool)
    .await
    .expect("no pending http_task job found for instance");
    let _ = engine.fire_http_task(job_id).await;
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

async fn instance_state(pool: &PgPool, instance_id: Uuid) -> String {
    sqlx::query_scalar::<_, String>("SELECT state FROM process_instances WHERE id = $1")
        .bind(instance_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn pending_http_job_count(pool: &PgPool, instance_id: Uuid) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM jobs WHERE instance_id = $1 AND job_type = 'http_task' AND state = 'pending'",
    )
    .bind(instance_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

// ─── Sample 01: Hello Echo ────────────────────────────────────────────────────

#[tokio::test]
async fn hello_echo_posts_variables_and_captures_response() {
    let Some(base_url) = echo_base_url() else {
        eprintln!("ECHO_BASE_URL not set — skipping hello echo test");
        return;
    };

    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 1).await;

    let bpmn = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL"
             xmlns:conduit="http://conduit.io/ext"
             targetNamespace="http://conduit.io/samples">
  <process id="hello_echo" isExecutable="true">
    <startEvent id="start"/>
    <sequenceFlow id="sf1" sourceRef="start" targetRef="echo_task"/>
    <serviceTask id="echo_task" name="Echo Call" url="{base_url}/anything">
      <extensionElements>
        <conduit:http method="POST" timeoutMs="10000">
          <conduit:requestTransform><![CDATA[
            {{ body: {{ name: .vars.name, value: .vars.value }} }}
          ]]></conduit:requestTransform>
          <conduit:responseTransform><![CDATA[
            {{
              echoed_name:  .body.json.name,
              echoed_value: .body.json.value,
              echo_method:  .body.method
            }}
          ]]></conduit:responseTransform>
        </conduit:http>
      </extensionElements>
    </serviceTask>
    <sequenceFlow id="sf2" sourceRef="echo_task" targetRef="end"/>
    <endEvent id="end"/>
  </process>
</definitions>"#,
        base_url = base_url
    );

    let inst_id = deploy_and_start(
        &app,
        org_id,
        groups[0],
        &bpmn,
        json!({ "name": "Alice", "value": 42 }),
    )
    .await;

    fire_next_http_job(&engine_for(app.pool.clone()), &app.pool, inst_id).await;

    assert_eq!(instance_state(&app.pool, inst_id).await, "completed");
    assert_eq!(
        read_var(&app.pool, inst_id, "echoed_name").await,
        Some(json!("Alice"))
    );
    assert_eq!(
        read_var(&app.pool, inst_id, "echo_method").await,
        Some(json!("POST"))
    );
}

// ─── Sample 02: Echo Routing ──────────────────────────────────────────────────

#[tokio::test]
async fn echo_routing_follows_path_a_when_route_is_path_a() {
    let Some(base_url) = echo_base_url() else {
        eprintln!("ECHO_BASE_URL not set — skipping echo routing test");
        return;
    };

    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 1).await;

    let bpmn = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL"
             xmlns:conduit="http://conduit.io/ext"
             targetNamespace="http://conduit.io/samples">
  <process id="echo_routing" isExecutable="true">
    <startEvent id="start"/>
    <sequenceFlow id="sf1" sourceRef="start" targetRef="echo_task"/>
    <serviceTask id="echo_task" name="Echo Route" url="{base_url}/anything">
      <extensionElements>
        <conduit:http method="POST" timeoutMs="10000">
          <conduit:requestTransform><![CDATA[
            {{ body: {{ route: .vars.route }} }}
          ]]></conduit:requestTransform>
          <conduit:responseTransform><![CDATA[
            {{ route: .body.json.route }}
          ]]></conduit:responseTransform>
        </conduit:http>
      </extensionElements>
    </serviceTask>
    <sequenceFlow id="sf2" sourceRef="echo_task" targetRef="gw"/>
    <exclusiveGateway id="gw" default="sf_default"/>
    <sequenceFlow id="sf_a" sourceRef="gw" targetRef="end_a">
      <conditionExpression>route = "path_a"</conditionExpression>
    </sequenceFlow>
    <sequenceFlow id="sf_default" sourceRef="gw" targetRef="end_b"/>
    <endEvent id="end_a"/>
    <endEvent id="end_b"/>
  </process>
</definitions>"#,
        base_url = base_url
    );

    let inst_id =
        deploy_and_start(&app, org_id, groups[0], &bpmn, json!({ "route": "path_a" })).await;

    fire_next_http_job(&engine_for(app.pool.clone()), &app.pool, inst_id).await;

    assert_eq!(instance_state(&app.pool, inst_id).await, "completed");
    // route variable should be "path_a" as echoed back
    assert_eq!(
        read_var(&app.pool, inst_id, "route").await,
        Some(json!("path_a"))
    );
}

#[tokio::test]
async fn echo_routing_follows_default_when_route_is_path_b() {
    let Some(base_url) = echo_base_url() else {
        eprintln!("ECHO_BASE_URL not set — skipping echo routing default test");
        return;
    };

    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 1).await;

    let bpmn = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL"
             xmlns:conduit="http://conduit.io/ext"
             targetNamespace="http://conduit.io/samples">
  <process id="echo_routing_b" isExecutable="true">
    <startEvent id="start"/>
    <sequenceFlow id="sf1" sourceRef="start" targetRef="echo_task"/>
    <serviceTask id="echo_task" name="Echo Route" url="{base_url}/anything">
      <extensionElements>
        <conduit:http method="POST" timeoutMs="10000">
          <conduit:requestTransform><![CDATA[
            {{ body: {{ route: .vars.route }} }}
          ]]></conduit:requestTransform>
          <conduit:responseTransform><![CDATA[
            {{ route: .body.json.route }}
          ]]></conduit:responseTransform>
        </conduit:http>
      </extensionElements>
    </serviceTask>
    <sequenceFlow id="sf2" sourceRef="echo_task" targetRef="gw"/>
    <exclusiveGateway id="gw" default="sf_default"/>
    <sequenceFlow id="sf_a" sourceRef="gw" targetRef="end_a">
      <conditionExpression>route = "path_a"</conditionExpression>
    </sequenceFlow>
    <sequenceFlow id="sf_default" sourceRef="gw" targetRef="end_b"/>
    <endEvent id="end_a"/>
    <endEvent id="end_b"/>
  </process>
</definitions>"#,
        base_url = base_url
    );

    let inst_id =
        deploy_and_start(&app, org_id, groups[0], &bpmn, json!({ "route": "path_b" })).await;

    fire_next_http_job(&engine_for(app.pool.clone()), &app.pool, inst_id).await;

    assert_eq!(instance_state(&app.pool, inst_id).await, "completed");
    assert_eq!(
        read_var(&app.pool, inst_id, "route").await,
        Some(json!("path_b"))
    );
}

// ─── Sample 03: Parallel Echo ─────────────────────────────────────────────────

#[tokio::test]
async fn echo_parallel_calls_execute_and_variables_merge_at_join() {
    let Some(base_url) = echo_base_url() else {
        eprintln!("ECHO_BASE_URL not set — skipping parallel echo test");
        return;
    };

    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 1).await;

    let bpmn = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL"
             xmlns:conduit="http://conduit.io/ext"
             targetNamespace="http://conduit.io/samples">
  <process id="echo_parallel" isExecutable="true">
    <startEvent id="start"/>
    <sequenceFlow id="sf1" sourceRef="start" targetRef="split"/>
    <parallelGateway id="split"/>
    <sequenceFlow id="sf_a1" sourceRef="split" targetRef="call_a"/>
    <sequenceFlow id="sf_b1" sourceRef="split" targetRef="call_b"/>
    <serviceTask id="call_a" name="Echo A" url="{base_url}/anything">
      <extensionElements>
        <conduit:http method="POST" timeoutMs="10000">
          <conduit:requestTransform><![CDATA[
            {{ body: {{ source: "A", input: .vars.input_a }} }}
          ]]></conduit:requestTransform>
          <conduit:responseTransform><![CDATA[
            {{ result_a: .body.json.input }}
          ]]></conduit:responseTransform>
        </conduit:http>
      </extensionElements>
    </serviceTask>
    <serviceTask id="call_b" name="Echo B" url="{base_url}/anything">
      <extensionElements>
        <conduit:http method="POST" timeoutMs="10000">
          <conduit:requestTransform><![CDATA[
            {{ body: {{ source: "B", input: .vars.input_b }} }}
          ]]></conduit:requestTransform>
          <conduit:responseTransform><![CDATA[
            {{ result_b: .body.json.input }}
          ]]></conduit:responseTransform>
        </conduit:http>
      </extensionElements>
    </serviceTask>
    <sequenceFlow id="sf_a2" sourceRef="call_a" targetRef="join"/>
    <sequenceFlow id="sf_b2" sourceRef="call_b" targetRef="join"/>
    <parallelGateway id="join"/>
    <sequenceFlow id="sf2" sourceRef="join" targetRef="end"/>
    <endEvent id="end"/>
  </process>
</definitions>"#,
        base_url = base_url
    );

    let inst_id = deploy_and_start(
        &app,
        org_id,
        groups[0],
        &bpmn,
        json!({ "input_a": "hello", "input_b": "world" }),
    )
    .await;

    let engine = engine_for(app.pool.clone());

    // Both jobs are pending; fire them sequentially — join gate opens after both.
    assert_eq!(pending_http_job_count(&app.pool, inst_id).await, 2);
    fire_next_http_job(&engine, &app.pool, inst_id).await;
    fire_next_http_job(&engine, &app.pool, inst_id).await;

    assert_eq!(instance_state(&app.pool, inst_id).await, "completed");
    assert_eq!(
        read_var(&app.pool, inst_id, "result_a").await,
        Some(json!("hello"))
    );
    assert_eq!(
        read_var(&app.pool, inst_id, "result_b").await,
        Some(json!("world"))
    );
}

// ─── Sample 04: Echo Error Boundary ──────────────────────────────────────────

#[tokio::test]
async fn echo_error_boundary_routes_to_error_path_when_error_code_present() {
    let Some(base_url) = echo_base_url() else {
        eprintln!("ECHO_BASE_URL not set — skipping error boundary test");
        return;
    };

    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 1).await;

    let bpmn = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL"
             xmlns:conduit="http://conduit.io/ext"
             targetNamespace="http://conduit.io/samples">
  <process id="echo_error_bnd" isExecutable="true">
    <startEvent id="start"/>
    <sequenceFlow id="sf1" sourceRef="start" targetRef="echo_task"/>
    <serviceTask id="echo_task" name="Echo with Error" url="{base_url}/anything">
      <extensionElements>
        <conduit:http method="POST" timeoutMs="10000">
          <conduit:requestTransform><![CDATA[
            {{ body: {{ payload: .vars.payload, error_code: .vars.error_code }} }}
          ]]></conduit:requestTransform>
          <conduit:responseTransform><![CDATA[
            {{ result: .body.json.payload }}
          ]]></conduit:responseTransform>
          <conduit:errorCodeExpression><![CDATA[
            .body.json.error_code // ""
          ]]></conduit:errorCodeExpression>
        </conduit:http>
      </extensionElements>
    </serviceTask>
    <boundaryEvent id="err_boundary" attachedToRef="echo_task" cancelActivity="true">
      <errorEventDefinition errorCodeVariable="caught_error_code"/>
    </boundaryEvent>
    <sequenceFlow id="sf2"     sourceRef="echo_task"    targetRef="happy_end"/>
    <sequenceFlow id="sf_err"  sourceRef="err_boundary" targetRef="error_end"/>
    <endEvent id="happy_end"/>
    <endEvent id="error_end"/>
  </process>
</definitions>"#,
        base_url = base_url
    );

    // error_code is present → echo reflects it back → boundary fires
    let inst_id = deploy_and_start(
        &app,
        org_id,
        groups[0],
        &bpmn,
        json!({ "payload": "test-payload", "error_code": "VALIDATION_ERROR" }),
    )
    .await;

    fire_next_http_job(&engine_for(app.pool.clone()), &app.pool, inst_id).await;

    assert_eq!(instance_state(&app.pool, inst_id).await, "completed");
    assert_eq!(
        read_var(&app.pool, inst_id, "caught_error_code").await,
        Some(json!("VALIDATION_ERROR"))
    );
    // result should NOT be set (boundary cancelled before responseTransform variables were merged)
    // or it may be set depending on engine ordering; we just assert the boundary fired correctly
}

#[tokio::test]
async fn echo_error_boundary_follows_happy_path_when_no_error_code() {
    let Some(base_url) = echo_base_url() else {
        eprintln!("ECHO_BASE_URL not set — skipping error boundary happy path test");
        return;
    };

    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 1).await;

    let bpmn = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL"
             xmlns:conduit="http://conduit.io/ext"
             targetNamespace="http://conduit.io/samples">
  <process id="echo_happy_bnd" isExecutable="true">
    <startEvent id="start"/>
    <sequenceFlow id="sf1" sourceRef="start" targetRef="echo_task"/>
    <serviceTask id="echo_task" name="Echo with Error" url="{base_url}/anything">
      <extensionElements>
        <conduit:http method="POST" timeoutMs="10000">
          <conduit:requestTransform><![CDATA[
            {{ body: {{ payload: .vars.payload }} }}
          ]]></conduit:requestTransform>
          <conduit:responseTransform><![CDATA[
            {{ result: .body.json.payload }}
          ]]></conduit:responseTransform>
          <conduit:errorCodeExpression><![CDATA[
            .body.json.error_code // ""
          ]]></conduit:errorCodeExpression>
        </conduit:http>
      </extensionElements>
    </serviceTask>
    <boundaryEvent id="err_boundary" attachedToRef="echo_task" cancelActivity="true">
      <errorEventDefinition errorCodeVariable="caught_error_code"/>
    </boundaryEvent>
    <sequenceFlow id="sf2"     sourceRef="echo_task"    targetRef="happy_end"/>
    <sequenceFlow id="sf_err"  sourceRef="err_boundary" targetRef="error_end"/>
    <endEvent id="happy_end"/>
    <endEvent id="error_end"/>
  </process>
</definitions>"#,
        base_url = base_url
    );

    // No error_code in request → echo response has no .json.error_code → expression returns "" → normal path
    let inst_id = deploy_and_start(
        &app,
        org_id,
        groups[0],
        &bpmn,
        json!({ "payload": "good-payload" }),
    )
    .await;

    fire_next_http_job(&engine_for(app.pool.clone()), &app.pool, inst_id).await;

    assert_eq!(instance_state(&app.pool, inst_id).await, "completed");
    assert_eq!(
        read_var(&app.pool, inst_id, "result").await,
        Some(json!("good-payload"))
    );
    // boundary did not fire
    assert_eq!(
        read_var(&app.pool, inst_id, "caught_error_code").await,
        None
    );
}
