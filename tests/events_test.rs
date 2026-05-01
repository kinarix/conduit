mod common;

use uuid::Uuid;

fn unique_key(prefix: &str) -> String {
    format!("{}-{}", prefix, Uuid::new_v4())
}

fn user_task_bpmn() -> String {
    r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def1" targetNamespace="urn:test">
  <process id="proc" isExecutable="true">
    <startEvent id="start"/>
    <userTask id="task1" name="Approve"/>
    <endEvent id="end"/>
    <sequenceFlow id="sf1" sourceRef="start" targetRef="task1"/>
    <sequenceFlow id="sf2" sourceRef="task1" targetRef="end"/>
  </process>
</definitions>"#
        .to_string()
}

async fn deploy(
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

#[tokio::test]
async fn events_endpoint_records_lifecycle() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 1).await;
    let process_group_id = groups[0];
    let client = reqwest::Client::new();

    let def_id = deploy(
        &app,
        org_id,
        process_group_id,
        &unique_key("evt"),
        &user_task_bpmn(),
    )
    .await;

    // Start instance with one variable
    let resp = client
        .post(format!("{}/api/v1/process-instances", app.address))
        .json(&serde_json::json!({
            "org_id": org_id,
            "definition_id": def_id,
            "variables": [
                {"name": "amount", "value_type": "integer", "value": 100}
            ]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let inst: serde_json::Value = resp.json().await.unwrap();
    let inst_id = Uuid::parse_str(inst["id"].as_str().unwrap()).unwrap();

    // Find and complete the user task with an output variable
    let tasks_resp: serde_json::Value = client
        .get(format!("{}/api/v1/tasks", app.address))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let task_id = tasks_resp["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["instance_id"].as_str() == Some(&inst_id.to_string()))
        .expect("task for our instance")["id"]
        .as_str()
        .unwrap()
        .to_string();
    client
        .post(format!("{}/api/v1/tasks/{}/complete", app.address, task_id))
        .json(&serde_json::json!({
            "variables": [{"name": "amount", "value_type": "integer", "value": 250}]
        }))
        .send()
        .await
        .unwrap();

    // Fetch events
    let events: Vec<serde_json::Value> = client
        .get(format!(
            "{}/api/v1/process-instances/{}/events",
            app.address, inst_id
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let types: Vec<&str> = events
        .iter()
        .map(|e| e["event_type"].as_str().unwrap())
        .collect();

    // Element traversal recorded
    assert!(types.contains(&"element_entered"), "got: {:?}", types);
    assert!(types.contains(&"element_left"), "got: {:?}", types);

    // Both variable writes recorded (different execution scopes → both are `variable_set`).
    let var_writes: Vec<&serde_json::Value> = events
        .iter()
        .filter(|e| {
            (e["event_type"] == "variable_set" || e["event_type"] == "variable_changed")
                && e["payload"]["name"] == "amount"
        })
        .collect();
    assert_eq!(var_writes.len(), 2, "got events: {:?}", types);
    assert_eq!(var_writes[0]["payload"]["new_value"], 100);
    assert_eq!(var_writes[1]["payload"]["new_value"], 250);

    // element_entered for the user task carries an input snapshot.
    let task_entered = events
        .iter()
        .find(|e| e["event_type"] == "element_entered" && e["element_id"] == "task1")
        .expect("element_entered for task1");
    assert!(task_entered["payload"]["input_variables"].is_object());
}
