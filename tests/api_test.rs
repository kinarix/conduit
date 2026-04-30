mod common;

use uuid::Uuid;

fn unique_key(prefix: &str) -> String {
    format!("{}-{}", prefix, Uuid::new_v4())
}

fn linear_bpmn() -> String {
    r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def1" targetNamespace="urn:test">
  <process id="proc" isExecutable="true">
    <startEvent id="start"/>
    <userTask id="task1" name="Do the thing"/>
    <endEvent id="end"/>
    <sequenceFlow id="sf1" sourceRef="start" targetRef="task1"/>
    <sequenceFlow id="sf2" sourceRef="task1" targetRef="end"/>
  </process>
</definitions>"#
        .to_string()
}

fn start_to_end_bpmn() -> String {
    r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def1" targetNamespace="urn:test">
  <process id="proc" isExecutable="true">
    <startEvent id="start"/>
    <endEvent id="end"/>
    <sequenceFlow id="sf1" sourceRef="start" targetRef="end"/>
  </process>
</definitions>"#
        .to_string()
}

async fn deploy_definition(
    app: &common::TestApp,
    org_id: Uuid,
    process_group_id: Uuid,
    key: &str,
    bpmn: &str,
) -> Uuid {
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
    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = resp.json().await.unwrap();
    Uuid::parse_str(body["id"].as_str().unwrap()).unwrap()
}

// ─── POST /api/v1/process-instances ──────────────────────────────────────────

#[tokio::test]
async fn start_instance_returns_201() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 2).await;
    let process_group_id = groups[0];
    let client = reqwest::Client::new();

    let def_id = deploy_definition(&app, org_id, process_group_id, &unique_key("inst"), &linear_bpmn()).await;

    let resp = client
        .post(format!("{}/api/v1/process-instances", app.address))
        .json(&serde_json::json!({ "org_id": org_id, "definition_id": def_id }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 201);

    let body: serde_json::Value = resp.json().await.unwrap();
    Uuid::parse_str(body["id"].as_str().unwrap()).expect("id is not a UUID");
    assert_eq!(body["definition_id"], def_id.to_string());
    assert_eq!(body["state"], "running");
    assert!(body["started_at"].is_string());
    assert!(body["ended_at"].is_null());
}

#[tokio::test]
async fn start_instance_start_to_end_returns_completed() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 2).await;
    let process_group_id = groups[0];
    let client = reqwest::Client::new();

    let def_id =
        deploy_definition(&app, org_id, process_group_id, &unique_key("inst-end"), &start_to_end_bpmn()).await;

    let resp = client
        .post(format!("{}/api/v1/process-instances", app.address))
        .json(&serde_json::json!({ "org_id": org_id, "definition_id": def_id }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["state"], "completed");
    assert!(body["ended_at"].is_string());
}

#[tokio::test]
async fn start_instance_unknown_definition_returns_404() {
    let app = common::spawn_test_app().await;
    let (org_id, _groups) = common::create_test_org_with_groups(&app, 2).await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{}/api/v1/process-instances", app.address))
        .json(&serde_json::json!({ "org_id": org_id, "definition_id": Uuid::new_v4() }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 404);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["error"].is_string());
}

#[tokio::test]
async fn start_instance_missing_definition_id_returns_422() {
    let app = common::spawn_test_app().await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{}/api/v1/process-instances", app.address))
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 422);
}

// ─── GET /api/v1/process-instances/:id ───────────────────────────────────────

