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
    let engine = Engine::new(pool.clone(), cache, [0xA5u8; 32]);
    (pool, engine)
}

async fn create_org(pool: &PgPool) -> (Uuid, Vec<Uuid>) {
    let slug = format!("igw-org-{}", Uuid::new_v4());
    let org = db::orgs::insert(pool, "Inclusive GW Test Org", &slug)
        .await
        .unwrap();
    let f1 = db::process_groups::insert(pool, org.id, "Primary")
        .await
        .unwrap();
    let f2 = db::process_groups::insert(pool, org.id, "Secondary")
        .await
        .unwrap();
    (org.id, vec![f1.id, f2.id])
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

// ─── BPMN fixtures ────────────────────────────────────────────────────────────

/// Start → setup(UserTask) → InclusiveGW(fork: x>0, y>0) → [task-a, task-b] → InclusiveGW(join) → End
fn two_condition_inclusive_bpmn() -> String {
    r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def1" targetNamespace="urn:test">
  <process id="proc" isExecutable="true">
    <startEvent id="start"/>
    <userTask id="setup" name="Set Variables"/>
    <inclusiveGateway id="igw-fork"/>
    <userTask id="task-a" name="Path A"/>
    <userTask id="task-b" name="Path B"/>
    <inclusiveGateway id="igw-join"/>
    <endEvent id="end"/>
    <sequenceFlow id="sf1" sourceRef="start" targetRef="setup"/>
    <sequenceFlow id="sf2" sourceRef="setup" targetRef="igw-fork"/>
    <sequenceFlow id="sf-a" sourceRef="igw-fork" targetRef="task-a">
      <conditionExpression>x &gt; 0</conditionExpression>
    </sequenceFlow>
    <sequenceFlow id="sf-b" sourceRef="igw-fork" targetRef="task-b">
      <conditionExpression>y &gt; 0</conditionExpression>
    </sequenceFlow>
    <sequenceFlow id="sf-a-join" sourceRef="task-a" targetRef="igw-join"/>
    <sequenceFlow id="sf-b-join" sourceRef="task-b" targetRef="igw-join"/>
    <sequenceFlow id="sf-out" sourceRef="igw-join" targetRef="end"/>
  </process>
</definitions>"#
        .to_string()
}

/// Start → setup(UserTask) → InclusiveGW(fork: x>0, y>0, z>0) → [task-a, task-b, task-c] → InclusiveGW(join) → End
fn three_condition_inclusive_bpmn() -> String {
    r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def1" targetNamespace="urn:test">
  <process id="proc" isExecutable="true">
    <startEvent id="start"/>
    <userTask id="setup" name="Set Variables"/>
    <inclusiveGateway id="igw-fork"/>
    <userTask id="task-a" name="Path A"/>
    <userTask id="task-b" name="Path B"/>
    <userTask id="task-c" name="Path C"/>
    <inclusiveGateway id="igw-join"/>
    <endEvent id="end"/>
    <sequenceFlow id="sf1" sourceRef="start" targetRef="setup"/>
    <sequenceFlow id="sf2" sourceRef="setup" targetRef="igw-fork"/>
    <sequenceFlow id="sf-a" sourceRef="igw-fork" targetRef="task-a">
      <conditionExpression>x &gt; 0</conditionExpression>
    </sequenceFlow>
    <sequenceFlow id="sf-b" sourceRef="igw-fork" targetRef="task-b">
      <conditionExpression>y &gt; 0</conditionExpression>
    </sequenceFlow>
    <sequenceFlow id="sf-c" sourceRef="igw-fork" targetRef="task-c">
      <conditionExpression>z &gt; 0</conditionExpression>
    </sequenceFlow>
    <sequenceFlow id="sf-a-join" sourceRef="task-a" targetRef="igw-join"/>
    <sequenceFlow id="sf-b-join" sourceRef="task-b" targetRef="igw-join"/>
    <sequenceFlow id="sf-c-join" sourceRef="task-c" targetRef="igw-join"/>
    <sequenceFlow id="sf-out" sourceRef="igw-join" targetRef="end"/>
  </process>
</definitions>"#
        .to_string()
}

/// Same two-condition process but with no default flow and conditions that won't match.
fn no_match_no_default_bpmn() -> String {
    r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def1" targetNamespace="urn:test">
  <process id="proc" isExecutable="true">
    <startEvent id="start"/>
    <userTask id="setup" name="Set Variables"/>
    <inclusiveGateway id="igw-fork"/>
    <userTask id="task-a" name="Path A"/>
    <userTask id="task-b" name="Path B"/>
    <inclusiveGateway id="igw-join"/>
    <endEvent id="end"/>
    <sequenceFlow id="sf1" sourceRef="start" targetRef="setup"/>
    <sequenceFlow id="sf2" sourceRef="setup" targetRef="igw-fork"/>
    <sequenceFlow id="sf-a" sourceRef="igw-fork" targetRef="task-a">
      <conditionExpression>x &gt; 100</conditionExpression>
    </sequenceFlow>
    <sequenceFlow id="sf-b" sourceRef="igw-fork" targetRef="task-b">
      <conditionExpression>y &gt; 100</conditionExpression>
    </sequenceFlow>
    <sequenceFlow id="sf-a-join" sourceRef="task-a" targetRef="igw-join"/>
    <sequenceFlow id="sf-b-join" sourceRef="task-b" targetRef="igw-join"/>
    <sequenceFlow id="sf-out" sourceRef="igw-join" targetRef="end"/>
  </process>
</definitions>"#
        .to_string()
}

// ─── Parser tests ─────────────────────────────────────────────────────────────

#[test]
fn parser_inclusive_gateway_parses() {
    let graph = conduit::parser::parse(&two_condition_inclusive_bpmn()).unwrap();
    let igw = graph.nodes.get("igw-fork").expect("igw-fork not found");
    assert!(
        matches!(
            &igw.kind,
            conduit::parser::FlowNodeKind::InclusiveGateway { .. }
        ),
        "igw-fork should be an InclusiveGateway"
    );
}

#[test]
fn parser_inclusive_gateway_reads_default_flow() {
    let bpmn = r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def1" targetNamespace="urn:test">
  <process id="proc" isExecutable="true">
    <startEvent id="start"/>
    <inclusiveGateway id="igw" default="sf-default"/>
    <endEvent id="end-a"/>
    <endEvent id="end-b"/>
    <sequenceFlow id="sf1" sourceRef="start" targetRef="igw"/>
    <sequenceFlow id="sf-hot" sourceRef="igw" targetRef="end-a">
      <conditionExpression>x &gt; 0</conditionExpression>
    </sequenceFlow>
    <sequenceFlow id="sf-default" sourceRef="igw" targetRef="end-b"/>
  </process>
</definitions>"#;

    let graph = conduit::parser::parse(bpmn).unwrap();
    let igw = graph.nodes.get("igw").expect("igw not found");
    match &igw.kind {
        conduit::parser::FlowNodeKind::InclusiveGateway { default_flow } => {
            assert_eq!(
                default_flow.as_deref(),
                Some("sf-default"),
                "default_flow should be 'sf-default'"
            );
        }
        _ => panic!("Expected InclusiveGateway"),
    }
}

// ─── Engine tests ─────────────────────────────────────────────────────────────

/// All conditions true → both paths run, instance waits for both tasks.
/// Completing both tasks completes the instance.
#[tokio::test]
async fn all_conditions_true_activates_all_paths() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("igw"),
        1,
        None,
        &two_condition_inclusive_bpmn(),
        &json!({}),
    )
    .await
    .unwrap();

    let instance = engine
        .start_instance(def.id, org_id, &json!({}), &[])
        .await
        .unwrap();
    assert_eq!(instance.state, "running");

    // Complete setup task with x=5, y=5 → both conditions true
    let tasks = db::tasks::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(tasks.len(), 1);
    let setup_task = tasks.iter().find(|t| t.element_id == "setup").unwrap();

    engine
        .complete_user_task(
            setup_task.id,
            &[var("x", "integer", json!(5)), var("y", "integer", json!(5))],
        )
        .await
        .unwrap();

    // Both paths should now be active — two tasks pending
    let tasks = db::tasks::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    let pending: Vec<_> = tasks.iter().filter(|t| t.state == "pending").collect();
    assert_eq!(
        pending.len(),
        2,
        "both paths should be active; got tasks: {:?}",
        pending.iter().map(|t| &t.element_id).collect::<Vec<_>>()
    );

    let has_a = pending.iter().any(|t| t.element_id == "task-a");
    let has_b = pending.iter().any(|t| t.element_id == "task-b");
    assert!(has_a, "task-a should be pending");
    assert!(has_b, "task-b should be pending");

    // Complete task-a — instance still running (task-b not done)
    let task_a = pending.iter().find(|t| t.element_id == "task-a").unwrap();
    engine.complete_user_task(task_a.id, &[]).await.unwrap();

    let state: (String,) = sqlx::query_as("SELECT state FROM process_instances WHERE id = $1")
        .bind(instance.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        state.0, "running",
        "instance should still be running after completing only task-a"
    );

    // Complete task-b — join fires, instance completes
    let task_b = pending.iter().find(|t| t.element_id == "task-b").unwrap();
    engine.complete_user_task(task_b.id, &[]).await.unwrap();

    let state: (String,) = sqlx::query_as("SELECT state FROM process_instances WHERE id = $1")
        .bind(instance.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        state.0, "completed",
        "instance should complete after all active paths finish"
    );
}

