use conduit::db;
use conduit::engine::{Engine, VariableInput};
use conduit::state::GraphCache;
use serde_json::json;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

async fn setup() -> (PgPool, Engine) {
    let database_url = std::env::var("TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .expect("TEST_DATABASE_URL or DATABASE_URL must be set for integration tests");

    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to test database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    let cache: GraphCache = Arc::new(RwLock::new(HashMap::new()));
    let engine = Engine::new(pool.clone(), cache);
    (pool, engine)
}

async fn create_org(pool: &PgPool) -> (Uuid, Vec<Uuid>) {
    let slug = format!("eng-org-{}", Uuid::new_v4());
    let org = db::orgs::insert(pool, "Engine Test Org", &slug)
        .await
        .unwrap();
    let f1 = db::process_groups::insert(pool, org.id, "Primary").await.unwrap();
    let f2 = db::process_groups::insert(pool, org.id, "Secondary").await.unwrap();
    (org.id, vec![f1.id, f2.id])
}

fn unique_key(prefix: &str) -> String {
    format!("{}-{}", prefix, Uuid::new_v4())
}

/// Start → UserTask → End
fn linear_bpmn() -> String {
    r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def1" targetNamespace="urn:test">
  <process id="proc" isExecutable="true">
    <startEvent id="start"/>
    <userTask id="task1" name="Do the thing"/>
    <endEvent id="end"/>
    <sequenceFlow id="sf1" sourceRef="start" targetRef="task1"/>
    <sequenceFlow id="sf2" sourceRef="task1" targetRef="end"/>
  </process>
</definitions>"#
        .to_string()
}

/// Start → End (no tasks)
fn start_to_end_bpmn() -> String {
    r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def1" targetNamespace="urn:test">
  <process id="proc" isExecutable="true">
    <startEvent id="start"/>
    <endEvent id="end"/>
    <sequenceFlow id="sf1" sourceRef="start" targetRef="end"/>
  </process>
</definitions>"#
        .to_string()
}

/// Start → ServiceTask → End
fn service_task_bpmn() -> String {
    r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def1" targetNamespace="urn:test">
  <process id="proc" isExecutable="true">
    <startEvent id="start"/>
    <serviceTask id="svc1" name="Call API" topic="my-topic"/>
    <endEvent id="end"/>
    <sequenceFlow id="sf1" sourceRef="start" targetRef="svc1"/>
    <sequenceFlow id="sf2" sourceRef="svc1" targetRef="end"/>
  </process>
</definitions>"#
        .to_string()
}

// ─── start_instance ──────────────────────────────────────────────────────────

#[tokio::test]
async fn start_instance_creates_running_instance() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("eng"),
        1,
        None,
        &linear_bpmn(),
        &json!({}),
    )
    .await
    .unwrap();

    let instance = engine
        .start_instance(def.id, org_id, &json!({}), &[])
        .await
        .unwrap();

    assert_eq!(instance.definition_id, def.id);
    assert_eq!(instance.state, "running");
    assert!(instance.ended_at.is_none());
}

#[tokio::test]
async fn start_instance_creates_pending_user_task() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("eng"),
        1,
        None,
        &linear_bpmn(),
        &json!({}),
    )
    .await
    .unwrap();

    let instance = engine
        .start_instance(def.id, org_id, &json!({}), &[])
        .await
        .unwrap();

    let tasks = db::tasks::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].element_id, "task1");
    assert_eq!(tasks[0].task_type, "user_task");
    assert_eq!(tasks[0].state, "pending");
    assert_eq!(tasks[0].name.as_deref(), Some("Do the thing"));
}

#[tokio::test]
async fn start_to_end_completes_instance_immediately() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("eng"),
        1,
        None,
        &start_to_end_bpmn(),
        &json!({}),
    )
    .await
    .unwrap();

    let instance = engine
        .start_instance(def.id, org_id, &json!({}), &[])
        .await
        .unwrap();

    assert_eq!(instance.state, "completed");
    assert!(instance.ended_at.is_some());

    let tasks = db::tasks::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(tasks.len(), 0);
}

#[tokio::test]
async fn start_instance_unknown_definition_returns_not_found() {
    let (pool, engine) = setup().await;
    let (org_id, _groups) = create_org(&pool).await;
    let result = engine
        .start_instance(Uuid::new_v4(), org_id, &json!({}), &[])
        .await;
    assert!(matches!(
        result,
        Err(conduit::error::EngineError::NotFound(_))
    ));
}

