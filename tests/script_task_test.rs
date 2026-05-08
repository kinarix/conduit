// Tests covering scriptTask: FEEL evaluation, context-vs-scalar result handling,
// missing resultVariable, invalid expressions, and routing through a downstream
// exclusiveGateway off a script-computed variable.

mod common;

use common::{create_engine_org, engine_setup, unique_key, var};
use conduit::db;
use serde_json::json;
use std::collections::HashMap;

/// Start → ScriptTask (context output: fee + tier) → End
fn script_task_context_bpmn() -> String {
    r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def1" targetNamespace="urn:test">
  <process id="proc" isExecutable="true">
    <startEvent id="start"/>
    <scriptTask id="compute" name="Compute fee">
      <script>{ fee: amount * 0.05, tier: if amount > 1000 then "premium" else "standard" }</script>
    </scriptTask>
    <endEvent id="end"/>
    <sequenceFlow id="sf1" sourceRef="start" targetRef="compute"/>
    <sequenceFlow id="sf2" sourceRef="compute" targetRef="end"/>
  </process>
</definitions>"#
    .to_string()
}

/// Start → ScriptTask (scalar + resultVariable) → End
fn script_task_scalar_bpmn() -> String {
    r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def1" targetNamespace="urn:test">
  <process id="proc" isExecutable="true">
    <startEvent id="start"/>
    <scriptTask id="compute" name="Compute total">
      <extensionElements>
        <conduit:resultVariable xmlns:conduit="http://conduit.io/ext">total</conduit:resultVariable>
      </extensionElements>
      <script>amount + shipping</script>
    </scriptTask>
    <endEvent id="end"/>
    <sequenceFlow id="sf1" sourceRef="start" targetRef="compute"/>
    <sequenceFlow id="sf2" sourceRef="compute" targetRef="end"/>
  </process>
</definitions>"#
    .to_string()
}

/// Start → ScriptTask (scalar, NO resultVariable) → End
fn script_task_scalar_no_var_bpmn() -> String {
    r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def1" targetNamespace="urn:test">
  <process id="proc" isExecutable="true">
    <startEvent id="start"/>
    <scriptTask id="compute" name="Compute total">
      <script>amount + shipping</script>
    </scriptTask>
    <endEvent id="end"/>
    <sequenceFlow id="sf1" sourceRef="start" targetRef="compute"/>
    <sequenceFlow id="sf2" sourceRef="compute" targetRef="end"/>
  </process>
</definitions>"#
    .to_string()
}

/// Start → ScriptTask → ExclusiveGateway (routes on computed var) → End
fn script_task_before_gateway_bpmn() -> String {
    r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def1" targetNamespace="urn:test">
  <process id="proc" isExecutable="true">
    <startEvent id="start"/>
    <scriptTask id="compute" name="Compute tier">
      <script>{ tier: if amount > 1000 then "premium" else "standard" }</script>
    </scriptTask>
    <exclusiveGateway id="xgw" default="sf_standard"/>
    <endEvent id="premium_end"/>
    <endEvent id="standard_end"/>
    <sequenceFlow id="sf1" sourceRef="start" targetRef="compute"/>
    <sequenceFlow id="sf2" sourceRef="compute" targetRef="xgw"/>
    <sequenceFlow id="sf_premium" sourceRef="xgw" targetRef="premium_end">
      <conditionExpression>tier = "premium"</conditionExpression>
    </sequenceFlow>
    <sequenceFlow id="sf_standard" sourceRef="xgw" targetRef="standard_end"/>
  </process>
</definitions>"#
    .to_string()
}