/// One condition true → only that path runs.
/// Join gateway sees expected_count=1 and advances as soon as the one task completes.
#[tokio::test]
async fn single_condition_true_activates_one_path() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("igw"),
        1,
        None,
        &two_condition_inclusive_bpmn(),
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
    let setup_task = tasks.iter().find(|t| t.element_id == "setup").unwrap();

    // x=5 matches, y=-1 does not → only path A activated
    engine
        .complete_user_task(
            setup_task.id,
            &[
                var("x", "integer", json!(5)),
                var("y", "integer", json!(-1)),
            ],
        )
        .await
        .unwrap();

    let tasks = db::tasks::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    let pending: Vec<_> = tasks.iter().filter(|t| t.state == "pending").collect();
    assert_eq!(
        pending.len(),
        1,
        "only one path should be active; got: {:?}",
        pending.iter().map(|t| &t.element_id).collect::<Vec<_>>()
    );
    assert_eq!(
        pending[0].element_id, "task-a",
        "task-a should be the active task"
    );

    // Completing the single active task should complete the instance immediately
    engine.complete_user_task(pending[0].id, &[]).await.unwrap();

    let state: (String,) = sqlx::query_as("SELECT state FROM process_instances WHERE id = $1")
        .bind(instance.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        state.0, "completed",
        "instance should complete when the only active path finishes"
    );
}

