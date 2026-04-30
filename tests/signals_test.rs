use conduit::db;
use conduit::engine::{Engine, VariableInput};
use conduit::parser::{self, FlowNodeKind};
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
    let slug = format!("sig-org-{}", Uuid::new_v4());
    let org = db::orgs::insert(pool, "Signal Test Org", &slug)
        .await
        .unwrap();
    let f1 = db::process_groups::insert(pool, org.id, "Primary").await.unwrap();
    let f2 = db::process_groups::insert(pool, org.id, "Secondary").await.unwrap();
    (org.id, vec![f1.id, f2.id])
}

fn unique_key(prefix: &str) -> String {
    format!("{}-{}", prefix, Uuid::new_v4())
}

// ─── BPMN fixtures ────────────────────────────────────────────────────────────

/// Start → IntermediateSignalCatchEvent(signal "OrderReady") → End
fn signal_catch_bpmn() -> String {
    r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def1" targetNamespace="urn:test">
  <signal id="sig-order-ready" name="OrderReady"/>
  <process id="proc" isExecutable="true">
    <startEvent id="start"/>
    <intermediateCatchEvent id="wait-sig" name="Wait for Signal">
      <signalEventDefinition signalRef="sig-order-ready"/>
    </intermediateCatchEvent>
    <endEvent id="end"/>
    <sequenceFlow id="sf1" sourceRef="start" targetRef="wait-sig"/>
    <sequenceFlow id="sf2" sourceRef="wait-sig" targetRef="end"/>
  </process>
</definitions>"#
    .to_string()
}

/// SignalStartEvent → UserTask → End (started by incoming signal)
fn signal_start_bpmn() -> String {
    r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def1" targetNamespace="urn:test">
  <signal id="sig-launch" name="LaunchProcess"/>
  <process id="proc" isExecutable="true">
    <startEvent id="start">
      <signalEventDefinition signalRef="sig-launch"/>
    </startEvent>
    <userTask id="task1" name="Do Work"/>
    <endEvent id="end"/>
    <sequenceFlow id="sf1" sourceRef="start" targetRef="task1"/>
    <sequenceFlow id="sf2" sourceRef="task1" targetRef="end"/>
  </process>
</definitions>"#
    .to_string()
}

/// Start → UserTask (with interrupting BoundarySignalEvent) → End
///                             ↓ (boundary path)
///                        End2
fn boundary_signal_interrupting_bpmn() -> String {
    r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def1" targetNamespace="urn:test">
  <signal id="sig-cancel" name="CancelOrder"/>
  <process id="proc" isExecutable="true">
    <startEvent id="start"/>
    <userTask id="task1" name="Process Order"/>
    <boundaryEvent id="boundary-sig" attachedToRef="task1" cancelActivity="true">
      <signalEventDefinition signalRef="sig-cancel"/>
    </boundaryEvent>
    <endEvent id="end"/>
    <endEvent id="end-cancel"/>
    <sequenceFlow id="sf1" sourceRef="start" targetRef="task1"/>
    <sequenceFlow id="sf2" sourceRef="task1" targetRef="end"/>
    <sequenceFlow id="sf3" sourceRef="boundary-sig" targetRef="end-cancel"/>
  </process>
</definitions>"#
    .to_string()
}

/// Start → UserTask (with non-interrupting BoundarySignalEvent) → End
fn boundary_signal_non_interrupting_bpmn() -> String {
    r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def1" targetNamespace="urn:test">
  <signal id="sig-notify" name="NotifyWorker"/>
  <process id="proc" isExecutable="true">
    <startEvent id="start"/>
    <userTask id="task1" name="Do Work"/>
    <boundaryEvent id="boundary-sig" attachedToRef="task1" cancelActivity="false">
      <signalEventDefinition signalRef="sig-notify"/>
    </boundaryEvent>
    <endEvent id="end"/>
    <endEvent id="end-notify"/>
    <sequenceFlow id="sf1" sourceRef="start" targetRef="task1"/>
    <sequenceFlow id="sf2" sourceRef="task1" targetRef="end"/>
    <sequenceFlow id="sf3" sourceRef="boundary-sig" targetRef="end-notify"/>
  </process>
</definitions>"#
    .to_string()
}

/// Two parallel processes both waiting for "StockAvailable"
fn two_catchers_bpmn(suffix: &str) -> String {
    format!(
        r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def{suffix}" targetNamespace="urn:test">
  <signal id="sig-stock" name="StockAvailable"/>
  <process id="proc{suffix}" isExecutable="true">
    <startEvent id="start{suffix}"/>
    <intermediateCatchEvent id="wait-sig{suffix}">
      <signalEventDefinition signalRef="sig-stock"/>
    </intermediateCatchEvent>
    <endEvent id="end{suffix}"/>
    <sequenceFlow id="sf1{suffix}" sourceRef="start{suffix}" targetRef="wait-sig{suffix}"/>
    <sequenceFlow id="sf2{suffix}" sourceRef="wait-sig{suffix}" targetRef="end{suffix}"/>
  </process>
</definitions>"#
    )
}

