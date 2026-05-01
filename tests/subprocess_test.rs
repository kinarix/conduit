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
    let slug = format!("sp-org-{}", Uuid::new_v4());
    let org = db::orgs::insert(pool, "Subprocess Test Org", &slug)
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

// ─── BPMN fixtures ────────────────────────────────────────────────────────────

/// Outer: Start → SubProcess(inner: Start → End) → End
fn simple_subprocess_bpmn() -> String {
    r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def1" targetNamespace="urn:test">
  <process id="proc" isExecutable="true">
    <startEvent id="outer-start"/>
    <subProcess id="sp1" name="My Subprocess">
      <startEvent id="inner-start"/>
      <endEvent id="inner-end"/>
      <sequenceFlow id="inner-sf1" sourceRef="inner-start" targetRef="inner-end"/>
    </subProcess>
    <endEvent id="outer-end"/>
    <sequenceFlow id="sf1" sourceRef="outer-start" targetRef="sp1"/>
    <sequenceFlow id="sf2" sourceRef="sp1" targetRef="outer-end"/>
  </process>
</definitions>"#
        .to_string()
}

/// Outer: Start → SubProcess(inner: Start → UserTask → End) → End
fn subprocess_with_user_task_bpmn() -> String {
    r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def1" targetNamespace="urn:test">
  <process id="proc" isExecutable="true">
    <startEvent id="outer-start"/>
    <subProcess id="sp1" name="Inner Work">
      <startEvent id="inner-start"/>
      <userTask id="inner-task" name="Inner Task"/>
      <endEvent id="inner-end"/>
      <sequenceFlow id="inner-sf1" sourceRef="inner-start" targetRef="inner-task"/>
      <sequenceFlow id="inner-sf2" sourceRef="inner-task" targetRef="inner-end"/>
    </subProcess>
    <endEvent id="outer-end"/>
    <sequenceFlow id="sf1" sourceRef="outer-start" targetRef="sp1"/>
    <sequenceFlow id="sf2" sourceRef="sp1" targetRef="outer-end"/>
  </process>
</definitions>"#
        .to_string()
}

/// Outer: Start → SubProcess(inner: Start → UserTask(writes result) → End) → End
/// Variable written inside subprocess should be readable after exit.
fn subprocess_writes_variable_bpmn() -> String {
    subprocess_with_user_task_bpmn()
}

/// Outer: Start(writes x=10) → SubProcess(inner: Start → ExclusiveGateway(x>5 → End-A, else End-B) → End-A/End-B) → End
fn subprocess_reads_parent_variable_bpmn() -> String {
    r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def1" targetNamespace="urn:test">
  <process id="proc" isExecutable="true">
    <startEvent id="outer-start"/>
    <subProcess id="sp1" name="Routing Subprocess">
      <startEvent id="inner-start"/>
      <exclusiveGateway id="inner-gw" default="inner-sf-default"/>
      <endEvent id="inner-end-a"/>
      <endEvent id="inner-end-b"/>
      <sequenceFlow id="inner-sf1" sourceRef="inner-start" targetRef="inner-gw"/>
      <sequenceFlow id="inner-sf-hot" sourceRef="inner-gw" targetRef="inner-end-a">
        <conditionExpression>x > 5</conditionExpression>
      </sequenceFlow>
      <sequenceFlow id="inner-sf-default" sourceRef="inner-gw" targetRef="inner-end-b"/>
    </subProcess>
    <endEvent id="outer-end"/>
    <sequenceFlow id="sf1" sourceRef="outer-start" targetRef="sp1"/>
    <sequenceFlow id="sf2" sourceRef="sp1" targetRef="outer-end"/>
  </process>
</definitions>"#
        .to_string()
}

/// Outer: Start → SubProcess(inner: Start → ExclusiveGateway → End) → End
fn subprocess_with_exclusive_gateway_bpmn() -> String {
    subprocess_reads_parent_variable_bpmn()
}

/// Outer: Start → SubProcess(inner: Start → SubProcess2(inner2: Start → End) → End) → End
fn nested_subprocess_bpmn() -> String {
    r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def1" targetNamespace="urn:test">
  <process id="proc" isExecutable="true">
    <startEvent id="outer-start"/>
    <subProcess id="sp1" name="Outer Subprocess">
      <startEvent id="inner-start"/>
      <subProcess id="sp2" name="Inner Subprocess">
        <startEvent id="inner2-start"/>
        <endEvent id="inner2-end"/>
        <sequenceFlow id="inner2-sf1" sourceRef="inner2-start" targetRef="inner2-end"/>
      </subProcess>
      <endEvent id="inner-end"/>
      <sequenceFlow id="inner-sf1" sourceRef="inner-start" targetRef="sp2"/>
      <sequenceFlow id="inner-sf2" sourceRef="sp2" targetRef="inner-end"/>
    </subProcess>
    <endEvent id="outer-end"/>
    <sequenceFlow id="sf1" sourceRef="outer-start" targetRef="sp1"/>
    <sequenceFlow id="sf2" sourceRef="sp1" targetRef="outer-end"/>
  </process>
</definitions>"#
        .to_string()
}