#[tokio::test]
async fn start_instance_with_service_task() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("eng"),
        1,
        None,
        &service_task_bpmn(),
        &json!({}),
    )
    .await
    .unwrap();

    let instance = engine
        .start_instance(def.id, org_id, &json!({}), &[])
        .await
        .unwrap();

    assert_eq!(instance.state, "running");

    let tasks = db::tasks::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(
        tasks.len(),
        0,
        "service tasks should not create task records"
    );

    let jobs = db::jobs::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].job_type, "external_task");
    assert_eq!(jobs[0].state, "pending");
    assert_eq!(jobs[0].topic.as_deref(), Some("my-topic"));
}

// ─── execution_history audit ─────────────────────────────────────────────────

#[tokio::test]
async fn start_instance_writes_history_for_each_element_visited() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("eng"),
        1,
        None,
        &linear_bpmn(),
        &json!({}),
    )
    .await
    .unwrap();

    let instance = engine
        .start_instance(def.id, org_id, &json!({}), &[])
        .await
        .unwrap();

    let history = db::execution_history::list_by_instance(&pool, instance.id)
        .await
        .unwrap();

    // StartEvent (left_at set) + UserTask (left_at still null)
    assert_eq!(history.len(), 2);

    let start_entry = history.iter().find(|h| h.element_id == "start").unwrap();
    assert_eq!(start_entry.element_type, "startEvent");
    assert!(start_entry.left_at.is_some());

    let task_entry = history.iter().find(|h| h.element_id == "task1").unwrap();
    assert_eq!(task_entry.element_type, "userTask");
    assert!(task_entry.left_at.is_none());
}

#[tokio::test]
async fn start_to_end_all_history_entries_closed() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("eng"),
        1,
        None,
        &start_to_end_bpmn(),
        &json!({}),
    )
    .await
    .unwrap();

    let instance = engine
        .start_instance(def.id, org_id, &json!({}), &[])
        .await
        .unwrap();

    let history = db::execution_history::list_by_instance(&pool, instance.id)
        .await
        .unwrap();

    assert_eq!(history.len(), 2);
    for entry in &history {
        assert!(
            entry.left_at.is_some(),
            "expected left_at set for {}",
            entry.element_id
        );
    }
}

// ─── complete_user_task ───────────────────────────────────────────────────────

#[tokio::test]
async fn complete_user_task_advances_token_to_end() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("eng"),
        1,
        None,
        &linear_bpmn(),
        &json!({}),
    )
    .await
    .unwrap();
    let instance = engine
        .start_instance(def.id, org_id, &json!({}), &[])
        .await
        .unwrap();

    let tasks = db::tasks::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    let task_id = tasks[0].id;

    engine.complete_user_task(task_id, &[]).await.unwrap();

    // Instance should now be completed
    let refreshed = db::process_instances::get_by_id(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(refreshed.state, "completed");
    assert!(refreshed.ended_at.is_some());

    // Task should be completed
    let task = db::tasks::get_by_id(&pool, task_id).await.unwrap();
    assert_eq!(task.state, "completed");
    assert!(task.completed_at.is_some());
}

#[tokio::test]
async fn complete_user_task_closes_history_entry() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("eng"),
        1,
        None,
        &linear_bpmn(),
        &json!({}),
    )
    .await
    .unwrap();
    let instance = engine
        .start_instance(def.id, org_id, &json!({}), &[])
        .await
        .unwrap();
    let task_id = db::tasks::list_by_instance(&pool, instance.id)
        .await
        .unwrap()[0]
        .id;

    engine.complete_user_task(task_id, &[]).await.unwrap();

    let history = db::execution_history::list_by_instance(&pool, instance.id)
        .await
        .unwrap();

    // start + userTask + endEvent = 3 entries, all closed
    assert_eq!(history.len(), 3);
    for entry in &history {
        assert!(
            entry.left_at.is_some(),
            "expected left_at set for {}",
            entry.element_id
        );
    }
}

#[tokio::test]
async fn complete_task_not_found_returns_error() {
    let (_, engine) = setup().await;
    let result = engine.complete_user_task(Uuid::new_v4(), &[]).await;
    assert!(matches!(
        result,
        Err(conduit::error::EngineError::NotFound(_))
    ));
}

#[tokio::test]
async fn complete_already_completed_task_returns_conflict() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("eng"),
        1,
        None,
        &linear_bpmn(),
        &json!({}),
    )
    .await
    .unwrap();
    let instance = engine
        .start_instance(def.id, org_id, &json!({}), &[])
        .await
        .unwrap();
    let task_id = db::tasks::list_by_instance(&pool, instance.id)
        .await
        .unwrap()[0]
        .id;

    engine.complete_user_task(task_id, &[]).await.unwrap();

    let result = engine.complete_user_task(task_id, &[]).await;
    assert!(matches!(
        result,
        Err(conduit::error::EngineError::Conflict(_))
    ));
}

