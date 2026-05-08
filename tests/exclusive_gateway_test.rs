// Tests covering exclusiveGateway routing semantics: condition evaluation,
// default-flow fallback, broken-expression handling, nested gateways, and
// the gateway-first input validation pattern (start → exclusiveGateway with
// a conduit:inputSchema).

mod common;

use common::{create_engine_org, engine_setup, unique_key, var};
use conduit::db;
use serde_json::json;

// ─── happy paths ──────────────────────────────────────────────────────────────

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
    let (pool, engine) = engine_setup().await;
    let (org_id, groups) = create_engine_org(&pool).await;

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
    let (pool, engine) = engine_setup().await;
    let (org_id, groups) = create_engine_org(&pool).await;

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
    let (pool, engine) = engine_setup().await;
    let (org_id, groups) = create_engine_org(&pool).await;

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
      <conditionExpression>tier = "premium"</conditionExpression>
    </sequenceFlow>
    <sequenceFlow id="sf_standard" sourceRef="xgw1" targetRef="standard"/>
    <sequenceFlow id="sf_high" sourceRef="xgw2" targetRef="premium_high">
      <conditionExpression>amount > 5000</conditionExpression>
    </sequenceFlow>
    <sequenceFlow id="sf_low" sourceRef="xgw2" targetRef="premium_low"/>
  </process>
</definitions>"#;

    let (pool, engine) = engine_setup().await;
    let (org_id, groups) = create_engine_org(&pool).await;

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

// ─── error / edge cases ───────────────────────────────────────────────────────

/// Bad expression on a non-default flow → instance error, default not taken.
fn gateway_broken_expression_bpmn() -> String {
    r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def1" targetNamespace="urn:test">
  <process id="proc" isExecutable="true">
    <startEvent id="start"/>
    <userTask id="task1" name="Submit Request"/>
    <exclusiveGateway id="xgw1" default="sf_default"/>
    <endEvent id="conditional_end"/>
    <endEvent id="default_end"/>
    <sequenceFlow id="sf1" sourceRef="start" targetRef="task1"/>
    <sequenceFlow id="sf2" sourceRef="task1" targetRef="xgw1"/>
    <sequenceFlow id="sf_broken" sourceRef="xgw1" targetRef="conditional_end">
      <conditionExpression>amount @@@ broken</conditionExpression>
    </sequenceFlow>
    <sequenceFlow id="sf_default" sourceRef="xgw1" targetRef="default_end"/>
  </process>
</definitions>"#
        .to_string()
}

#[tokio::test]
async fn gateway_eval_error_marks_instance_error_does_not_take_default() {
    let (pool, engine) = engine_setup().await;
    let (org_id, groups) = create_engine_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("eng"),
        1,
        None,
        &gateway_broken_expression_bpmn(),
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
        .complete_user_task(task_id, &[var("amount", "integer", json!(100))])
        .await
        .unwrap();

    let refreshed = db::process_instances::get_by_id(&pool, instance.id)
        .await
        .unwrap();
    assert_eq!(
        refreshed.state, "error",
        "broken condition must mark instance error, not silently take default"
    );

    let history = db::execution_history::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    let visited: Vec<_> = history.iter().map(|h| h.element_id.as_str()).collect();
    assert!(
        !visited.contains(&"default_end"),
        "default flow must NOT be taken when a non-default condition errors"
    );
    assert!(
        !visited.contains(&"conditional_end"),
        "conditional flow must NOT be taken when its condition errors"
    );
}

/// A default flow that *also* carries a conditionExpression — its condition must be ignored.
/// (BPMN spec: the default is taken iff no other condition matches; its own condition is irrelevant.)
fn gateway_default_with_condition_bpmn() -> String {
    r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def1" targetNamespace="urn:test">
  <process id="proc" isExecutable="true">
    <startEvent id="start"/>
    <userTask id="task1" name="Submit"/>
    <exclusiveGateway id="xgw1" default="sf_default"/>
    <endEvent id="approved"/>
    <endEvent id="rejected"/>
    <sequenceFlow id="sf1" sourceRef="start" targetRef="task1"/>
    <sequenceFlow id="sf2" sourceRef="task1" targetRef="xgw1"/>
    <sequenceFlow id="sf_approved" sourceRef="xgw1" targetRef="approved">
      <conditionExpression>amount > 1000</conditionExpression>
    </sequenceFlow>
    <sequenceFlow id="sf_default" sourceRef="xgw1" targetRef="rejected">
      <conditionExpression>false</conditionExpression>
    </sequenceFlow>
  </process>
</definitions>"#
        .to_string()
}