/// Two of three conditions true → exactly two paths run.
/// Join waits for those two, not for the third inactive path.
#[tokio::test]
async fn two_of_three_conditions_join_waits_for_both() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("igw"),
        1,
        None,
        &three_condition_inclusive_bpmn(),
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
    let setup_task = tasks.iter().find(|t| t.element_id == "setup").unwrap();

    // x=5 and y=5 match; z=-1 does not → paths A and B activated, path C skipped
    engine
        .complete_user_task(
            setup_task.id,
            &[
                var("x", "integer", json!(5)),
                var("y", "integer", json!(5)),
                var("z", "integer", json!(-1)),
            ],
        )
        .await
        .unwrap();

    let tasks = db::tasks::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    let pending: Vec<_> = tasks.iter().filter(|t| t.state == "pending").collect();
    assert_eq!(
        pending.len(),
        2,
        "exactly two paths should be active; got: {:?}",
        pending.iter().map(|t| &t.element_id).collect::<Vec<_>>()
    );

    let has_a = pending.iter().any(|t| t.element_id == "task-a");
    let has_b = pending.iter().any(|t| t.element_id == "task-b");
    let has_c = pending.iter().any(|t| t.element_id == "task-c");
    assert!(has_a, "task-a should be active");
    assert!(has_b, "task-b should be active");
    assert!(
        !has_c,
        "task-c should NOT be active (z condition was false)"
    );

    // Complete task-a — still waiting for task-b
    let task_a = pending.iter().find(|t| t.element_id == "task-a").unwrap();
    engine.complete_user_task(task_a.id, &[]).await.unwrap();

    let state: (String,) = sqlx::query_as("SELECT state FROM process_instances WHERE id = $1")
        .bind(instance.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        state.0, "running",
        "instance should still be running after completing only task-a of two active paths"
    );

    // Complete task-b — join fires (2 of 2 arrived), instance completes
    let task_b = pending.iter().find(|t| t.element_id == "task-b").unwrap();
    engine.complete_user_task(task_b.id, &[]).await.unwrap();

    let state: (String,) = sqlx::query_as("SELECT state FROM process_instances WHERE id = $1")
        .bind(instance.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        state.0, "completed",
        "instance should complete after both active paths (A and B) finish"
    );
}