// ─── Tests ────────────────────────────────────────────────────────────────────

/// Token enters subprocess, runs inner start → inner end, then exits to outer end.
/// Instance should complete automatically.
#[tokio::test]
async fn subprocess_executes_inner_flow_before_parent_continues() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("sp"),
        1,
        None,
        &simple_subprocess_bpmn(),
        &json!({}),
    )
    .await
    .unwrap();

    let instance = engine
        .start_instance(def.id, org_id, &json!({}), &[])
        .await
        .unwrap();

    // Instance should complete since there are no wait states
    assert_eq!(instance.state, "completed", "instance should complete");

    // Inner elements should appear in execution_history
    let history: Vec<(String,)> = sqlx::query_as(
        "SELECT element_id FROM execution_history WHERE instance_id = $1 ORDER BY entered_at",
    )
    .bind(instance.id)
    .fetch_all(&pool)
    .await
    .unwrap();

    let visited: Vec<&str> = history.iter().map(|(id,)| id.as_str()).collect();
    assert!(
        visited.contains(&"inner-start"),
        "inner-start should be in history; got: {visited:?}"
    );
    assert!(
        visited.contains(&"inner-end"),
        "inner-end should be in history; got: {visited:?}"
    );
    assert!(
        visited.contains(&"outer-end"),
        "outer-end should be in history; got: {visited:?}"
    );
}

/// After subprocess runs, instance should be in 'completed' state.
#[tokio::test]
async fn subprocess_completes_instance_after_exit() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("sp"),
        1,
        None,
        &simple_subprocess_bpmn(),
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
}

/// Subprocess containing a UserTask: instance pauses, task is visible,
/// completing task advances inner flow then exits subprocess, completing instance.
#[tokio::test]
async fn subprocess_with_user_task_pauses_and_resumes() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("sp"),
        1,
        None,
        &subprocess_with_user_task_bpmn(),
        &json!({}),
    )
    .await
    .unwrap();

    let instance = engine
        .start_instance(def.id, org_id, &json!({}), &[])
        .await
        .unwrap();

    // Instance pauses at inner-task
    assert_eq!(
        instance.state, "running",
        "instance should be running (paused at inner task)"
    );

    // Inner task should be visible
    let tasks = db::tasks::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(tasks.len(), 1, "should have exactly 1 pending task");
    let task = &tasks[0];
    assert_eq!(task.element_id, "inner-task");
    assert_eq!(task.state, "pending");

    // Complete the inner task
    engine.complete_user_task(task.id, &[]).await.unwrap();

    // Instance should now be completed
    let updated: (String,) = sqlx::query_as("SELECT state FROM process_instances WHERE id = $1")
        .bind(instance.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        updated.0, "completed",
        "instance should complete after inner task done"
    );

    // Outer end should be in history
    let outer_end: Option<(String,)> = sqlx::query_as(
        "SELECT element_id FROM execution_history WHERE instance_id = $1 AND element_id = 'outer-end'",
    )
    .bind(instance.id)
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert!(
        outer_end.is_some(),
        "outer-end should appear in history after subprocess exits"
    );
}

/// Variable written inside subprocess (on task completion) is readable after subprocess exits.
#[tokio::test]
async fn subprocess_variables_visible_to_parent() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("sp"),
        1,
        None,
        &subprocess_writes_variable_bpmn(),
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
    let task = &tasks[0];

    engine
        .complete_user_task(
            task.id,
            &[VariableInput {
                name: "result".to_string(),
                value_type: "string".to_string(),
                value: json!("done"),
            }],
        )
        .await
        .unwrap();

    // Variable should be readable by instance_id (instance-scoped read)
    let var: Option<(serde_json::Value,)> = sqlx::query_as(
        "SELECT value FROM variables WHERE instance_id = $1 AND name = 'result' LIMIT 1",
    )
    .bind(instance.id)
    .fetch_optional(&pool)
    .await
    .unwrap();

    assert!(
        var.is_some(),
        "variable 'result' should be readable after subprocess exits"
    );
    assert_eq!(var.unwrap().0, json!("done"));
}

/// Parent (instance-scope) variable is visible to ExclusiveGateway conditions inside a subprocess.
#[tokio::test]
async fn parent_variables_visible_inside_subprocess() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("sp"),
        1,
        None,
        &subprocess_reads_parent_variable_bpmn(),
        &json!({}),
    )
    .await
    .unwrap();

    let instance = engine
        .start_instance(
            def.id,
            org_id,
            &json!({}),
            &[VariableInput {
                name: "x".to_string(),
                value_type: "integer".to_string(),
                value: json!(10),
            }],
        )
        .await
        .unwrap();

    let history: Vec<(String,)> =
        sqlx::query_as("SELECT element_id FROM execution_history WHERE instance_id = $1")
            .bind(instance.id)
            .fetch_all(&pool)
            .await
            .unwrap();
    let visited: Vec<&str> = history.iter().map(|(id,)| id.as_str()).collect();
    assert!(
        visited.contains(&"inner-end-a"),
        "x=10 → x>5 condition true → hot path inner-end-a; got: {visited:?}"
    );
    assert!(
        !visited.contains(&"inner-end-b"),
        "default path should NOT be taken when condition matches; got: {visited:?}"
    );
}