#[tokio::test]
async fn gateway_default_flow_condition_is_ignored() {
    let (pool, engine) = engine_setup().await;
    let (org_id, groups) = create_engine_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("eng"),
        1,
        None,
        &gateway_default_with_condition_bpmn(),
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

    // amount <= 1000 → sf_approved condition false → fall to default. Default has
    // a `false` condition, but that condition must be ignored — token still routes.
    engine
        .complete_user_task(task_id, &[var("amount", "integer", json!(100))])
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
    assert!(visited.contains(&"rejected"));
}

// ─── object / array routing ───────────────────────────────────────────────────

/// Object/array variables routed via field-access conditions.
fn gateway_object_variable_bpmn() -> String {
    r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def1" targetNamespace="urn:test">
  <process id="proc" isExecutable="true">
    <startEvent id="start"/>
    <userTask id="task1" name="Submit"/>
    <exclusiveGateway id="xgw1" default="sf_other"/>
    <endEvent id="vip_end"/>
    <endEvent id="bulk_end"/>
    <endEvent id="other_end"/>
    <sequenceFlow id="sf1" sourceRef="start" targetRef="task1"/>
    <sequenceFlow id="sf2" sourceRef="task1" targetRef="xgw1"/>
    <sequenceFlow id="sf_vip" sourceRef="xgw1" targetRef="vip_end">
      <conditionExpression>customer.tier = "gold"</conditionExpression>
    </sequenceFlow>
    <sequenceFlow id="sf_bulk" sourceRef="xgw1" targetRef="bulk_end">
      <conditionExpression>count(items) >= 10</conditionExpression>
    </sequenceFlow>
    <sequenceFlow id="sf_other" sourceRef="xgw1" targetRef="other_end"/>
  </process>
</definitions>"#
        .to_string()
}

#[tokio::test]
async fn gateway_routes_on_object_field_access() {
    let (pool, engine) = engine_setup().await;
    let (org_id, groups) = create_engine_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("eng"),
        1,
        None,
        &gateway_object_variable_bpmn(),
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
                var("customer", "json", json!({"tier": "gold", "id": 7})),
                var("items", "json", json!([1, 2, 3])),
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
    assert!(visited.contains(&"vip_end"));
    assert!(!visited.contains(&"bulk_end"));
    assert!(!visited.contains(&"other_end"));
}

