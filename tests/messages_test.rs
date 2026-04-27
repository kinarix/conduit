use conduit::db;
use conduit::engine::{Engine, VariableInput};
use conduit::error::EngineError;
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

async fn create_org(pool: &PgPool) -> Uuid {
    let slug = format!("msg-org-{}", Uuid::new_v4());
    db::orgs::insert(pool, "Message Test Org", &slug)
        .await
        .unwrap()
        .id
}

fn unique_key(prefix: &str) -> String {
    format!("{}-{}", prefix, Uuid::new_v4())
}

// ─── BPMN fixtures ────────────────────────────────────────────────────────────

/// Start → IntermediateCatchEvent(message "OrderShipped") → End
fn message_catch_bpmn() -> String {
    r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def1" targetNamespace="urn:test">
  <message id="msg-order-shipped" name="OrderShipped"/>
  <process id="proc" isExecutable="true">
    <startEvent id="start"/>
    <intermediateCatchEvent id="wait-msg" name="Wait for Shipment">
      <messageEventDefinition messageRef="msg-order-shipped"/>
    </intermediateCatchEvent>
    <endEvent id="end"/>
    <sequenceFlow id="sf1" sourceRef="start" targetRef="wait-msg"/>
    <sequenceFlow id="sf2" sourceRef="wait-msg" targetRef="end"/>
  </process>
</definitions>"#
    .to_string()
}

/// Same but uses correlation key via ${orderId} variable reference
fn message_catch_correlated_bpmn() -> String {
    r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def1" targetNamespace="urn:test">
  <message id="msg-order-shipped" name="OrderShipped"/>
  <process id="proc" isExecutable="true">
    <startEvent id="start"/>
    <intermediateCatchEvent id="wait-msg" name="Wait for Shipment" correlationKey="${orderId}">
      <messageEventDefinition messageRef="msg-order-shipped"/>
    </intermediateCatchEvent>
    <endEvent id="end"/>
    <sequenceFlow id="sf1" sourceRef="start" targetRef="wait-msg"/>
    <sequenceFlow id="sf2" sourceRef="wait-msg" targetRef="end"/>
  </process>
</definitions>"#
    .to_string()
}

/// MessageStartEvent → UserTask → End (started by incoming message, not REST /instances)
fn message_start_bpmn() -> String {
    r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def1" targetNamespace="urn:test">
  <message id="msg-order-placed" name="OrderPlaced"/>
  <process id="proc" isExecutable="true">
    <startEvent id="start">
      <messageEventDefinition messageRef="msg-order-placed"/>
    </startEvent>
    <userTask id="task1" name="Process Order"/>
    <endEvent id="end"/>
    <sequenceFlow id="sf1" sourceRef="start" targetRef="task1"/>
    <sequenceFlow id="sf2" sourceRef="task1" targetRef="end"/>
  </process>
</definitions>"#
    .to_string()
}

/// Start → ReceiveTask(message "PaymentConfirmed") → End
fn receive_task_bpmn() -> String {
    r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def1" targetNamespace="urn:test">
  <message id="msg-payment" name="PaymentConfirmed"/>
  <process id="proc" isExecutable="true">
    <startEvent id="start"/>
    <receiveTask id="rt1" name="Wait for Payment">
      <messageEventDefinition messageRef="msg-payment"/>
    </receiveTask>
    <endEvent id="end"/>
    <sequenceFlow id="sf1" sourceRef="start" targetRef="rt1"/>
    <sequenceFlow id="sf2" sourceRef="rt1" targetRef="end"/>
  </process>
</definitions>"#
    .to_string()
}

// ─── Parser tests ─────────────────────────────────────────────────────────────

#[test]
fn parse_intermediate_message_catch_event() {
    let graph = parser::parse(&message_catch_bpmn()).unwrap();
    let node = graph.nodes.get("wait-msg").unwrap();
    match &node.kind {
        FlowNodeKind::IntermediateMessageCatchEvent {
            message_name,
            correlation_key_expr,
        } => {
            assert_eq!(message_name, "OrderShipped");
            assert!(correlation_key_expr.is_none());
        }
        _ => panic!(
            "expected IntermediateMessageCatchEvent, got {:?}",
            node.kind
        ),
    }
}

