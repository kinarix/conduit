mod common;

use uuid::Uuid;

fn unique_key(prefix: &str) -> String {
    format!("{}.{}", prefix, Uuid::new_v4())
}

fn risk_check_dmn() -> String {
    std::fs::read_to_string("tests/fixtures/dmn/risk_check.dmn")
        .expect("risk_check.dmn fixture not found")
}

fn multi_decision_dmn() -> String {
    std::fs::read_to_string("tests/fixtures/dmn/multi_decision.dmn")
        .expect("multi_decision.dmn fixture not found")
}

fn business_rule_bpmn() -> String {
    std::fs::read_to_string("tests/fixtures/bpmn/business_rule_task.bpmn")
        .expect("business_rule_task.bpmn fixture not found")
}

// ── Deployment API ────────────────────────────────────────────────────────────

#[tokio::test]
async fn deploy_single_decision() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 2).await;
    let _process_group_id = groups[0];
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{}/api/v1/decisions", app.address))
        .header("Content-Type", "application/xml")
        .header("X-Org-Id", org_id.to_string())
        .body(risk_check_dmn())
        .send()
        .await
        .unwrap();

    assert_eq!(
        resp.status(),
        201,
        "expected 201, body: {}",
        resp.text().await.unwrap()
    );
    let body: serde_json::Value = resp.json().await.unwrap();

    let deployed = body["deployed"].as_array().unwrap();
    assert_eq!(deployed.len(), 1);
    assert_eq!(deployed[0]["decision_key"], "risk-check");
    assert_eq!(deployed[0]["version"], 1);
    assert!(deployed[0]["id"].is_string());
}

#[tokio::test]
async fn deploy_increments_version() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 2).await;
    let _process_group_id = groups[0];
    let client = reqwest::Client::new();

    // First deploy
    let resp1 = client
        .post(format!("{}/api/v1/decisions", app.address))
        .header("Content-Type", "application/xml")
        .header("X-Org-Id", org_id.to_string())
        .body(risk_check_dmn())
        .send()
        .await
        .unwrap();
    assert_eq!(resp1.status(), 201);

    // Second deploy of same decision
    let resp2 = client
        .post(format!("{}/api/v1/decisions", app.address))
        .header("Content-Type", "application/xml")
        .header("X-Org-Id", org_id.to_string())
        .body(risk_check_dmn())
        .send()
        .await
        .unwrap();
    assert_eq!(resp2.status(), 201);
    let body2: serde_json::Value = resp2.json().await.unwrap();
    assert_eq!(body2["deployed"][0]["version"], 2);
}

#[tokio::test]
async fn deploy_multi_decision_file() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 2).await;
    let _process_group_id = groups[0];
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{}/api/v1/decisions", app.address))
        .header("Content-Type", "application/xml")
        .header("X-Org-Id", org_id.to_string())
        .body(multi_decision_dmn())
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = resp.json().await.unwrap();
    let deployed = body["deployed"].as_array().unwrap();
    assert_eq!(deployed.len(), 2);

    let keys: Vec<&str> = deployed
        .iter()
        .map(|d| d["decision_key"].as_str().unwrap())
        .collect();
    assert!(keys.contains(&"decision-a"));
    assert!(keys.contains(&"decision-b"));
}

#[tokio::test]
async fn list_decisions_returns_latest_versions() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 2).await;
    let _process_group_id = groups[0];
    let client = reqwest::Client::new();

    // Deploy once, then again to bump version
    for _ in 0..2 {
        client
            .post(format!("{}/api/v1/decisions", app.address))
            .header("Content-Type", "application/xml")
            .header("X-Org-Id", org_id.to_string())
            .body(risk_check_dmn())
            .send()
            .await
            .unwrap();
    }

    let resp = client
        .get(format!("{}/api/v1/decisions", app.address))
        .header("X-Org-Id", org_id.to_string())
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    let list = body.as_array().unwrap();

    // Only one entry for risk-check (latest version = 2)
    let risk_check: Vec<&serde_json::Value> = list
        .iter()
        .filter(|d| d["decision_key"] == "risk-check")
        .collect();
    assert_eq!(risk_check.len(), 1);
    assert_eq!(risk_check[0]["version"], 2);
}

#[tokio::test]
async fn deploy_invalid_xml_returns_400() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 2).await;
    let _process_group_id = groups[0];
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{}/api/v1/decisions", app.address))
        .header("Content-Type", "application/xml")
        .header("X-Org-Id", org_id.to_string())
        .body("<invalid <<xml")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
}

