mod common;

use uuid::Uuid;

fn unique_key(prefix: &str) -> String {
    format!("{}.{}", prefix, Uuid::new_v4())
}

fn minimal_bpmn(process_id: &str) -> String {
    format!(
        r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL">
  <process id="{process_id}">
    <startEvent id="start"/>
    <endEvent id="end"/>
    <sequenceFlow id="f1" sourceRef="start" targetRef="end"/>
  </process>
</definitions>"#
    )
}

// ── Successful deploys ────────────────────────────────────────────────────────

#[tokio::test]
async fn deploy_valid_bpmn_returns_201() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 2).await;
    let process_group_id = groups[0];
    let client = app.client.clone();

    let key = unique_key("deploy");
    let bpmn = minimal_bpmn("p1");

    let resp = client
        .post(format!("{}/api/v1/orgs/{}/deployments", app.address, org_id))
        .json(&serde_json::json!({ "org_id": org_id, "process_group_id": process_group_id, "key": key, "bpmn_xml": bpmn }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 201);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["id"].is_string(), "id should be a UUID string");
    assert_eq!(body["key"], key);
    assert_eq!(body["version"], 1);
    assert!(body["deployed_at"].is_string());

    // id must parse as a UUID
    Uuid::parse_str(body["id"].as_str().unwrap()).expect("id is not a valid UUID");
}

#[tokio::test]
async fn deploy_with_optional_name() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 2).await;
    let process_group_id = groups[0];
    let client = app.client.clone();

    let key = unique_key("named");
    let bpmn = minimal_bpmn("p2");

    let resp = client
        .post(format!("{}/api/v1/orgs/{}/deployments", app.address, org_id))
        .json(&serde_json::json!({ "org_id": org_id, "process_group_id": process_group_id, "key": key, "name": "My Process", "bpmn_xml": bpmn }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 201);
}

#[tokio::test]
async fn deploy_same_key_twice_increments_version() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 2).await;
    let process_group_id = groups[0];
    let client = app.client.clone();

    let key = unique_key("versioned");
    let bpmn = minimal_bpmn("p3");

    let resp1 = client
        .post(format!("{}/api/v1/orgs/{}/deployments", app.address, org_id))
        .json(&serde_json::json!({ "org_id": org_id, "process_group_id": process_group_id, "key": &key, "bpmn_xml": &bpmn }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp1.status(), 201);
    let body1: serde_json::Value = resp1.json().await.unwrap();
    assert_eq!(body1["version"], 1);

    let resp2 = client
        .post(format!("{}/api/v1/orgs/{}/deployments", app.address, org_id))
        .json(&serde_json::json!({ "org_id": org_id, "process_group_id": process_group_id, "key": &key, "bpmn_xml": &bpmn }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp2.status(), 201);
    let body2: serde_json::Value = resp2.json().await.unwrap();
    assert_eq!(body2["version"], 2);
    assert_eq!(body2["key"], key);

    // IDs must be distinct
    assert_ne!(body1["id"], body2["id"]);
}

#[tokio::test]
async fn deploy_stores_bpmn_xml_in_db() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 2).await;
    let process_group_id = groups[0];
    let client = app.client.clone();

    let key = unique_key("stored");
    let bpmn = minimal_bpmn("p4");

    let resp = client
        .post(format!("{}/api/v1/orgs/{}/deployments", app.address, org_id))
        .json(&serde_json::json!({ "org_id": org_id, "process_group_id": process_group_id, "key": &key, "bpmn_xml": &bpmn }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);

    let body: serde_json::Value = resp.json().await.unwrap();
    let id = Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();

    let row: (String,) = sqlx::query_as("SELECT bpmn_xml FROM process_definitions WHERE id = $1")
        .bind(id)
        .fetch_one(&app.pool)
        .await
        .expect("row not found in DB");

    assert_eq!(row.0, bpmn);
}

// ── Validation rejections ─────────────────────────────────────────────────────

#[tokio::test]
async fn deploy_empty_key_returns_400() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 2).await;
    let process_group_id = groups[0];
    let client = app.client.clone();

    let bpmn = minimal_bpmn("p5");

    let resp = client
        .post(format!("{}/api/v1/orgs/{}/deployments", app.address, org_id))
        .json(&serde_json::json!({ "org_id": org_id, "process_group_id": process_group_id, "key": "   ", "bpmn_xml": bpmn }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["message"].is_string());
}