// ─── Parser tests ─────────────────────────────────────────────────────────────

#[test]
fn parse_intermediate_signal_catch_event() {
    let graph = parser::parse(&signal_catch_bpmn()).unwrap();
    let node = graph.nodes.get("wait-sig").unwrap();
    match &node.kind {
        FlowNodeKind::IntermediateSignalCatchEvent { signal_name } => {
            assert_eq!(signal_name, "OrderReady");
        }
        _ => panic!("expected IntermediateSignalCatchEvent, got {:?}", node.kind),
    }
}

#[test]
fn parse_signal_start_event() {
    let graph = parser::parse(&signal_start_bpmn()).unwrap();
    let node = graph.nodes.get("start").unwrap();
    match &node.kind {
        FlowNodeKind::SignalStartEvent { signal_name } => {
            assert_eq!(signal_name, "LaunchProcess");
        }
        _ => panic!("expected SignalStartEvent, got {:?}", node.kind),
    }
}

#[test]
fn parse_boundary_signal_event_interrupting() {
    let graph = parser::parse(&boundary_signal_interrupting_bpmn()).unwrap();
    let node = graph.nodes.get("boundary-sig").unwrap();
    match &node.kind {
        FlowNodeKind::BoundarySignalEvent {
            signal_name,
            attached_to,
            cancelling,
        } => {
            assert_eq!(signal_name, "CancelOrder");
            assert_eq!(attached_to, "task1");
            assert!(*cancelling, "expected cancelling=true");
        }
        _ => panic!("expected BoundarySignalEvent, got {:?}", node.kind),
    }
    // Boundary should be registered in attached_to map
    let attached = graph.attached_to.get("task1").unwrap();
    assert!(attached.contains(&"boundary-sig".to_string()));
}

#[test]
fn parse_boundary_signal_event_non_interrupting() {
    let graph = parser::parse(&boundary_signal_non_interrupting_bpmn()).unwrap();
    let node = graph.nodes.get("boundary-sig").unwrap();
    match &node.kind {
        FlowNodeKind::BoundarySignalEvent { cancelling, .. } => {
            assert!(!*cancelling, "expected cancelling=false");
        }
        _ => panic!("expected BoundarySignalEvent, got {:?}", node.kind),
    }
}

// ─── Engine tests ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn signal_catch_pauses_instance_and_creates_subscription() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("sig-catch"),
        1,
        None,
        &signal_catch_bpmn(),
        &json!({}),
    )
    .await
    .unwrap();

    let instance = engine
        .start_instance(def.id, org_id, &json!({}), &[])
        .await
        .unwrap();

    assert_eq!(instance.state, "running");

    let subs = db::event_subscriptions::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(subs.len(), 1);
    assert_eq!(subs[0].event_type, "signal");
    assert_eq!(subs[0].event_name, "OrderReady");
}

#[tokio::test]
async fn broadcast_signal_advances_waiting_instance_to_end() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("sig-catch"),
        1,
        None,
        &signal_catch_bpmn(),
        &json!({}),
    )
    .await
    .unwrap();

    let instance = engine
        .start_instance(def.id, org_id, &json!({}), &[])
        .await
        .unwrap();

    engine
        .broadcast_signal("OrderReady", &[], org_id)
        .await
        .unwrap();

    let refreshed = db::process_instances::get_by_id(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(refreshed.state, "completed");

    let subs = db::event_subscriptions::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    assert!(subs.is_empty(), "subscription should be deleted");
}

#[tokio::test]
async fn broadcast_signal_passes_variables() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("sig-vars"),
        1,
        None,
        &signal_catch_bpmn(),
        &json!({}),
    )
    .await
    .unwrap();

    let instance = engine
        .start_instance(def.id, org_id, &json!({}), &[])
        .await
        .unwrap();

    let vars = vec![VariableInput {
        name: "qty".to_string(),
        value_type: "integer".to_string(),
        value: json!(42),
    }];
    engine
        .broadcast_signal("OrderReady", &vars, org_id)
        .await
        .unwrap();

    let variables: Vec<conduit::db::models::Variable> =
        sqlx::query_as("SELECT * FROM variables WHERE instance_id = $1")
            .bind(instance.id)
            .fetch_all(&pool)
            .await
            .unwrap();
    let qty = variables.iter().find(|v| v.name == "qty");
    assert!(qty.is_some(), "qty variable should exist");
    assert_eq!(qty.unwrap().value, json!(42));
}