/// No conditions true and no default flow → instance enters error state.
#[tokio::test]
async fn no_matching_condition_no_default_errors_instance() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("igw"),
        1,
        None,
        &no_match_no_default_bpmn(),
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
    let setup_task = tasks.iter().find(|t| t.element_id == "setup").unwrap();

    // x=1, y=1 — neither satisfies x>100 or y>100
    engine
        .complete_user_task(
            setup_task.id,
            &[var("x", "integer", json!(1)), var("y", "integer", json!(1))],
        )
        .await
        .unwrap();

    let state: (String,) = sqlx::query_as("SELECT state FROM process_instances WHERE id = $1")
        .bind(instance.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        state.0, "error",
        "instance should enter error state when no inclusive gateway condition matches"
    );
}

/// Bad expression on an inclusive gateway flow → instance error; default not silently taken.
fn inclusive_broken_expression_bpmn() -> String {
    r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def1" targetNamespace="urn:test">
  <process id="proc" isExecutable="true">
    <startEvent id="start"/>
    <userTask id="setup" name="Set Variables"/>
    <inclusiveGateway id="igw-fork" default="sf-default"/>
    <userTask id="task-a" name="Path A"/>
    <userTask id="task-default" name="Default Path"/>
    <inclusiveGateway id="igw-join"/>
    <endEvent id="end"/>
    <sequenceFlow id="sf1" sourceRef="start" targetRef="setup"/>
    <sequenceFlow id="sf2" sourceRef="setup" targetRef="igw-fork"/>
    <sequenceFlow id="sf-broken" sourceRef="igw-fork" targetRef="task-a">
      <conditionExpression>x @@@ broken</conditionExpression>
    </sequenceFlow>
    <sequenceFlow id="sf-default" sourceRef="igw-fork" targetRef="task-default"/>
    <sequenceFlow id="sf-a-join" sourceRef="task-a" targetRef="igw-join"/>
    <sequenceFlow id="sf-default-join" sourceRef="task-default" targetRef="igw-join"/>
    <sequenceFlow id="sf-out" sourceRef="igw-join" targetRef="end"/>
  </process>
</definitions>"#
        .to_string()
}

#[tokio::test]
async fn inclusive_gateway_eval_error_marks_instance_error_does_not_take_default() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("igw"),
        1,
        None,
        &inclusive_broken_expression_bpmn(),
        &json!({}),
    )
    .await
    .unwrap();
    let instance = engine
        .start_instance(def.id, org_id, &json!({}), &[])
        .await
        .unwrap();
    let setup_task = db::tasks::list_by_instance(&pool, instance.id)
        .await
        .unwrap()
        .into_iter()
        .find(|t| t.element_id == "setup")
        .expect("setup task exists");

    engine
        .complete_user_task(setup_task.id, &[var("x", "integer", json!(1))])
        .await
        .unwrap();

    let state: (String,) = sqlx::query_as("SELECT state FROM process_instances WHERE id = $1")
        .bind(instance.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        state.0, "error",
        "broken inclusive condition must mark instance error, not silently take default"
    );

    // No downstream task should have been created
    let task_default_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM tasks WHERE instance_id = $1 AND element_id = 'task-default')",
    )
    .bind(instance.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        !task_default_exists,
        "default flow must NOT be taken when a non-default condition errors"
    );
}