// ── Engine integration ────────────────────────────────────────────────────────

async fn deploy_decision(app: &common::TestApp, org_id: Uuid, dmn_xml: &str) {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/api/v1/decisions", app.address))
        .header("Content-Type", "application/xml")
        .header("X-Org-Id", org_id.to_string())
        .body(dmn_xml.to_string())
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201, "decision deploy failed");
}

async fn deploy_bpmn(
    app: &common::TestApp,
    org_id: Uuid,
    process_group_id: Uuid,
    key: &str,
    bpmn: &str,
) -> serde_json::Value {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/api/v1/deployments", app.address))
        .json(&serde_json::json!({
            "org_id": org_id,
            "process_group_id": process_group_id,
            "key": key,
            "bpmn_xml": bpmn,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201, "BPMN deploy failed");
    resp.json().await.unwrap()
}

async fn start_instance(
    app: &common::TestApp,
    org_id: Uuid,
    def_id: Uuid,
    variables: serde_json::Value,
) -> serde_json::Value {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/api/v1/process-instances", app.address))
        .json(&serde_json::json!({
            "org_id": org_id,
            "definition_id": def_id,
            "variables": variables
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201, "start_instance failed");
    resp.json().await.unwrap()
}

#[tokio::test]
async fn engine_runs_business_rule_task() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 2).await;
    let process_group_id = groups[0];

    // Deploy the decision first
    deploy_decision(&app, org_id, &risk_check_dmn()).await;

    // Deploy the BPMN
    let key = unique_key("brt");
    let def = deploy_bpmn(&app, org_id, process_group_id, &key, &business_rule_bpmn()).await;
    let def_id: Uuid = def["id"].as_str().unwrap().parse().unwrap();

    // Start instance with inputs that match the "low" risk rule
    let instance = start_instance(
        &app,
        org_id,
        def_id,
        serde_json::json!([
            { "name": "age",          "value_type": "integer", "value": 25 },
            { "name": "credit_score", "value_type": "integer", "value": 750 }
        ]),
    )
    .await;
    let instance_id: Uuid = instance["id"].as_str().unwrap().parse().unwrap();

    // Engine should have evaluated the BusinessRuleTask and completed the instance.
    // Verify the output variable was written directly via the DB.
    let vars: Vec<(String, String, serde_json::Value)> = sqlx::query_as(
        "SELECT name, value_type, value FROM variables WHERE instance_id = $1 ORDER BY name",
    )
    .bind(instance_id)
    .fetch_all(&app.pool)
    .await
    .unwrap();

    let risk_level = vars
        .iter()
        .find(|(n, _, _)| n == "risk_level")
        .expect("risk_level variable should be set after BusinessRuleTask");
    assert_eq!(risk_level.2, serde_json::json!("low"));
}

#[test]
fn parser_accepts_business_rule_task() {
    use conduit::parser::{self, FlowNodeKind};

    let bpmn = business_rule_bpmn();
    let graph = parser::parse(&bpmn).unwrap();

    let brt = graph.nodes.get("task_risk").unwrap();
    match &brt.kind {
        FlowNodeKind::BusinessRuleTask { decision_ref } => {
            assert_eq!(decision_ref, "risk-check");
        }
        other => panic!("expected BusinessRuleTask, got: {other:?}"),
    }
}

#[tokio::test]
async fn engine_decision_not_found_errors_instance() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 2).await;
    let process_group_id = groups[0];

    // Deploy BPMN referencing "risk-check" but do NOT deploy the DMN
    let key = unique_key("brt-nofound");
    let def = deploy_bpmn(&app, org_id, process_group_id, &key, &business_rule_bpmn()).await;
    let def_id: Uuid = def["id"].as_str().unwrap().parse().unwrap();

    let instance = start_instance(
        &app,
        org_id,
        def_id,
        serde_json::json!([
            { "name": "age",          "value_type": "integer", "value": 25 },
            { "name": "credit_score", "value_type": "integer", "value": 750 }
        ]),
    )
    .await;
    let instance_id = instance["id"].as_str().unwrap();

    // Instance should be in a failed state
    let client = reqwest::Client::new();
    let resp = client
        .get(format!(
            "{}/api/v1/process-instances/{}",
            app.address, instance_id
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(
        body["state"], "error",
        "instance should be in error state when decision not found"
    );
}