/// Sibling check: when the gateway condition references an unset variable, the
/// instance is marked error rather than silently routing to the default flow.
#[tokio::test]
async fn subprocess_gateway_with_undefined_variable_errors() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("sp"),
        1,
        None,
        &subprocess_reads_parent_variable_bpmn(),
        &json!({}),
    )
    .await
    .unwrap();

    let instance = engine
        .start_instance(def.id, org_id, &json!({}), &[])
        .await
        .unwrap();

    let refreshed = db::process_instances::get_by_id(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(
        refreshed.state, "error",
        "undefined variable in gateway condition must mark instance error, not silently default"
    );
}

/// Subprocess containing an exclusive gateway routes correctly based on variable.
#[tokio::test]
async fn subprocess_with_exclusive_gateway() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    // Use a process: Outer Start → UserTask(outer-task) → SubProcess(ExclusiveGateway) → End
    // outer-task sets x=10, subprocess routes x>5 → inner-end-a
    let bpmn = r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def1" targetNamespace="urn:test">
  <process id="proc" isExecutable="true">
    <startEvent id="outer-start"/>
    <userTask id="outer-task" name="Set Variables"/>
    <subProcess id="sp1" name="Routing">
      <startEvent id="inner-start"/>
      <exclusiveGateway id="inner-gw" default="inner-sf-default"/>
      <endEvent id="inner-end-a"/>
      <endEvent id="inner-end-b"/>
      <sequenceFlow id="inner-sf1" sourceRef="inner-start" targetRef="inner-gw"/>
      <sequenceFlow id="inner-sf-hot" sourceRef="inner-gw" targetRef="inner-end-a">
        <conditionExpression>x > 5</conditionExpression>
      </sequenceFlow>
      <sequenceFlow id="inner-sf-default" sourceRef="inner-gw" targetRef="inner-end-b"/>
    </subProcess>
    <endEvent id="outer-end"/>
    <sequenceFlow id="sf1" sourceRef="outer-start" targetRef="outer-task"/>
    <sequenceFlow id="sf2" sourceRef="outer-task" targetRef="sp1"/>
    <sequenceFlow id="sf3" sourceRef="sp1" targetRef="outer-end"/>
  </process>
</definitions>"#.to_string();

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("sp"),
        1,
        None,
        &bpmn,
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
    let task = &tasks[0];
    assert_eq!(task.element_id, "outer-task");

    // Complete outer task with x=10
    engine
        .complete_user_task(
            task.id,
            &[VariableInput {
                name: "x".to_string(),
                value_type: "integer".to_string(),
                value: json!(10),
            }],
        )
        .await
        .unwrap();

    let final_state: (String,) =
        sqlx::query_as("SELECT state FROM process_instances WHERE id = $1")
            .bind(instance.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(final_state.0, "completed");

    let history: Vec<(String,)> =
        sqlx::query_as("SELECT element_id FROM execution_history WHERE instance_id = $1")
            .bind(instance.id)
            .fetch_all(&pool)
            .await
            .unwrap();
    let visited: Vec<&str> = history.iter().map(|(id,)| id.as_str()).collect();
    assert!(
        visited.contains(&"inner-end-a"),
        "x>5 should route to inner-end-a; got: {visited:?}"
    );
    assert!(
        !visited.contains(&"inner-end-b"),
        "inner-end-b should NOT be visited; got: {visited:?}"
    );
}

/// Nested subprocess: subprocess containing another subprocess works end-to-end.
#[tokio::test]
async fn nested_subprocess() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("sp"),
        1,
        None,
        &nested_subprocess_bpmn(),
        &json!({}),
    )
    .await
    .unwrap();

    let instance = engine
        .start_instance(def.id, org_id, &json!({}), &[])
        .await
        .unwrap();

    assert_eq!(
        instance.state, "completed",
        "nested subprocess should complete instance"
    );

    let history: Vec<(String,)> =
        sqlx::query_as("SELECT element_id FROM execution_history WHERE instance_id = $1")
            .bind(instance.id)
            .fetch_all(&pool)
            .await
            .unwrap();
    let visited: Vec<&str> = history.iter().map(|(id,)| id.as_str()).collect();
    assert!(
        visited.contains(&"inner2-start"),
        "inner2-start should be visited; got: {visited:?}"
    );
    assert!(
        visited.contains(&"inner2-end"),
        "inner2-end should be visited; got: {visited:?}"
    );
    assert!(
        visited.contains(&"inner-end"),
        "inner-end should be visited; got: {visited:?}"
    );
    assert!(
        visited.contains(&"outer-end"),
        "outer-end should be visited; got: {visited:?}"
    );
}