// ─── cold cache (simulates restart) ─────────────────────────────────────────

#[tokio::test]
async fn engine_cold_cache_can_start_instance() {
    let (pool, _warm_engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    // Deploy a definition with one engine (warms the cache)
    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("eng"),
        1,
        None,
        &linear_bpmn(),
        &json!({}),
    )
    .await
    .unwrap();

    // Create a second engine with a completely empty cache (simulates a restart)
    let cold_cache: GraphCache = Arc::new(RwLock::new(HashMap::new()));
    let cold_engine = Engine::new(pool.clone(), cold_cache);

    let instance = cold_engine
        .start_instance(def.id, org_id, &json!({}), &[])
        .await
        .unwrap();

    assert_eq!(instance.state, "running");
}

// ─── Phase 6: variable passing ───────────────────────────────────────────────

fn var(name: &str, value_type: &str, value: serde_json::Value) -> VariableInput {
    VariableInput {
        name: name.to_string(),
        value_type: value_type.to_string(),
        value,
    }
}

#[tokio::test]
async fn complete_task_with_variables_writes_to_db() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("eng"),
        1,
        None,
        &linear_bpmn(),
        &json!({}),
    )
    .await
    .unwrap();
    let instance = engine
        .start_instance(def.id, org_id, &json!({}), &[])
        .await
        .unwrap();
    let task_id = db::tasks::list_by_instance(&pool, instance.id)
        .await
        .unwrap()[0]
        .id;

    engine
        .complete_user_task(
            task_id,
            &[
                var("approved", "boolean", json!(true)),
                var("amount", "integer", json!(2500)),
            ],
        )
        .await
        .unwrap();

    let vars = db::variables::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(vars.len(), 2);

    let approved = vars.iter().find(|v| v.name == "approved").unwrap();
    assert_eq!(approved.value_type, "boolean");
    assert_eq!(approved.value, json!(true));

    let amount = vars.iter().find(|v| v.name == "amount").unwrap();
    assert_eq!(amount.value_type, "integer");
    assert_eq!(amount.value, json!(2500));
}

// ─── Phase 6: exclusive gateway ──────────────────────────────────────────────

/// Start → UserTask → ExclusiveGateway → approved (amount > 1000) / rejected (default)
fn gateway_bpmn() -> String {
    r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def1" targetNamespace="urn:test">
  <process id="proc" isExecutable="true">
    <startEvent id="start"/>
    <userTask id="task1" name="Submit Request"/>
    <exclusiveGateway id="xgw1" default="sf_rejected"/>
    <endEvent id="approved"/>
    <endEvent id="rejected"/>
    <sequenceFlow id="sf1" sourceRef="start" targetRef="task1"/>
    <sequenceFlow id="sf2" sourceRef="task1" targetRef="xgw1"/>
    <sequenceFlow id="sf_approved" sourceRef="xgw1" targetRef="approved">
      <conditionExpression>amount > 1000</conditionExpression>
    </sequenceFlow>
    <sequenceFlow id="sf_rejected" sourceRef="xgw1" targetRef="rejected"/>
  </process>
</definitions>"#
        .to_string()
}

/// Start → UserTask → ExclusiveGateway → high (amount > 1000) / low (amount <= 1000), no default
fn gateway_no_default_bpmn() -> String {
    r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def1" targetNamespace="urn:test">
  <process id="proc" isExecutable="true">
    <startEvent id="start"/>
    <userTask id="task1" name="Submit Request"/>
    <exclusiveGateway id="xgw1"/>
    <endEvent id="high"/>
    <sequenceFlow id="sf1" sourceRef="start" targetRef="task1"/>
    <sequenceFlow id="sf2" sourceRef="task1" targetRef="xgw1"/>
    <sequenceFlow id="sf_high" sourceRef="xgw1" targetRef="high">
      <conditionExpression>amount > 1000</conditionExpression>
    </sequenceFlow>
  </process>
</definitions>"#
        .to_string()
}

#[tokio::test]
async fn gateway_routes_to_conditioned_flow_when_condition_true() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("eng"),
        1,
        None,
        &gateway_bpmn(),
        &json!({}),
    )
    .await
    .unwrap();
    let instance = engine
        .start_instance(def.id, org_id, &json!({}), &[])
        .await
        .unwrap();
    let task_id = db::tasks::list_by_instance(&pool, instance.id)
        .await
        .unwrap()[0]
        .id;

    // amount > 1000 → should follow sf_approved → end at "approved"
    engine
        .complete_user_task(task_id, &[var("amount", "integer", json!(5000))])
        .await
        .unwrap();

    let refreshed = db::process_instances::get_by_id(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(refreshed.state, "completed");

    let history = db::execution_history::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    let visited: Vec<_> = history.iter().map(|h| h.element_id.as_str()).collect();
    assert!(
        visited.contains(&"approved"),
        "expected 'approved' end event in history"
    );
    assert!(
        !visited.contains(&"rejected"),
        "expected 'rejected' end event NOT in history"
    );
}