#[tokio::test]
async fn gateway_routes_on_array_length() {
    let (pool, engine) = engine_setup().await;
    let (org_id, groups) = create_engine_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("eng"),
        1,
        None,
        &gateway_object_variable_bpmn(),
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

    // tier != "gold", count(items) = 12 → sf_bulk
    engine
        .complete_user_task(
            task_id,
            &[
                var("customer", "json", json!({"tier": "silver"})),
                var(
                    "items",
                    "json",
                    json!([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]),
                ),
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
    assert!(visited.contains(&"bulk_end"));
    assert!(!visited.contains(&"vip_end"));
    assert!(!visited.contains(&"other_end"));
}

// ─── Gateway-first input validation ───────────────────────────────────────────

/// Start → ExclusiveGateway (amount > 1000 → approved / default → rejected)
/// with a conduit:inputSchema requiring `amount: integer`.
fn gateway_first_bpmn_with_schema() -> String {
    r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def1" targetNamespace="urn:test">
  <process id="proc" isExecutable="true">
    <extensionElements>
      <conduit:inputSchema xmlns:conduit="http://conduit.io/ext">
        {"type":"object","required":["amount"],"properties":{"amount":{"type":"integer"}},"additionalProperties":true}
      </conduit:inputSchema>
    </extensionElements>
    <startEvent id="start"/>
    <exclusiveGateway id="xgw1" default="sf_rejected"/>
    <endEvent id="approved"/>
    <endEvent id="rejected"/>
    <sequenceFlow id="sf1" sourceRef="start" targetRef="xgw1"/>
    <sequenceFlow id="sf_approved" sourceRef="xgw1" targetRef="approved">
      <conditionExpression>amount > 1000</conditionExpression>
    </sequenceFlow>
    <sequenceFlow id="sf_rejected" sourceRef="xgw1" targetRef="rejected"/>
  </process>
</definitions>"#
        .to_string()
}

/// Start → ExclusiveGateway with NO input schema.
fn gateway_first_bpmn_no_schema() -> String {
    r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def1" targetNamespace="urn:test">
  <process id="proc" isExecutable="true">
    <startEvent id="start"/>
    <exclusiveGateway id="xgw1" default="sf_rejected"/>
    <endEvent id="approved"/>
    <endEvent id="rejected"/>
    <sequenceFlow id="sf1" sourceRef="start" targetRef="xgw1"/>
    <sequenceFlow id="sf_approved" sourceRef="xgw1" targetRef="approved">
      <conditionExpression>amount > 1000</conditionExpression>
    </sequenceFlow>
    <sequenceFlow id="sf_rejected" sourceRef="xgw1" targetRef="rejected"/>
  </process>
</definitions>"#
        .to_string()
}

#[tokio::test]
async fn gateway_first_valid_variables_routes_correctly() {
    let (pool, engine) = engine_setup().await;
    let (org_id, groups) = create_engine_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("eng"),
        1,
        None,
        &gateway_first_bpmn_with_schema(),
        &json!({}),
    )
    .await
    .unwrap();

    let instance = engine
        .start_instance(
            def.id,
            org_id,
            &json!({}),
            &[var("amount", "integer", json!(1500))],
        )
        .await
        .unwrap();

    assert_eq!(instance.state, "completed");

    let history = db::execution_history::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    let visited: Vec<_> = history.iter().map(|h| h.element_id.as_str()).collect();
    assert!(visited.contains(&"approved"));
    assert!(!visited.contains(&"rejected"));
}

#[tokio::test]
async fn gateway_first_missing_required_variable_returns_422() {
    let (pool, engine) = engine_setup().await;
    let (org_id, groups) = create_engine_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("eng"),
        1,
        None,
        &gateway_first_bpmn_with_schema(),
        &json!({}),
    )
    .await
    .unwrap();

    // Start with no variables — `amount` is required by the schema
    let result = engine.start_instance(def.id, org_id, &json!({}), &[]).await;

    assert!(
        matches!(result, Err(conduit::error::EngineError::Validation(_))),
        "expected Validation error, got: {:?}",
        result
    );

    // No instance should have been created
    let instances = db::process_instances::list_by_definition(&pool, def.id)
        .await
        .unwrap();
    assert!(instances.is_empty());
}

#[tokio::test]
async fn gateway_first_wrong_variable_type_returns_422() {
    let (pool, engine) = engine_setup().await;
    let (org_id, groups) = create_engine_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("eng"),
        1,
        None,
        &gateway_first_bpmn_with_schema(),
        &json!({}),
    )
    .await
    .unwrap();

    // Pass `amount` as a string instead of integer
    let result = engine
        .start_instance(
            def.id,
            org_id,
            &json!({}),
            &[var("amount", "string", json!("not-a-number"))],
        )
        .await;

    assert!(
        matches!(result, Err(conduit::error::EngineError::Validation(_))),
        "expected Validation error, got: {:?}",
        result
    );
}

#[tokio::test]
async fn gateway_first_no_schema_undefined_variable_produces_error_state() {
    let (pool, engine) = engine_setup().await;
    let (org_id, groups) = create_engine_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("eng"),
        1,
        None,
        &gateway_first_bpmn_no_schema(),
        &json!({}),
    )
    .await
    .unwrap();

    // No schema: start_instance succeeds but the instance ends up in error
    // because FEEL can't evaluate `amount > 1000` without `amount`.
    let instance = engine
        .start_instance(def.id, org_id, &json!({}), &[])
        .await
        .unwrap();

    assert_eq!(
        instance.state, "error",
        "instance should be in error state when FEEL condition references undefined variable"
    );
}