#[tokio::test]
async fn deploy_invalid_xml_returns_400() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 2).await;
    let process_group_id = groups[0];
    let client = app.client.clone();

    let key = unique_key("badxml");

    let resp = client
        .post(format!("{}/api/v1/orgs/{}/deployments", app.address, org_id))
        .json(
            &serde_json::json!({ "org_id": org_id, "process_group_id": process_group_id, "key": key, "bpmn_xml": "<not valid xml <<>>" }),
        )
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["message"].is_string());
}

#[tokio::test]
async fn deploy_unsupported_gateway_returns_400() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 2).await;
    let process_group_id = groups[0];
    let client = app.client.clone();

    let key = unique_key("gateway");
    let bpmn = r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL">
  <process id="gw_process">
    <startEvent id="start"/>
    <eventBasedGateway id="gw"/>
    <endEvent id="end"/>
    <sequenceFlow id="f1" sourceRef="start" targetRef="gw"/>
    <sequenceFlow id="f2" sourceRef="gw" targetRef="end"/>
  </process>
</definitions>"#;

    let resp = client
        .post(format!("{}/api/v1/orgs/{}/deployments", app.address, org_id))
        .json(&serde_json::json!({ "org_id": org_id, "process_group_id": process_group_id, "key": key, "bpmn_xml": bpmn }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(
        body["message"]
            .as_str()
            .unwrap_or("")
            .contains("eventBasedGateway"),
        "error message should mention eventBasedGateway, got: {}",
        body["message"]
    );
}

#[tokio::test]
async fn deploy_missing_start_event_returns_400() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 2).await;
    let process_group_id = groups[0];
    let client = app.client.clone();

    let key = unique_key("nostart");
    let bpmn = r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL">
  <process id="no_start">
    <userTask id="t"/>
    <endEvent id="end"/>
    <sequenceFlow id="f1" sourceRef="t" targetRef="end"/>
  </process>