#[tokio::test]
async fn gateway_routes_to_default_flow_when_no_condition_matches() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("eng"),
        1,
        None,
        &gateway_bpmn(),
        &json!({}),
    )
    .await
    .unwrap();
    let instance = engine
        .start_instance(def.id, org_id, &json!({}), &[])
        .await
        .unwrap();
    let task_id = db::tasks::list_by_instance(&pool, instance.id)
        .await
        .unwrap()[0]
        .id;

    // amount <= 1000 → condition false → falls through to default (sf_rejected)
    engine
        .complete_user_task(task_id, &[var("amount", "integer", json!(500))])
        .await
        .unwrap();

    let refreshed = db::process_instances::get_by_id(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(refreshed.state, "completed");

    let history = db::execution_history::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    let visited: Vec<_> = history.iter().map(|h| h.element_id.as_str()).collect();
    assert!(
        visited.contains(&"rejected"),
        "expected 'rejected' end event in history"
    );
    assert!(
        !visited.contains(&"approved"),
        "expected 'approved' end event NOT in history"
    );
}

#[tokio::test]
async fn gateway_no_match_no_default_marks_instance_error() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("eng"),
        1,
        None,
        &gateway_no_default_bpmn(),
        &json!({}),
    )
    .await
    .unwrap();
    let instance = engine
        .start_instance(def.id, org_id, &json!({}), &[])
        .await
        .unwrap();
    let task_id = db::tasks::list_by_instance(&pool, instance.id)
        .await
        .unwrap()[0]
        .id;

    // amount <= 1000 → condition false → no default → instance error
    engine
        .complete_user_task(task_id, &[var("amount", "integer", json!(500))])
        .await
        .unwrap();

    let refreshed = db::process_instances::get_by_id(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(refreshed.state, "error");
}

#[tokio::test]
async fn gateway_nested_routes_correctly() {
    // Two gateways in sequence: first splits on `tier`, second splits on `amount`.
    let bpmn = r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def1" targetNamespace="urn:test">
  <process id="proc" isExecutable="true">
    <startEvent id="start"/>
    <userTask id="task1" name="Submit"/>
    <exclusiveGateway id="xgw1" default="sf_standard"/>
    <exclusiveGateway id="xgw2" default="sf_low"/>
    <endEvent id="premium_high"/>
    <endEvent id="premium_low"/>
    <endEvent id="standard"/>
    <sequenceFlow id="sf1" sourceRef="start" targetRef="task1"/>
    <sequenceFlow id="sf2" sourceRef="task1" targetRef="xgw1"/>
    <sequenceFlow id="sf_premium" sourceRef="xgw1" targetRef="xgw2">
      <conditionExpression>tier == "premium"</conditionExpression>
    </sequenceFlow>
    <sequenceFlow id="sf_standard" sourceRef="xgw1" targetRef="standard"/>
    <sequenceFlow id="sf_high" sourceRef="xgw2" targetRef="premium_high">
      <conditionExpression>amount > 5000</conditionExpression>
    </sequenceFlow>
    <sequenceFlow id="sf_low" sourceRef="xgw2" targetRef="premium_low"/>
  </process>
</definitions>"#;

    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("eng"),
        1,
        None,
        bpmn,
        &json!({}),
    )
    .await
    .unwrap();

    let instance = engine
        .start_instance(def.id, org_id, &json!({}), &[])
        .await
        .unwrap();
    let task_id = db::tasks::list_by_instance(&pool, instance.id)
        .await
        .unwrap()[0]
        .id;

    // tier = "premium", amount = 8000 → xgw1 takes sf_premium → xgw2 takes sf_high → premium_high
    engine
        .complete_user_task(
            task_id,
            &[
                var("tier", "string", json!("premium")),
                var("amount", "integer", json!(8000)),
            ],
        )
        .await
        .unwrap();

    let refreshed = db::process_instances::get_by_id(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(refreshed.state, "completed");

    let history = db::execution_history::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    let visited: Vec<_> = history.iter().map(|h| h.element_id.as_str()).collect();
    assert!(visited.contains(&"premium_high"));
    assert!(!visited.contains(&"premium_low"));
    assert!(!visited.contains(&"standard"));
}