#[tokio::test]
async fn get_instance_returns_200() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 2).await;
    let process_group_id = groups[0];
    let client = reqwest::Client::new();

    let def_id = deploy_definition(&app, org_id, process_group_id, &unique_key("get-inst"), &linear_bpmn()).await;

    let start_resp = client
        .post(format!("{}/api/v1/process-instances", app.address))
        .json(&serde_json::json!({ "org_id": org_id, "definition_id": def_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(start_resp.status(), 201);
    let start_body: serde_json::Value = start_resp.json().await.unwrap();
    let instance_id = start_body["id"].as_str().unwrap();

    let get_resp = client
        .get(format!(
            "{}/api/v1/process-instances/{}",
            app.address, instance_id
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(get_resp.status(), 200);
    let get_body: serde_json::Value = get_resp.json().await.unwrap();
    assert_eq!(get_body["id"], instance_id);
    assert_eq!(get_body["state"], "running");
}

#[tokio::test]
async fn get_instance_not_found_returns_404() {
    let app = common::spawn_test_app().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!(
            "{}/api/v1/process-instances/{}",
            app.address,
            Uuid::new_v4()
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 404);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["error"].is_string());
}

// ─── GET /api/v1/tasks ───────────────────────────────────────────────────────

#[tokio::test]
async fn list_tasks_returns_pending_task() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 2).await;
    let process_group_id = groups[0];
    let client = reqwest::Client::new();

    let def_id = deploy_definition(&app, org_id, process_group_id, &unique_key("list-tasks"), &linear_bpmn()).await;
    let start_resp = client
        .post(format!("{}/api/v1/process-instances", app.address))
        .json(&serde_json::json!({ "org_id": org_id, "definition_id": def_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(start_resp.status(), 201);
    let start_body: serde_json::Value = start_resp.json().await.unwrap();
    let instance_id = start_body["id"].as_str().unwrap();

    let resp = client
        .get(format!("{}/api/v1/tasks", app.address))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    let items = body["items"].as_array().unwrap();

    let task = items
        .iter()
        .find(|t| t["instance_id"] == instance_id)
        .expect("should find a task for the started instance");

    assert_eq!(task["state"], "pending");
    assert_eq!(task["task_type"], "user_task");
    assert_eq!(task["element_id"], "task1");
}

// ─── GET /api/v1/tasks/:id ────────────────────────────────────────────────────

#[tokio::test]
async fn get_task_returns_200() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 2).await;
    let process_group_id = groups[0];
    let client = reqwest::Client::new();

    let def_id = deploy_definition(&app, org_id, process_group_id, &unique_key("get-task"), &linear_bpmn()).await;
    let start_resp = client
        .post(format!("{}/api/v1/process-instances", app.address))
        .json(&serde_json::json!({ "org_id": org_id, "definition_id": def_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(start_resp.status(), 201);
    let start_body: serde_json::Value = start_resp.json().await.unwrap();
    let instance_id = start_body["id"].as_str().unwrap();

    let list_resp = client
        .get(format!("{}/api/v1/tasks", app.address))
        .send()
        .await
        .unwrap();
    let list_body: serde_json::Value = list_resp.json().await.unwrap();
    let task_id = list_body["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["instance_id"] == instance_id)
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let resp = client
        .get(format!("{}/api/v1/tasks/{}", app.address, task_id))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["id"], task_id);
    assert_eq!(body["state"], "pending");
}

#[tokio::test]
async fn get_task_not_found_returns_404() {
    let app = common::spawn_test_app().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{}/api/v1/tasks/{}", app.address, Uuid::new_v4()))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 404);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["error"].is_string());
}

// ─── POST /api/v1/tasks/:id/complete ─────────────────────────────────────────

#[tokio::test]
async fn complete_task_returns_204() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 2).await;
    let process_group_id = groups[0];
    let client = reqwest::Client::new();

    let def_id =
        deploy_definition(&app, org_id, process_group_id, &unique_key("complete-task"), &linear_bpmn()).await;
    let start_resp = client
        .post(format!("{}/api/v1/process-instances", app.address))
        .json(&serde_json::json!({ "org_id": org_id, "definition_id": def_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(start_resp.status(), 201);
    let start_body: serde_json::Value = start_resp.json().await.unwrap();
    let instance_id = start_body["id"].as_str().unwrap();

    let list_resp = client
        .get(format!("{}/api/v1/tasks", app.address))
        .send()
        .await
        .unwrap();
    let list_body: serde_json::Value = list_resp.json().await.unwrap();
    let task_id = list_body["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["instance_id"] == instance_id)
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let resp = client
        .post(format!("{}/api/v1/tasks/{}/complete", app.address, task_id))
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn complete_task_advances_instance_to_completed() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 2).await;
    let process_group_id = groups[0];
    let client = reqwest::Client::new();

    let def_id = deploy_definition(&app, org_id, process_group_id, &unique_key("advance"), &linear_bpmn()).await;
    let start_resp = client
        .post(format!("{}/api/v1/process-instances", app.address))
        .json(&serde_json::json!({ "org_id": org_id, "definition_id": def_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(start_resp.status(), 201);
    let start_body: serde_json::Value = start_resp.json().await.unwrap();
    let instance_id = start_body["id"].as_str().unwrap().to_string();

    let list_resp = client
        .get(format!("{}/api/v1/tasks", app.address))
        .send()
        .await
        .unwrap();
    let list_body: serde_json::Value = list_resp.json().await.unwrap();
    let task_id = list_body["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["instance_id"] == instance_id)
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    client
        .post(format!("{}/api/v1/tasks/{}/complete", app.address, task_id))
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();

    let get_resp = client
        .get(format!(
            "{}/api/v1/process-instances/{}",
            app.address, instance_id
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(get_resp.status(), 200);
    let get_body: serde_json::Value = get_resp.json().await.unwrap();
    assert_eq!(get_body["state"], "completed");
    assert!(get_body["ended_at"].is_string());
}

#[tokio::test]
async fn complete_task_not_found_returns_404() {
    let app = common::spawn_test_app().await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!(
            "{}/api/v1/tasks/{}/complete",
            app.address,
            Uuid::new_v4()
        ))
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 404);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["error"].is_string());
}

#[tokio::test]
async fn complete_already_completed_task_returns_409() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 2).await;
    let process_group_id = groups[0];
    let client = reqwest::Client::new();

    let def_id = deploy_definition(&app, org_id, process_group_id, &unique_key("conflict"), &linear_bpmn()).await;
    let start_resp = client
        .post(format!("{}/api/v1/process-instances", app.address))
        .json(&serde_json::json!({ "org_id": org_id, "definition_id": def_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(start_resp.status(), 201);
    let start_body: serde_json::Value = start_resp.json().await.unwrap();
    let instance_id = start_body["id"].as_str().unwrap();

    let list_resp = client
        .get(format!("{}/api/v1/tasks", app.address))
        .send()
        .await
        .unwrap();
    let list_body: serde_json::Value = list_resp.json().await.unwrap();
    let task_id = list_body["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["instance_id"] == instance_id)
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let r1 = client
        .post(format!("{}/api/v1/tasks/{}/complete", app.address, task_id))
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(r1.status(), 204);

    let r2 = client
        .post(format!("{}/api/v1/tasks/{}/complete", app.address, task_id))
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(r2.status(), 409);
    let body: serde_json::Value = r2.json().await.unwrap();
    assert!(body["error"].is_string());
}

// ─── Labels roundtrip ─────────────────────────────────────────────────────────

#[tokio::test]
async fn deploy_with_labels_roundtrips_on_get_instance() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 2).await;
    let process_group_id = groups[0];
    let client = reqwest::Client::new();

    let deploy_resp = client
        .post(format!("{}/api/v1/deployments", app.address))
        .json(&serde_json::json!({
            "org_id": org_id,
            "process_group_id": process_group_id,
            "key": unique_key("labels-def"),
            "bpmn_xml": start_to_end_bpmn(),
            "labels": { "env": "test", "team": "platform" }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(deploy_resp.status(), 201);
    let def_body: serde_json::Value = deploy_resp.json().await.unwrap();
    let def_id = Uuid::parse_str(def_body["id"].as_str().unwrap()).unwrap();

    let inst_resp = client
        .post(format!("{}/api/v1/process-instances", app.address))
        .json(&serde_json::json!({
            "org_id": org_id,
            "definition_id": def_id,
            "labels": { "customer": "acme", "priority": "high" }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(inst_resp.status(), 201);
    let inst_body: serde_json::Value = inst_resp.json().await.unwrap();
    let instance_id = inst_body["id"].as_str().unwrap();

    assert_eq!(inst_body["labels"]["customer"], "acme");
    assert_eq!(inst_body["labels"]["priority"], "high");

    let get_resp = client
        .get(format!(
            "{}/api/v1/process-instances/{}",
            app.address, instance_id
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(get_resp.status(), 200);
    let get_body: serde_json::Value = get_resp.json().await.unwrap();
    assert_eq!(get_body["labels"]["customer"], "acme");
    assert_eq!(get_body["labels"]["priority"], "high");
}

#[tokio::test]
async fn start_instance_default_labels_is_empty_object() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 2).await;
    let process_group_id = groups[0];
    let client = reqwest::Client::new();

    let def_id =
        deploy_definition(&app, org_id, process_group_id, &unique_key("no-labels"), &start_to_end_bpmn()).await;

    let resp = client
        .post(format!("{}/api/v1/process-instances", app.address))
        .json(&serde_json::json!({ "org_id": org_id, "definition_id": def_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["labels"].is_object());
    assert_eq!(body["labels"].as_object().unwrap().len(), 0);
}