#[tokio::test]
async fn script_task_context_sets_multiple_variables() {
    let (pool, engine) = engine_setup().await;
    let (org_id, groups) = create_engine_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("script"),
        1,
        None,
        &script_task_context_bpmn(),
        &json!({}),
    )
    .await
    .unwrap();

    let instance = engine
        .start_instance(
            def.id,
            org_id,
            &json!({}),
            &[var("amount", "integer", json!(2000))],
        )
        .await
        .unwrap();

    assert_eq!(instance.state, "completed");

    let vars = db::variables::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    let var_map: HashMap<String, _> = vars.into_iter().map(|v| (v.name, v.value)).collect();
    // FEEL: 2000 * 0.05 = 100 (exact decimal), feel_to_json → json!(100)
    assert_eq!(var_map["fee"], json!(100));
    assert_eq!(var_map["tier"], json!("premium"));
}

#[tokio::test]
async fn script_task_scalar_with_result_variable() {
    let (pool, engine) = engine_setup().await;
    let (org_id, groups) = create_engine_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("script"),
        1,
        None,
        &script_task_scalar_bpmn(),
        &json!({}),
    )
    .await
    .unwrap();

    let instance = engine
        .start_instance(
            def.id,
            org_id,
            &json!({}),
            &[
                var("amount", "integer", json!(500)),
                var("shipping", "integer", json!(25)),
            ],
        )
        .await
        .unwrap();

    assert_eq!(instance.state, "completed");

    let vars = db::variables::list_by_instance(&pool, instance.id)
        .await
        .unwrap();
    let var_map: HashMap<String, _> = vars.into_iter().map(|v| (v.name, v.value)).collect();
    // FEEL: 500 + 25 = 525, feel_to_json → json!(525)
    assert_eq!(var_map["total"], json!(525));
}

#[tokio::test]
async fn script_task_scalar_without_result_variable_errors() {
    let (pool, engine) = engine_setup().await;
    let (org_id, groups) = create_engine_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("script"),
        1,
        None,
        &script_task_scalar_no_var_bpmn(),
        &json!({}),
    )
    .await
    .unwrap();

    let instance = engine
        .start_instance(
            def.id,
            org_id,
            &json!({}),
            &[
                var("amount", "integer", json!(100)),
                var("shipping", "integer", json!(10)),
            ],
        )
        .await
        .unwrap();

    assert_eq!(
        instance.state, "error",
        "scalar result with no resultVariable should put instance in error"
    );
}

/// Start → ScriptTask with syntactically invalid FEEL → instance errors
fn script_task_invalid_feel_bpmn() -> String {
    r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def1" targetNamespace="urn:test">
  <process id="proc" isExecutable="true">
    <startEvent id="start"/>
    <scriptTask id="compute" name="Bad script">
      <script>@@@not valid feel@@@</script>
    </scriptTask>
    <endEvent id="end"/>
    <sequenceFlow id="sf1" sourceRef="start" targetRef="compute"/>
    <sequenceFlow id="sf2" sourceRef="compute" targetRef="end"/>
  </process>
</definitions>"#
    .to_string()
}

#[tokio::test]
async fn script_task_invalid_feel_expression_errors() {
    let (pool, engine) = engine_setup().await;
    let (org_id, groups) = create_engine_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("script"),
        1,
        None,
        &script_task_invalid_feel_bpmn(),
        &json!({}),
    )
    .await
    .unwrap();

    let instance = engine
        .start_instance(def.id, org_id, &json!({}), &[])
        .await
        .unwrap();

    assert_eq!(
        instance.state, "error",
        "invalid FEEL expression in script should put instance in error state"
    );
}

#[tokio::test]
async fn script_task_before_gateway_routes_correctly() {
    let (pool, engine) = engine_setup().await;
    let (org_id, groups) = create_engine_org(&pool).await;

    let def = db::process_definitions::insert(
        &pool,
        org_id,
        None,
        groups[0],
        &unique_key("script"),
        1,
        None,
        &script_task_before_gateway_bpmn(),
        &json!({}),
    )
    .await
    .unwrap();

    // amount > 1000 → tier = "premium" → premium_end
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
    assert!(visited.contains(&"premium_end"));
    assert!(!visited.contains(&"standard_end"));
}