#[tokio::test]
async fn broadcast_signal_reaches_all_waiting_instances() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    // Deploy two separate processes both waiting on the same signal
    let def_a = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("sig-multi-a"),
        1,
        None,
        &two_catchers_bpmn("A"),
        &json!({}),
    )
    .await
    .unwrap();

    let def_b = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("sig-multi-b"),
        1,
        None,
        &two_catchers_bpmn("B"),
        &json!({}),
    )
    .await
    .unwrap();

    let inst_a = engine
        .start_instance(def_a.id, org_id, &json!({}), &[])
        .await
        .unwrap();
    let inst_b = engine
        .start_instance(def_b.id, org_id, &json!({}), &[])
        .await
        .unwrap();

    engine
        .broadcast_signal("StockAvailable", &[], org_id)
        .await
        .unwrap();

    let a = db::process_instances::get_by_id(&pool, inst_a.id)
        .await
        .unwrap();
    let b = db::process_instances::get_by_id(&pool, inst_b.id)
        .await
        .unwrap();
    assert_eq!(a.state, "completed", "instance A should be completed");
    assert_eq!(b.state, "completed", "instance B should be completed");
}

#[tokio::test]
async fn broadcast_signal_with_no_listeners_succeeds() {
    let (pool, engine) = setup().await;
    let (org_id, _groups) = create_org(&pool).await;

    // No instances running — broadcast should still return Ok (unlike correlate_message)
    engine
        .broadcast_signal("GhostSignal", &[], org_id)
        .await
        .unwrap();
}

#[tokio::test]
async fn signal_start_event_creates_new_instance() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("sig-start"),
        1,
        None,
        &signal_start_bpmn(),
        &json!({}),
    )
    .await
    .unwrap();

    // No instances yet
    let initial: Vec<conduit::db::models::ProcessInstance> =
        sqlx::query_as("SELECT * FROM process_instances WHERE definition_id = $1")
            .bind(def.id)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert!(initial.is_empty());

    engine
        .broadcast_signal("LaunchProcess", &[], org_id)
        .await
        .unwrap();

    let instances: Vec<conduit::db::models::ProcessInstance> =
        sqlx::query_as("SELECT * FROM process_instances WHERE definition_id = $1")
            .bind(def.id)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(instances.len(), 1);
    assert_eq!(instances[0].state, "running");

    let tasks = db::tasks::list_by_instance(&pool, instances[0].id)
        .await
        .unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].element_id, "task1");
}

#[tokio::test]
async fn boundary_signal_interrupting_cancels_task() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("sig-boundary-int"),
        1,
        None,
        &boundary_signal_interrupting_bpmn(),
        &json!({}),
    )
    .await
    .unwrap();

    let instance = engine
        .start_instance(def.id, org_id, &json!({}), &[])
        .await
        .unwrap();

    // A signal subscription should be set up for the boundary event
    let subs = db::event_subscriptions::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(subs.len(), 1);
    assert_eq!(subs[0].event_name, "CancelOrder");

    // Task should be pending
    let tasks = db::tasks::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].state, "pending");

    engine
        .broadcast_signal("CancelOrder", &[], org_id)
        .await
        .unwrap();

    // Task should be cancelled
    let tasks_after = db::tasks::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(tasks_after[0].state, "cancelled");

    // Instance should be completed (boundary path → end-cancel)
    let refreshed = db::process_instances::get_by_id(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(refreshed.state, "completed");

    // Subscription should be gone
    let subs_after = db::event_subscriptions::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    assert!(subs_after.is_empty());
}

#[tokio::test]
async fn boundary_signal_non_interrupting_keeps_task() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("sig-boundary-non"),
        1,
        None,
        &boundary_signal_non_interrupting_bpmn(),
        &json!({}),
    )
    .await
    .unwrap();

    let instance = engine
        .start_instance(def.id, org_id, &json!({}), &[])
        .await
        .unwrap();

    engine
        .broadcast_signal("NotifyWorker", &[], org_id)
        .await
        .unwrap();

    // Task should still be pending (non-interrupting)
    let tasks = db::tasks::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(tasks[0].state, "pending");

    // Instance should still be running (waiting at the user task)
    let refreshed = db::process_instances::get_by_id(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(refreshed.state, "running");
}

#[tokio::test]
async fn normal_task_completion_cleans_up_signal_subscription() {
    let (pool, engine) = setup().await;
    let (org_id, groups) = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("sig-cleanup"),
        1,
        None,
        &boundary_signal_interrupting_bpmn(),
        &json!({}),
    )
    .await
    .unwrap();

    let instance = engine
        .start_instance(def.id, org_id, &json!({}), &[])
        .await
        .unwrap();

    let subs = db::event_subscriptions::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(subs.len(), 1);

    // Complete the task normally (not via the signal)
    let tasks = db::tasks::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    engine.complete_user_task(tasks[0].id, &[]).await.unwrap();

    // Signal subscription should be deleted
    let subs_after = db::event_subscriptions::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    assert!(
        subs_after.is_empty(),
        "signal subscription should be removed on normal task completion"
    );

    let refreshed = db::process_instances::get_by_id(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(refreshed.state, "completed");
}
