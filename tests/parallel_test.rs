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

async fn create_org(pool: &PgPool) -> Uuid {
    let slug = format!("par-org-{}", Uuid::new_v4());
    db::orgs::insert(pool, "Parallel Test Org", &slug)
        .await
        .unwrap()
        .id
}

fn unique_key(prefix: &str) -> String {
    format!("{}-{}", prefix, Uuid::new_v4())
}

fn var(name: &str, value_type: &str, value: serde_json::Value) -> VariableInput {
    VariableInput {
        name: name.to_string(),
        value_type: value_type.to_string(),
        value,
    }
}

/// Start → ParallelGW(fork) → TaskA → ParallelGW(join) → End
///                           → TaskB ↗
fn fork_join_bpmn() -> String {
    r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def1" targetNamespace="urn:test">
  <process id="proc" isExecutable="true">
    <startEvent id="start"/>
    <parallelGateway id="fork"/>
    <userTask id="taskA" name="Task A"/>
    <userTask id="taskB" name="Task B"/>
    <parallelGateway id="join"/>
    <endEvent id="end"/>
    <sequenceFlow id="sf1" sourceRef="start" targetRef="fork"/>
    <sequenceFlow id="sf2" sourceRef="fork" targetRef="taskA"/>
    <sequenceFlow id="sf3" sourceRef="fork" targetRef="taskB"/>
    <sequenceFlow id="sf4" sourceRef="taskA" targetRef="join"/>
    <sequenceFlow id="sf5" sourceRef="taskB" targetRef="join"/>
    <sequenceFlow id="sf6" sourceRef="join" targetRef="end"/>
  </process>
</definitions>"#
        .to_string()
}

/// Start → ParallelGW(fork) → TaskA → ParallelGW(join) → UserTask(post) → End
///                           → TaskB ↗
fn fork_join_then_task_bpmn() -> String {
    r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def1" targetNamespace="urn:test">
  <process id="proc" isExecutable="true">
    <startEvent id="start"/>
    <parallelGateway id="fork"/>
    <userTask id="taskA" name="Task A"/>
    <userTask id="taskB" name="Task B"/>
    <parallelGateway id="join"/>
    <userTask id="post" name="Post Join Task"/>
    <endEvent id="end"/>
    <sequenceFlow id="sf1" sourceRef="start" targetRef="fork"/>
    <sequenceFlow id="sf2" sourceRef="fork" targetRef="taskA"/>
    <sequenceFlow id="sf3" sourceRef="fork" targetRef="taskB"/>
    <sequenceFlow id="sf4" sourceRef="taskA" targetRef="join"/>
    <sequenceFlow id="sf5" sourceRef="taskB" targetRef="join"/>
    <sequenceFlow id="sf6" sourceRef="join" targetRef="post"/>
    <sequenceFlow id="sf7" sourceRef="post" targetRef="end"/>
  </process>
</definitions>"#
        .to_string()
}

// ─── Parser tests ─────────────────────────────────────────────────────────────

#[test]
fn parser_parallel_gateway_parses() {
    let result = conduit::parser::parse(&fork_join_bpmn());
    assert!(
        result.is_ok(),
        "expected parse to succeed: {:?}",
        result.err()
    );

    let graph = result.unwrap();
    let fork = graph.nodes.get("fork").expect("fork node missing");
    assert!(
        matches!(fork.kind, conduit::parser::FlowNodeKind::ParallelGateway),
        "fork should be ParallelGateway, got {:?}",
        fork.kind
    );
    let join = graph.nodes.get("join").expect("join node missing");
    assert!(
        matches!(join.kind, conduit::parser::FlowNodeKind::ParallelGateway),
        "join should be ParallelGateway, got {:?}",
        join.kind
    );
}

// ─── Engine: fork ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn fork_creates_two_pending_tasks() {
    let (pool, engine) = setup().await;
    let org_id = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        &unique_key("par"),
        1,
        None,
        &fork_join_bpmn(),
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
        2,
        "expected 2 tasks after fork, got {}",
        tasks.len()
    );

    let element_ids: Vec<&str> = tasks.iter().map(|t| t.element_id.as_str()).collect();
    assert!(element_ids.contains(&"taskA"), "taskA missing");
    assert!(element_ids.contains(&"taskB"), "taskB missing");
    assert!(
        tasks.iter().all(|t| t.state == "pending"),
        "all tasks should be pending"
    );
}