#[test]
fn parse_intermediate_message_catch_event_with_correlation_key() {
    let graph = parser::parse(&message_catch_correlated_bpmn()).unwrap();
    let node = graph.nodes.get("wait-msg").unwrap();
    match &node.kind {
        FlowNodeKind::IntermediateMessageCatchEvent {
            message_name,
            correlation_key_expr,
        } => {
            assert_eq!(message_name, "OrderShipped");
            assert_eq!(correlation_key_expr.as_deref(), Some("${orderId}"));
        }
        _ => panic!(
            "expected IntermediateMessageCatchEvent, got {:?}",
            node.kind
        ),
    }
}

#[test]
fn parse_message_start_event() {
    let graph = parser::parse(&message_start_bpmn()).unwrap();
    let node = graph.nodes.get("start").unwrap();
    match &node.kind {
        FlowNodeKind::MessageStartEvent { message_name } => {
            assert_eq!(message_name, "OrderPlaced");
        }
        _ => panic!("expected MessageStartEvent, got {:?}", node.kind),
    }
}

#[test]
fn parse_receive_task() {
    let graph = parser::parse(&receive_task_bpmn()).unwrap();
    let node = graph.nodes.get("rt1").unwrap();
    match &node.kind {
        FlowNodeKind::ReceiveTask {
            message_name,
            correlation_key_expr,
        } => {
            assert_eq!(message_name, "PaymentConfirmed");
            assert!(correlation_key_expr.is_none());
        }
        _ => panic!("expected ReceiveTask, got {:?}", node.kind),
    }
}

// ─── Engine: message wait → advance ──────────────────────────────────────────

#[tokio::test]
async fn message_catch_event_pauses_instance_and_creates_subscription() {
    let (pool, engine) = setup().await;
    let org_id = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        &unique_key("msg-catch"),
        1,
        None,
        &message_catch_bpmn(),
        &json!({}),
    )
    .await
    .unwrap();

    let instance = engine
        .start_instance(def.id, org_id, &json!({}))
        .await
        .unwrap();

    assert_eq!(instance.state, "running");

    // An event subscription should exist
    let subs = db::event_subscriptions::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(subs.len(), 1);
    assert_eq!(subs[0].event_type, "message");
    assert_eq!(subs[0].event_name, "OrderShipped");
    assert!(subs[0].correlation_key.is_none());
}

#[tokio::test]
async fn correlate_message_advances_waiting_instance_to_end() {
    let (pool, engine) = setup().await;
    let org_id = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        &unique_key("msg-catch"),
        1,
        None,
        &message_catch_bpmn(),
        &json!({}),
    )
    .await
    .unwrap();

    let instance = engine
        .start_instance(def.id, org_id, &json!({}))
        .await
        .unwrap();

    engine
        .correlate_message("OrderShipped", None, &[], org_id)
        .await
        .unwrap();

    let refreshed = db::process_instances::get_by_id(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(refreshed.state, "completed");

    // Subscription should be deleted
    let subs = db::event_subscriptions::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    assert!(subs.is_empty());
}

#[tokio::test]
async fn correlate_message_passes_variables_to_instance() {
    let (pool, engine) = setup().await;
    let org_id = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        &unique_key("msg-catch"),
        1,
        None,
        &message_catch_bpmn(),
        &json!({}),
    )
    .await
    .unwrap();

    let instance = engine
        .start_instance(def.id, org_id, &json!({}))
        .await
        .unwrap();

    let vars = vec![VariableInput {
        name: "trackingId".to_string(),
        value_type: "string".to_string(),
        value: json!("TRK-001"),
    }];
    engine
        .correlate_message("OrderShipped", None, &vars, org_id)
        .await
        .unwrap();

    let variables: Vec<conduit::db::models::Variable> =
        sqlx::query_as("SELECT * FROM variables WHERE instance_id = $1")
            .bind(instance.id)
            .fetch_all(&pool)
            .await
            .unwrap();
    let tracking = variables.iter().find(|v| v.name == "trackingId");
    assert!(tracking.is_some(), "trackingId variable should exist");
    assert_eq!(tracking.unwrap().value, json!("TRK-001"));
}

#[tokio::test]
async fn message_before_ready_returns_not_found() {
    let (pool, engine) = setup().await;
    let org_id = create_org(&pool).await;

    // No instance is running — no subscription exists, no MessageStartEvent deployed
    let result = engine
        .correlate_message("OrderShipped", None, &[], org_id)
        .await;

    assert!(
        matches!(result, Err(EngineError::NotFound(_))),
        "expected NotFound, got {result:?}"
    );
}

