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
    let org_id = common::create_test_org(&app).await;
    let client = reqwest::Client::new();

    let key = unique_key("deploy");
    let bpmn = minimal_bpmn("p1");

    let resp = client
        .post(format!("{}/api/v1/deployments", app.address))
        .json(&serde_json::json!({ "org_id": org_id, "key": key, "bpmn_xml": bpmn }))
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
    let org_id = common::create_test_org(&app).await;
    let client = reqwest::Client::new();

    let key = unique_key("named");
    let bpmn = minimal_bpmn("p2");

    let resp = client
        .post(format!("{}/api/v1/deployments", app.address))
        .json(&serde_json::json!({ "org_id": org_id, "key": key, "name": "My Process", "bpmn_xml": bpmn }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 201);
}

#[tokio::test]
async fn deploy_same_key_twice_increments_version() {
    let app = common::spawn_test_app().await;
    let org_id = common::create_test_org(&app).await;
    let client = reqwest::Client::new();

    let key = unique_key("versioned");
    let bpmn = minimal_bpmn("p3");

    let resp1 = client
        .post(format!("{}/api/v1/deployments", app.address))
        .json(&serde_json::json!({ "org_id": org_id, "key": &key, "bpmn_xml": &bpmn }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp1.status(), 201);
    let body1: serde_json::Value = resp1.json().await.unwrap();
    assert_eq!(body1["version"], 1);

    let resp2 = client
        .post(format!("{}/api/v1/deployments", app.address))
        .json(&serde_json::json!({ "org_id": org_id, "key": &key, "bpmn_xml": &bpmn }))
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
    let org_id = common::create_test_org(&app).await;
    let client = reqwest::Client::new();

    let key = unique_key("stored");
    let bpmn = minimal_bpmn("p4");

    let resp = client
        .post(format!("{}/api/v1/deployments", app.address))
        .json(&serde_json::json!({ "org_id": org_id, "key": &key, "bpmn_xml": &bpmn }))
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
    let org_id = common::create_test_org(&app).await;
    let client = reqwest::Client::new();

    let bpmn = minimal_bpmn("p5");

    let resp = client
        .post(format!("{}/api/v1/deployments", app.address))
        .json(&serde_json::json!({ "org_id": org_id, "key": "   ", "bpmn_xml": bpmn }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["error"].is_string());
}

#[tokio::test]
async fn deploy_invalid_xml_returns_400() {
    let app = common::spawn_test_app().await;
    let org_id = common::create_test_org(&app).await;
    let client = reqwest::Client::new();

    let key = unique_key("badxml");

    let resp = client
        .post(format!("{}/api/v1/deployments", app.address))
        .json(
            &serde_json::json!({ "org_id": org_id, "key": key, "bpmn_xml": "<not valid xml <<>>" }),
        )
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["error"].is_string());
}

#[tokio::test]
async fn deploy_unsupported_gateway_returns_400() {
    let app = common::spawn_test_app().await;
    let org_id = common::create_test_org(&app).await;
    let client = reqwest::Client::new();

    let key = unique_key("gateway");
    let bpmn = r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL">
  <process id="gw_process">
    <startEvent id="start"/>
    <parallelGateway id="gw"/>
    <endEvent id="end"/>
    <sequenceFlow id="f1" sourceRef="start" targetRef="gw"/>
    <sequenceFlow id="f2" sourceRef="gw" targetRef="end"/>
  </process>
</definitions>"#;

    let resp = client
        .post(format!("{}/api/v1/deployments", app.address))
        .json(&serde_json::json!({ "org_id": org_id, "key": key, "bpmn_xml": bpmn }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(
        body["error"]
            .as_str()
            .unwrap_or("")
            .contains("parallelGateway"),
        "error message should mention parallelGateway, got: {}",
        body["error"]
    );
}

#[tokio::test]
async fn deploy_missing_start_event_returns_400() {
    let app = common::spawn_test_app().await;
    let org_id = common::create_test_org(&app).await;
    let client = reqwest::Client::new();

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
        .post(format!("{}/api/v1/deployments", app.address))
        .json(&serde_json::json!({ "org_id": org_id, "key": key, "bpmn_xml": bpmn }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn deploy_does_not_persist_on_parse_failure() {
    let app = common::spawn_test_app().await;
    let org_id = common::create_test_org(&app).await;
    let client = reqwest::Client::new();

    let key = unique_key("nopersist");

    let resp = client
        .post(format!("{}/api/v1/deployments", app.address))
        .json(&serde_json::json!({ "org_id": org_id, "key": &key, "bpmn_xml": "<bad>" }))
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
async fn deploy_missing_key_field_returns_422() {
    let app = common::spawn_test_app().await;
    let org_id = common::create_test_org(&app).await;
    let client = reqwest::Client::new();

    let bpmn = minimal_bpmn("p6");

    let resp = client
        .post(format!("{}/api/v1/deployments", app.address))
        .json(&serde_json::json!({ "org_id": org_id, "bpmn_xml": bpmn }))
        .send()
        .await
        .unwrap();

    // Axum returns 422 Unprocessable Entity when required JSON fields are missing
    assert_eq!(resp.status(), 422);
}

#[tokio::test]
async fn deploy_missing_bpmn_xml_field_returns_422() {
    let app = common::spawn_test_app().await;
    let org_id = common::create_test_org(&app).await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{}/api/v1/deployments", app.address))
        .json(&serde_json::json!({ "org_id": org_id, "key": "some-key" }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 422);
}