// ─── Engine: join ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn completing_one_parallel_task_does_not_complete_instance() {
    let (pool, engine) = setup().await;
    let org_id = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        &unique_key("par"),
        1,
        None,
        &fork_join_bpmn(),
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
    let task_a_id = tasks.iter().find(|t| t.element_id == "taskA").unwrap().id;

    engine.complete_user_task(task_a_id, &[]).await.unwrap();

    let refreshed = db::process_instances::get_by_id(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(
        refreshed.state, "running",
        "instance should still be running after only one branch completes"
    );
}

#[tokio::test]
async fn completing_both_parallel_tasks_completes_instance() {
    let (pool, engine) = setup().await;
    let org_id = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        &unique_key("par"),
        1,
        None,
        &fork_join_bpmn(),
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
    let task_a_id = tasks.iter().find(|t| t.element_id == "taskA").unwrap().id;
    let task_b_id = tasks.iter().find(|t| t.element_id == "taskB").unwrap().id;

    engine.complete_user_task(task_a_id, &[]).await.unwrap();
    engine.complete_user_task(task_b_id, &[]).await.unwrap();

    let refreshed = db::process_instances::get_by_id(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(refreshed.state, "completed");
    assert!(refreshed.ended_at.is_some());
}

#[tokio::test]
async fn completing_tasks_in_reverse_order_also_completes_instance() {
    let (pool, engine) = setup().await;
    let org_id = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        &unique_key("par"),
        1,
        None,
        &fork_join_bpmn(),
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
    let task_a_id = tasks.iter().find(|t| t.element_id == "taskA").unwrap().id;
    let task_b_id = tasks.iter().find(|t| t.element_id == "taskB").unwrap().id;

    // Complete B first, then A
    engine.complete_user_task(task_b_id, &[]).await.unwrap();
    engine.complete_user_task(task_a_id, &[]).await.unwrap();

    let refreshed = db::process_instances::get_by_id(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(refreshed.state, "completed");
}

// ─── Engine: variables ────────────────────────────────────────────────────────

#[tokio::test]
async fn parallel_branch_variables_are_merged_after_join() {
    let (pool, engine) = setup().await;
    let org_id = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        &unique_key("par"),
        1,
        None,
        &fork_join_bpmn(),
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
    let task_a_id = tasks.iter().find(|t| t.element_id == "taskA").unwrap().id;
    let task_b_id = tasks.iter().find(|t| t.element_id == "taskB").unwrap().id;

    engine
        .complete_user_task(task_a_id, &[var("result_a", "string", json!("done_a"))])
        .await
        .unwrap();
    engine
        .complete_user_task(task_b_id, &[var("result_b", "string", json!("done_b"))])
        .await
        .unwrap();

    let vars = db::variables::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    let names: Vec<&str> = vars.iter().map(|v| v.name.as_str()).collect();
    assert!(
        names.contains(&"result_a"),
        "result_a should be visible after join"
    );
    assert!(
        names.contains(&"result_b"),
        "result_b should be visible after join"
    );
}

// ─── Engine: post-join continuation ──────────────────────────────────────────

#[tokio::test]
async fn after_join_process_continues_to_next_task() {
    let (pool, engine) = setup().await;
    let org_id = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        &unique_key("par"),
        1,
        None,
        &fork_join_then_task_bpmn(),
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
    let task_a_id = tasks.iter().find(|t| t.element_id == "taskA").unwrap().id;
    let task_b_id = tasks.iter().find(|t| t.element_id == "taskB").unwrap().id;

    engine.complete_user_task(task_a_id, &[]).await.unwrap();
    engine.complete_user_task(task_b_id, &[]).await.unwrap();

    // After the join, the post-join task should be pending
    let refreshed = db::process_instances::get_by_id(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(refreshed.state, "running");

    let all_tasks = db::tasks::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    let post_task = all_tasks
        .iter()
        .find(|t| t.element_id == "post")
        .expect("post-join task should exist");
    assert_eq!(post_task.state, "pending");
}