#[tokio::test]
async fn correlate_message_wrong_correlation_key_returns_not_found() {
    let (pool, engine) = setup().await;
    let org_id = create_org(&pool).await;

    // Deploy the correlated version (uses ${orderId} expression)
    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        &unique_key("msg-corr"),
        1,
        None,
        &message_catch_correlated_bpmn(),
        &json!({}),
    )
    .await
    .unwrap();

    // Set orderId variable so the subscription is created with key "ORD-1"
    let instance = engine
        .start_instance(def.id, org_id, &json!({}))
        .await
        .unwrap();

    // The ${orderId} correlation key will fail to resolve since no variable was set before
    // starting. The subscription will have been skipped or stored as NULL. Let's force it:
    // Inject orderId variable and restart a fresh instance with it pre-seeded.
    // Simpler: directly set correlation_key in the subscription row.
    sqlx::query("UPDATE event_subscriptions SET correlation_key = 'ORD-1' WHERE instance_id = $1")
        .bind(instance.id)
        .execute(&pool)
        .await
        .unwrap();

    // Correlate with the wrong key — should not match
    let result = engine
        .correlate_message("OrderShipped", Some("ORD-999"), &[], org_id)
        .await;

    assert!(
        matches!(result, Err(EngineError::NotFound(_))),
        "expected NotFound for wrong correlation key, got {result:?}"
    );

    // Correct key works
    engine
        .correlate_message("OrderShipped", Some("ORD-1"), &[], org_id)
        .await
        .unwrap();

    let refreshed = db::process_instances::get_by_id(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(refreshed.state, "completed");
}

#[tokio::test]
async fn message_start_event_creates_new_instance() {
    let (pool, engine) = setup().await;
    let org_id = create_org(&pool).await;

    // Deploy a process with a MessageStartEvent — do NOT call start_instance
    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        &unique_key("msg-start"),
        1,
        None,
        &message_start_bpmn(),
        &json!({}),
    )
    .await
    .unwrap();

    // No instances running yet
    let initial: Vec<conduit::db::models::ProcessInstance> =
        sqlx::query_as("SELECT * FROM process_instances WHERE definition_id = $1")
            .bind(def.id)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert!(initial.is_empty());

    // Correlate — should create a new instance
    engine
        .correlate_message("OrderPlaced", None, &[], org_id)
        .await
        .unwrap();

    let instances: Vec<conduit::db::models::ProcessInstance> =
        sqlx::query_as("SELECT * FROM process_instances WHERE definition_id = $1")
            .bind(def.id)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(instances.len(), 1);
    // Should be running at the UserTask (waiting for human completion)
    assert_eq!(instances[0].state, "running");

    // The UserTask should be pending
    let tasks = db::tasks::list_by_instance(&pool, instances[0].id)
        .await
        .unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].element_id, "task1");
}

#[tokio::test]
async fn receive_task_pauses_and_advances_via_message() {
    let (pool, engine) = setup().await;
    let org_id = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        &unique_key("rt"),
        1,
        None,
        &receive_task_bpmn(),
        &json!({}),
    )
    .await
    .unwrap();

    let instance = engine
        .start_instance(def.id, org_id, &json!({}))
        .await
        .unwrap();

    assert_eq!(instance.state, "running");

    let subs = db::event_subscriptions::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(subs.len(), 1);
    assert_eq!(subs[0].event_name, "PaymentConfirmed");

    engine
        .correlate_message("PaymentConfirmed", None, &[], org_id)
        .await
        .unwrap();

    let refreshed = db::process_instances::get_by_id(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(refreshed.state, "completed");
}

#[tokio::test]
async fn correlate_message_closes_execution_history() {
    let (pool, engine) = setup().await;
    let org_id = create_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        &unique_key("msg-hist"),
        1,
        None,
        &message_catch_bpmn(),
        &json!({}),
    )
    .await
    .unwrap();

    let instance = engine
        .start_instance(def.id, org_id, &json!({}))
        .await
        .unwrap();

    // History: start (left_at set) + wait-msg (left_at NULL)
    let history_before = db::execution_history::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    let wait_entry = history_before.iter().find(|h| h.element_id == "wait-msg");
    assert!(wait_entry.is_some());
    assert!(wait_entry.unwrap().left_at.is_none());

    engine
        .correlate_message("OrderShipped", None, &[], org_id)
        .await
        .unwrap();

    let history_after = db::execution_history::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    for entry in &history_after {
        assert!(
            entry.left_at.is_some(),
            "expected left_at set for {}",
            entry.element_id
        );
    }
}