</definitions>"#;

    let resp = client
        .post(format!("{}/api/v1/orgs/{}/deployments", app.address, org_id))
        .json(&serde_json::json!({ "org_id": org_id, "process_group_id": process_group_id, "key": key, "bpmn_xml": bpmn }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn deploy_does_not_persist_on_parse_failure() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 2).await;
    let process_group_id = groups[0];
    let client = app.client.clone();

    let key = unique_key("nopersist");

    let resp = client
        .post(format!("{}/api/v1/orgs/{}/deployments", app.address, org_id))
        .json(&serde_json::json!({ "org_id": org_id, "process_group_id": process_group_id, "key": &key, "bpmn_xml": "<bad>" }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);

    // No row should have been written for this key
    let count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM process_definitions WHERE process_key = $1")
            .bind(&key)
            .fetch_one(&app.pool)
            .await
            .unwrap();

    assert_eq!(count.0, 0, "no DB row should be created on parse failure");
}

// ── Missing / malformed request body ─────────────────────────────────────────

#[tokio::test]
async fn deploy_missing_key_field_returns_400() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 2).await;
    let process_group_id = groups[0];
    let client = app.client.clone();

    let bpmn = minimal_bpmn("p6");

    let resp = client
        .post(format!("{}/api/v1/orgs/{}/deployments", app.address, org_id))
        .json(&serde_json::json!({ "org_id": org_id, "process_group_id": process_group_id, "bpmn_xml": bpmn }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn deploy_missing_bpmn_xml_field_returns_400() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 2).await;
    let process_group_id = groups[0];
    let client = app.client.clone();

    let resp = client
        .post(format!("{}/api/v1/orgs/{}/deployments", app.address, org_id))
        .json(&serde_json::json!({ "org_id": org_id, "process_group_id": process_group_id, "key": "some-key" }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
}

// ── Phase 20: <conduit:http> deprecation warning ──────────────────────────────

fn http_connector_bpmn(process_id: &str, task_id: &str) -> String {
    format!(
        r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL"
             xmlns:conduit="http://conduit.io/ext">
  <process id="{process_id}">
    <startEvent id="start"/>
    <serviceTask id="{task_id}">
      <extensionElements>
        <conduit:http>
          <conduit:url>https://example.invalid/api</conduit:url>
          <conduit:method>POST</conduit:method>
        </conduit:http>
      </extensionElements>
    </serviceTask>
    <endEvent id="end"/>
    <sequenceFlow id="f1" sourceRef="start" targetRef="{task_id}"/>
    <sequenceFlow id="f2" sourceRef="{task_id}" targetRef="end"/>
  </process>
</definitions>"#
    )
}

#[tokio::test]
async fn deploy_with_conduit_http_returns_u010_warning() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 2).await;
    let process_group_id = groups[0];
    let client = app.client.clone();

    let key = unique_key("deprecated-http");
    let bpmn = http_connector_bpmn("dep_http_proc", "call_api");

    let resp = client
        .post(format!(
            "{}/api/v1/orgs/{}/deployments",
            app.address, org_id
        ))
        .json(&serde_json::json!({
            "process_group_id": process_group_id,
            "key": key,
            "bpmn_xml": bpmn,
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(
        resp.status(),
        201,
        "deployment with <conduit:http> must still succeed"
    );

    let body: serde_json::Value = resp.json().await.unwrap();
    let warnings = body["warnings"]
        .as_array()
        .expect("response should include a `warnings` array");
    assert_eq!(
        warnings.len(),
        1,
        "expected exactly one warning, got {warnings:?}"
    );

    let w = &warnings[0];
    assert_eq!(w["code"], "U010");
    assert_eq!(w["element_id"], "call_api");
    assert!(
        w["message"].as_str().unwrap().contains("conduit:http"),
        "warning message should mention conduit:http; got {:?}",
        w["message"]
    );
}

#[tokio::test]
async fn deploy_without_deprecated_elements_has_empty_warnings() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 2).await;
    let process_group_id = groups[0];
    let client = app.client.clone();

    let key = unique_key("no-warnings");
    let bpmn = minimal_bpmn("clean_proc");

    let resp = client
        .post(format!(
            "{}/api/v1/orgs/{}/deployments",
            app.address, org_id
        ))
        .json(&serde_json::json!({
            "process_group_id": process_group_id,
            "key": key,
            "bpmn_xml": bpmn,
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 201);

    let body: serde_json::Value = resp.json().await.unwrap();
    let warnings = body["warnings"]
        .as_array()
        .expect("response should include a `warnings` array");
    assert!(
        warnings.is_empty(),
        "clean BPMN must produce no warnings; got {warnings:?}"
    );
}

#[tokio::test]
async fn deploy_with_two_conduit_http_emits_two_warnings() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 2).await;
    let process_group_id = groups[0];
    let client = app.client.clone();

    let key = unique_key("two-deprecated");
    let bpmn = r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL"
             xmlns:conduit="http://conduit.io/ext">
  <process id="two_http_proc">
    <startEvent id="start"/>
    <serviceTask id="call_a">
      <extensionElements>
        <conduit:http>
          <conduit:url>https://example.invalid/a</conduit:url>
        </conduit:http>
      </extensionElements>
    </serviceTask>
    <serviceTask id="call_b">
      <extensionElements>
        <conduit:http>
          <conduit:url>https://example.invalid/b</conduit:url>
        </conduit:http>
      </extensionElements>
    </serviceTask>
    <endEvent id="end"/>
    <sequenceFlow id="f1" sourceRef="start" targetRef="call_a"/>
    <sequenceFlow id="f2" sourceRef="call_a" targetRef="call_b"/>
    <sequenceFlow id="f3" sourceRef="call_b" targetRef="end"/>
  </process>
</definitions>"#;

    let resp = client
        .post(format!(
            "{}/api/v1/orgs/{}/deployments",
            app.address, org_id
        ))
        .json(&serde_json::json!({
            "process_group_id": process_group_id,
            "key": key,
            "bpmn_xml": bpmn,
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 201);

    let body: serde_json::Value = resp.json().await.unwrap();
    let warnings = body["warnings"].as_array().unwrap();
    assert_eq!(warnings.len(), 2);
    let element_ids: Vec<&str> = warnings
        .iter()
        .map(|w| w["element_id"].as_str().unwrap())
        .collect();
    assert!(element_ids.contains(&"call_a"));
    assert!(element_ids.contains(&"call_b"));
    for w in warnings {
        assert_eq!(w["code"], "U010");
    }
}
