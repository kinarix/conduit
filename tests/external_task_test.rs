mod common;

use uuid::Uuid;

fn unique_key(prefix: &str) -> String {
    format!("{}.{}", prefix, Uuid::new_v4())
}

/// Start → ServiceTask(topic) → End
fn service_task_bpmn(topic: &str) -> String {
    format!(
        r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL">
  <process id="svc_proc">
    <startEvent id="start"/>
    <serviceTask id="svc1" name="Call API" topic="{topic}"/>
    <endEvent id="end"/>
    <sequenceFlow id="f1" sourceRef="start" targetRef="svc1"/>
    <sequenceFlow id="f2" sourceRef="svc1" targetRef="end"/>
  </process>
</definitions>"#
    )
}

async fn deploy_and_start(
    app: &common::TestApp,
    org_id: Uuid,
    process_group_id: Uuid,
    topic: &str,
) -> (Uuid, serde_json::Value) {
    let client = app.client.clone();
    let key = unique_key("ext");

    let deploy_resp = client
        .post(format!("{}/api/v1/deployments", app.address))
        .json(&serde_json::json!({
            "process_group_id": process_group_id,
            "key": key,
            "bpmn_xml": service_task_bpmn(topic)
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(deploy_resp.status(), 201, "deploy failed");
    let def: serde_json::Value = deploy_resp.json().await.unwrap();
    let def_id = Uuid::parse_str(def["id"].as_str().unwrap()).unwrap();

    let start_resp = client
        .post(format!("{}/api/v1/process-instances", app.address))
        .json(&serde_json::json!({ "org_id": org_id, "definition_id": def_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(start_resp.status(), 201, "start_instance failed");
    let instance: serde_json::Value = start_resp.json().await.unwrap();

    (def_id, instance)
}

// ── fetch-and-lock ────────────────────────────────────────────────────────────

#[tokio::test]
async fn fetch_and_lock_returns_pending_service_task() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 2).await;
    let process_group_id = groups[0];
    let client = app.client.clone();

    let topic = format!("payments-{}", Uuid::new_v4());
    let (_, instance) = deploy_and_start(&app, org_id, process_group_id, &topic).await;
    let instance_id = instance["id"].as_str().unwrap();

    let resp = client
        .post(format!(
            "{}/api/v1/external-tasks/fetch-and-lock",
            app.address
        ))
        .json(&serde_json::json!({
            "worker_id": "worker-1",
            "topic": topic,
            "lock_duration_secs": 30
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let jobs: serde_json::Value = resp.json().await.unwrap();
    assert!(!jobs.as_array().unwrap().is_empty());

    let job = jobs
        .as_array()
        .unwrap()
        .iter()
        .find(|j| j["instance_id"].as_str() == Some(instance_id))
        .expect("locked job for our instance not found");

    assert_eq!(job["topic"].as_str(), Some(topic.as_str()));
    assert!(job["locked_until"].is_string());
}

#[tokio::test]
async fn fetch_and_lock_by_topic_filters() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 2).await;
    let process_group_id = groups[0];
    let client = app.client.clone();

    let topic_other = format!("invoicing-{}", Uuid::new_v4());
    let topic_target = format!("email-sender-{}", Uuid::new_v4());
    deploy_and_start(&app, org_id, process_group_id, &topic_other).await;
    let (_, target) = deploy_and_start(&app, org_id, process_group_id, &topic_target).await;
    let target_id = target["id"].as_str().unwrap();

    let resp = client
        .post(format!(
            "{}/api/v1/external-tasks/fetch-and-lock",
            app.address
        ))
        .json(&serde_json::json!({
            "worker_id": "worker-2",
            "topic": topic_target,
            "lock_duration_secs": 30
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let jobs: serde_json::Value = resp.json().await.unwrap();
    let arr = jobs.as_array().unwrap();
    assert!(
        arr.iter()
            .all(|j| j["topic"].as_str() == Some(topic_target.as_str())),
        "all returned jobs must match the requested topic"
    );
    assert!(
        arr.iter()
            .any(|j| j["instance_id"].as_str() == Some(target_id)),
        "our email-sender instance should be in the result"
    );
}

#[tokio::test]
async fn fetch_and_lock_exclusive() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 2).await;
    let process_group_id = groups[0];
    let client = app.client.clone();

    let topic = format!("exclusive-{}", Uuid::new_v4());
    deploy_and_start(&app, org_id, process_group_id, &topic).await;

    let resp1 = client
        .post(format!(
            "{}/api/v1/external-tasks/fetch-and-lock",
            app.address
        ))
        .json(&serde_json::json!({
            "worker_id": "worker-a",
            "topic": topic,
            "max_jobs": 1,
            "lock_duration_secs": 60
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp1.status(), 200);
    let jobs1: serde_json::Value = resp1.json().await.unwrap();
    assert_eq!(jobs1.as_array().unwrap().len(), 1);

    // Second fetch by a different worker should return nothing for this topic
    let resp2 = client
        .post(format!(
            "{}/api/v1/external-tasks/fetch-and-lock",
            app.address
        ))
        .json(&serde_json::json!({
            "worker_id": "worker-b",
            "topic": topic,
            "max_jobs": 10,
            "lock_duration_secs": 60
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp2.status(), 200);
    let jobs2: serde_json::Value = resp2.json().await.unwrap();
    assert_eq!(
        jobs2.as_array().unwrap().len(),
        0,
        "locked job must not be returned to a second worker"
    );
}

// ── complete ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn complete_advances_token_to_end() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 2).await;
    let process_group_id = groups[0];
    let client = app.client.clone();

    let topic = format!("complete-{}", Uuid::new_v4());
    let (_, instance) = deploy_and_start(&app, org_id, process_group_id, &topic).await;
    let instance_id = instance["id"].as_str().unwrap();

    // Fetch and lock
    let lock_resp = client
        .post(format!(
            "{}/api/v1/external-tasks/fetch-and-lock",
            app.address
        ))
        .json(&serde_json::json!({
            "worker_id": "worker-1",
            "topic": topic,
            "lock_duration_secs": 30
        }))
        .send()
        .await
        .unwrap();
    let jobs: serde_json::Value = lock_resp.json().await.unwrap();
    let job_id = jobs
        .as_array()
        .unwrap()
        .iter()
        .find(|j| j["instance_id"].as_str() == Some(instance_id))
        .expect("our job not found")["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Complete
    let complete_resp = client
        .post(format!(
            "{}/api/v1/external-tasks/{}/complete",
            app.address, job_id
        ))
        .json(&serde_json::json!({ "worker_id": "worker-1" }))
        .send()
        .await
        .unwrap();
    assert_eq!(complete_resp.status(), 204);

    // Instance should now be completed
    let get_resp = client
        .get(format!(
            "{}/api/v1/process-instances/{}",
            app.address, instance_id
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(get_resp.status(), 200);
    let inst: serde_json::Value = get_resp.json().await.unwrap();
    assert_eq!(inst["state"].as_str(), Some("completed"));
}

#[tokio::test]
async fn complete_with_output_variables() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 2).await;
    let process_group_id = groups[0];
    let client = app.client.clone();

    let topic = format!("vars-{}", Uuid::new_v4());
    let (_, instance) = deploy_and_start(&app, org_id, process_group_id, &topic).await;
    let instance_id = instance["id"].as_str().unwrap();

    let lock_resp = client
        .post(format!(
            "{}/api/v1/external-tasks/fetch-and-lock",
            app.address
        ))
        .json(&serde_json::json!({
            "worker_id": "worker-1",
            "topic": topic,
            "lock_duration_secs": 30
        }))
        .send()
        .await
        .unwrap();
    let jobs: serde_json::Value = lock_resp.json().await.unwrap();
    let job_id = jobs
        .as_array()
        .unwrap()
        .iter()
        .find(|j| j["instance_id"].as_str() == Some(instance_id))
        .expect("our job not found")["id"]
        .as_str()
        .unwrap()
        .to_string();

    let complete_resp = client
        .post(format!(
            "{}/api/v1/external-tasks/{}/complete",
            app.address, job_id
        ))
        .json(&serde_json::json!({
            "worker_id": "worker-1",
            "variables": [
                { "name": "result", "value_type": "string", "value": "ok" },
                { "name": "score",  "value_type": "integer", "value": 42 }
            ]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(complete_resp.status(), 204);

    // Verify variables were written
    let vars: Vec<(String, String, serde_json::Value)> = sqlx::query_as(
        "SELECT name, value_type, value FROM variables WHERE instance_id = $1 ORDER BY name",
    )
    .bind(Uuid::parse_str(instance_id).unwrap())
    .fetch_all(&app.pool)
    .await
    .unwrap();

    assert_eq!(vars.len(), 2);
    let result_var = vars.iter().find(|(n, _, _)| n == "result").unwrap();
    assert_eq!(result_var.1, "string");
    assert_eq!(result_var.2, serde_json::json!("ok"));

    let score_var = vars.iter().find(|(n, _, _)| n == "score").unwrap();
    assert_eq!(score_var.1, "integer");
    assert_eq!(score_var.2, serde_json::json!(42));
}

#[tokio::test]
async fn complete_wrong_worker_returns_409() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 2).await;
    let process_group_id = groups[0];
    let client = app.client.clone();

    let topic = format!("wrong-worker-{}", Uuid::new_v4());
    let (_, instance) = deploy_and_start(&app, org_id, process_group_id, &topic).await;
    let instance_id = instance["id"].as_str().unwrap();

    let lock_resp = client
        .post(format!(
            "{}/api/v1/external-tasks/fetch-and-lock",
            app.address
        ))
        .json(&serde_json::json!({
            "worker_id": "worker-correct",
            "topic": topic,
            "lock_duration_secs": 60
        }))
        .send()
        .await
        .unwrap();
    let jobs: serde_json::Value = lock_resp.json().await.unwrap();
    let job_id = jobs
        .as_array()
        .unwrap()
        .iter()
        .find(|j| j["instance_id"].as_str() == Some(instance_id))
        .expect("our job not found")["id"]
        .as_str()
        .unwrap()
        .to_string();

    let resp = client
        .post(format!(
            "{}/api/v1/external-tasks/{}/complete",
            app.address, job_id
        ))
        .json(&serde_json::json!({ "worker_id": "worker-impostor" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 409);
}

// ── failure ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn failure_decrements_retries() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 2).await;
    let process_group_id = groups[0];
    let client = app.client.clone();

    let topic = format!("fail-retry-{}", Uuid::new_v4());
    let (_, instance) = deploy_and_start(&app, org_id, process_group_id, &topic).await;
    let instance_id = instance["id"].as_str().unwrap();

    let lock_resp = client
        .post(format!(
            "{}/api/v1/external-tasks/fetch-and-lock",
            app.address
        ))
        .json(&serde_json::json!({
            "worker_id": "worker-1",
            "topic": topic,
            "lock_duration_secs": 30
        }))
        .send()
        .await
        .unwrap();
    let jobs: serde_json::Value = lock_resp.json().await.unwrap();
    let job_id = jobs
        .as_array()
        .unwrap()
        .iter()
        .find(|j| j["instance_id"].as_str() == Some(instance_id))
        .expect("our job not found")["id"]
        .as_str()
        .unwrap()
        .to_string();

    let fail_resp = client
        .post(format!(
            "{}/api/v1/external-tasks/{}/failure",
            app.address, job_id
        ))
        .json(&serde_json::json!({
            "worker_id": "worker-1",
            "error_message": "connection timeout"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(fail_resp.status(), 204);

    // Job should be pending again (retries=3, retry_count=1 → still has retries left)
    let job: (String, i32) = sqlx::query_as("SELECT state, retry_count FROM jobs WHERE id = $1")
        .bind(Uuid::parse_str(&job_id).unwrap())
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(job.0, "pending");
    assert_eq!(job.1, 1);

    // Instance should still be running
    let inst_state: (String,) = sqlx::query_as("SELECT state FROM process_instances WHERE id = $1")
        .bind(Uuid::parse_str(instance_id).unwrap())
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(inst_state.0, "running");
}

#[tokio::test]
async fn failure_max_retries_marks_instance_error() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 2).await;
    let process_group_id = groups[0];
    let client = app.client.clone();

    let topic = format!("fail-max-{}", Uuid::new_v4());
    let (_, instance) = deploy_and_start(&app, org_id, process_group_id, &topic).await;
    let instance_id = instance["id"].as_str().unwrap();

    // Exhaust all retries (default retries = 3)
    for attempt in 0..3 {
        let lock_resp = client
            .post(format!(
                "{}/api/v1/external-tasks/fetch-and-lock",
                app.address
            ))
            .json(&serde_json::json!({
                "worker_id": "worker-1",
                "topic": topic,
                "lock_duration_secs": 30
            }))
            .send()
            .await
            .unwrap();
        let jobs: serde_json::Value = lock_resp.json().await.unwrap();
        let arr = jobs.as_array().unwrap();
        assert_eq!(arr.len(), 1, "attempt {attempt}: expected 1 job");
        let job_id = arr[0]["id"].as_str().unwrap();

        let fail_resp = client
            .post(format!(
                "{}/api/v1/external-tasks/{}/failure",
                app.address, job_id
            ))
            .json(&serde_json::json!({
                "worker_id": "worker-1",
                "error_message": "fatal error"
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(fail_resp.status(), 204, "attempt {attempt}");
    }

    // Instance should now be in error state
    let inst_state: (String,) = sqlx::query_as("SELECT state FROM process_instances WHERE id = $1")
        .bind(Uuid::parse_str(instance_id).unwrap())
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(inst_state.0, "error");
}

// ── extend-lock ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn extend_lock_updates_deadline() {
    let app = common::spawn_test_app().await;
    let (org_id, groups) = common::create_test_org_with_groups(&app, 2).await;
    let process_group_id = groups[0];
    let client = app.client.clone();

    let topic = format!("extend-{}", Uuid::new_v4());
    let (_, instance) = deploy_and_start(&app, org_id, process_group_id, &topic).await;
    let instance_id = instance["id"].as_str().unwrap();

    let lock_resp = client
        .post(format!(
            "{}/api/v1/external-tasks/fetch-and-lock",
            app.address
        ))
        .json(&serde_json::json!({
            "worker_id": "worker-1",
            "topic": topic,
            "lock_duration_secs": 10
        }))
        .send()
        .await
        .unwrap();
    let jobs: serde_json::Value = lock_resp.json().await.unwrap();
    let job = jobs
        .as_array()
        .unwrap()
        .iter()
        .find(|j| j["instance_id"].as_str() == Some(instance_id))
        .expect("our job not found");
    let job_id = job["id"].as_str().unwrap();
    let original_deadline = job["locked_until"].as_str().unwrap().to_string();

    let extend_resp = client
        .post(format!(
            "{}/api/v1/external-tasks/{}/extend-lock",
            app.address, job_id
        ))
        .json(&serde_json::json!({
            "worker_id": "worker-1",
            "lock_duration_secs": 300
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(extend_resp.status(), 204);

    // New deadline should be later than the original
    let new_deadline: (chrono::DateTime<chrono::Utc>,) =
        sqlx::query_as("SELECT locked_until FROM jobs WHERE id = $1")
            .bind(Uuid::parse_str(job_id).unwrap())
            .fetch_one(&app.pool)
            .await
            .unwrap();

    let orig_dt = chrono::DateTime::parse_from_rfc3339(&original_deadline)
        .unwrap()
        .with_timezone(&chrono::Utc);
    assert!(
        new_deadline.0 > orig_dt,
        "extended deadline should be later than original"
    );
}
