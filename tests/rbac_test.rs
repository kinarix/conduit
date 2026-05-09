//! Phase 23 — RBAC permission matrix tests.
//!
//! Verifies that each built-in role (Admin, Deployer, Operator, Reader) has
//! exactly the permissions the spec says it has, and that users with no role
//! are denied every write endpoint. Role management endpoints (list, assign,
//! revoke) are also covered, plus cross-org isolation for role operations.

mod common;

use serde_json::json;
use uuid::Uuid;

use common::auth::{authed_client, mint_jwt};
use common::TestPrincipal;

// ─── Fixture helpers ──────────────────────────────────────────────────────────

fn minimal_bpmn(process_id: &str) -> String {
    format!(
        r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL">
  <process id="{process_id}" isExecutable="true">
    <startEvent id="start"/>
    <endEvent id="end"/>
    <sequenceFlow id="f1" sourceRef="start" targetRef="end"/>
  </process>
</definitions>"#
    )
}

fn usertask_bpmn(process_id: &str) -> String {
    format!(
        r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL">
  <process id="{process_id}" isExecutable="true">
    <startEvent id="start"/>
    <userTask id="t1"/>
    <endEvent id="end"/>
    <sequenceFlow id="f1" sourceRef="start" targetRef="t1"/>
    <sequenceFlow id="f2" sourceRef="t1" targetRef="end"/>
  </process>
</definitions>"#
    )
}

/// Add a user to the default test org with a specific built-in role.
async fn user_with_role(
    app: &common::TestApp,
    role_name: &str,
) -> (TestPrincipal, reqwest::Client) {
    let email = format!("rbac-{}-{}@test.local", role_name.to_lowercase(), Uuid::new_v4());
    let user = conduit::db::users::insert(
        &app.pool,
        app.principal.org_id,
        "internal",
        None,
        &email,
        None,
    )
    .await
    .unwrap();
    conduit::db::roles::assign_role(&app.pool, user.id, role_name, app.principal.user_id)
        .await
        .unwrap();
    let token = mint_jwt(user.id, app.principal.org_id);
    let client = authed_client(&token);
    let principal = TestPrincipal {
        user_id: user.id,
        org_id: app.principal.org_id,
        token,
    };
    (principal, client)
}

/// Add a user to the default test org with NO role.
async fn user_no_role(app: &common::TestApp) -> (TestPrincipal, reqwest::Client) {
    let email = format!("norole-{}@test.local", Uuid::new_v4());
    let user = conduit::db::users::insert(
        &app.pool,
        app.principal.org_id,
        "internal",
        None,
        &email,
        None,
    )
    .await
    .unwrap();
    let token = mint_jwt(user.id, app.principal.org_id);
    let client = authed_client(&token);
    let principal = TestPrincipal {
        user_id: user.id,
        org_id: app.principal.org_id,
        token,
    };
    (principal, client)
}

// ─── No-role: 403 on every write endpoint ─────────────────────────────────────

#[tokio::test]
async fn no_role_cannot_deploy() {
    let app = common::spawn_test_app().await;
    let group_id =
        common::create_test_process_group(&app, app.principal.org_id, "norole-deploy-g").await;
    let (_, client) = user_no_role(&app).await;

    let key = format!("norole-{}", Uuid::new_v4());
    let resp = client
        .post(format!("{}/api/v1/deployments", app.address))
        .json(&json!({
            "process_group_id": group_id,
            "key": key,
            "bpmn_xml": minimal_bpmn(&key),
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["code"], "U403");
}

#[tokio::test]
async fn no_role_cannot_start_instance() {
    let app = common::spawn_test_app().await;
    let (_, client) = user_no_role(&app).await;

    // Permission check fires before the definition lookup, so a fake UUID is fine.
    let resp = client
        .post(format!("{}/api/v1/process-instances", app.address))
        .json(&json!({ "definition_id": Uuid::new_v4() }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["code"], "U403");
}

#[tokio::test]
async fn no_role_cannot_create_user() {
    let app = common::spawn_test_app().await;
    let (_, client) = user_no_role(&app).await;

    let resp = client
        .post(format!("{}/api/v1/users", app.address))
        .json(&json!({ "email": "x@test.local", "auth_provider": "internal", "password": "hunter2!" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
}

// ─── Deployer role ────────────────────────────────────────────────────────────

#[tokio::test]
async fn deployer_can_deploy_and_start_instance() {
    let app = common::spawn_test_app().await;
    let group_id =
        common::create_test_process_group(&app, app.principal.org_id, "deployer-g").await;
    let (_, client) = user_with_role(&app, "Deployer").await;

    let key = format!("deployer-{}", Uuid::new_v4());
    let resp = client
        .post(format!("{}/api/v1/deployments", app.address))
        .json(&json!({
            "process_group_id": group_id,
            "key": &key,
            "bpmn_xml": minimal_bpmn(&key),
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let def_id = resp.json::<serde_json::Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let resp = client
        .post(format!("{}/api/v1/process-instances", app.address))
        .json(&json!({ "definition_id": def_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
}

#[tokio::test]
async fn deployer_cannot_manage_users() {
    let app = common::spawn_test_app().await;
    let (_, client) = user_with_role(&app, "Deployer").await;

    let resp = client
        .post(format!("{}/api/v1/users", app.address))
        .json(&json!({ "email": "x@test.local", "auth_provider": "internal", "password": "hunter2!" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn deployer_cannot_manage_secrets() {
    let app = common::spawn_test_app().await;
    let (_, client) = user_with_role(&app, "Deployer").await;

    let resp = client
        .post(format!(
            "{}/api/v1/orgs/{}/secrets",
            app.address, app.principal.org_id
        ))
        .json(&json!({ "name": "secret-key", "value": "v" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
}

// ─── Operator role ────────────────────────────────────────────────────────────

#[tokio::test]
async fn operator_can_start_instance() {
    let app = common::spawn_test_app().await;
    let group_id =
        common::create_test_process_group(&app, app.principal.org_id, "operator-start-g").await;
    let (_, operator_client) = user_with_role(&app, "Operator").await;

    // Admin deploys the process.
    let key = format!("operator-start-{}", Uuid::new_v4());
    let resp = app
        .client
        .post(format!("{}/api/v1/deployments", app.address))
        .json(&json!({
            "process_group_id": group_id,
            "key": &key,
            "bpmn_xml": minimal_bpmn(&key),
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let def_id = resp.json::<serde_json::Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Operator can start an instance of it.
    let resp = operator_client
        .post(format!("{}/api/v1/process-instances", app.address))
        .json(&json!({ "definition_id": def_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
}

#[tokio::test]
async fn operator_cannot_deploy() {
    let app = common::spawn_test_app().await;
    let group_id =
        common::create_test_process_group(&app, app.principal.org_id, "operator-nodeploy-g").await;
    let (_, client) = user_with_role(&app, "Operator").await;

    let key = format!("operator-nodeploy-{}", Uuid::new_v4());
    let resp = client
        .post(format!("{}/api/v1/deployments", app.address))
        .json(&json!({
            "process_group_id": group_id,
            "key": &key,
            "bpmn_xml": minimal_bpmn(&key),
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn operator_can_complete_user_task() {
    let app = common::spawn_test_app().await;
    let group_id =
        common::create_test_process_group(&app, app.principal.org_id, "operator-task-g").await;
    let (_, operator_client) = user_with_role(&app, "Operator").await;

    // Admin deploys a process with a userTask.
    let key = format!("operator-task-{}", Uuid::new_v4());
    let resp = app
        .client
        .post(format!("{}/api/v1/deployments", app.address))
        .json(&json!({
            "process_group_id": group_id,
            "key": &key,
            "bpmn_xml": usertask_bpmn(&key),
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let def_id = resp.json::<serde_json::Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Admin starts an instance — execution pauses at the userTask.
    let resp = app
        .client
        .post(format!("{}/api/v1/process-instances", app.address))
        .json(&json!({ "definition_id": def_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);

    // List pending tasks (no permission required, any authenticated user).
    let resp = app
        .client
        .get(format!("{}/api/v1/tasks", app.address))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let tasks_body: serde_json::Value = resp.json().await.unwrap();
    let task_id = tasks_body["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["definition_key"].as_str() == Some(&key))
        .or_else(|| tasks_body["items"].as_array().unwrap().first())
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Operator completes the task.
    let resp = operator_client
        .post(format!("{}/api/v1/tasks/{}/complete", app.address, task_id))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);
}

// ─── Reader role ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn reader_can_list_instances() {
    let app = common::spawn_test_app().await;
    let (_, client) = user_with_role(&app, "Reader").await;

    let resp = client
        .get(format!("{}/api/v1/process-instances", app.address))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn reader_cannot_deploy() {
    let app = common::spawn_test_app().await;
    let group_id =
        common::create_test_process_group(&app, app.principal.org_id, "reader-nodeploy-g").await;
    let (_, client) = user_with_role(&app, "Reader").await;

    let key = format!("reader-{}", Uuid::new_v4());
    let resp = client
        .post(format!("{}/api/v1/deployments", app.address))
        .json(&json!({
            "process_group_id": group_id,
            "key": &key,
            "bpmn_xml": minimal_bpmn(&key),
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn reader_cannot_start_instance() {
    let app = common::spawn_test_app().await;
    let (_, client) = user_with_role(&app, "Reader").await;

    let resp = client
        .post(format!("{}/api/v1/process-instances", app.address))
        .json(&json!({ "definition_id": Uuid::new_v4() }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
}

// ─── Role management endpoints ────────────────────────────────────────────────

#[tokio::test]
async fn any_authenticated_user_can_list_global_roles() {
    let app = common::spawn_test_app().await;
    let (_, client) = user_no_role(&app).await;

    let resp = client
        .get(format!("{}/api/v1/roles", app.address))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let roles: Vec<serde_json::Value> = resp.json().await.unwrap();
    let names: Vec<&str> = roles.iter().map(|r| r["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"Admin"), "Admin role must be listed");
    assert!(names.contains(&"Deployer"), "Deployer role must be listed");
    assert!(names.contains(&"Operator"), "Operator role must be listed");
    assert!(names.contains(&"Reader"), "Reader role must be listed");
}

#[tokio::test]
async fn role_manage_required_to_list_user_roles() {
    let app = common::spawn_test_app().await;
    let (target, _) = user_no_role(&app).await;
    let (_, deployer_client) = user_with_role(&app, "Deployer").await;

    let resp = deployer_client
        .get(format!(
            "{}/api/v1/users/{}/roles",
            app.address, target.user_id
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn role_manage_required_to_assign_role() {
    let app = common::spawn_test_app().await;
    let (target, _) = user_no_role(&app).await;
    let (_, operator_client) = user_with_role(&app, "Operator").await;

    let resp = operator_client
        .post(format!(
            "{}/api/v1/users/{}/roles",
            app.address, target.user_id
        ))
        .json(&json!({ "role_name": "Reader" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn admin_can_assign_list_and_revoke_roles() {
    let app = common::spawn_test_app().await;
    let (target, _) = user_no_role(&app).await;

    // Assign Reader role via Admin client.
    let resp = app
        .client
        .post(format!(
            "{}/api/v1/users/{}/roles",
            app.address, target.user_id
        ))
        .json(&json!({ "role_name": "Reader" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204, "assign Reader → 204");

    // List roles: should see Reader.
    let resp = app
        .client
        .get(format!(
            "{}/api/v1/users/{}/roles",
            app.address, target.user_id
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let rows: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["role_name"], "Reader");
    let role_id = rows[0]["role_id"].as_str().unwrap().to_string();

    // Revoke.
    let resp = app
        .client
        .delete(format!(
            "{}/api/v1/users/{}/roles/{}",
            app.address, target.user_id, role_id
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204, "revoke Reader → 204");

    // List again: should be empty.
    let resp = app
        .client
        .get(format!(
            "{}/api/v1/users/{}/roles",
            app.address, target.user_id
        ))
        .send()
        .await
        .unwrap();
    let rows: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert!(rows.is_empty(), "no roles after revoke");
}

#[tokio::test]
async fn assign_nonexistent_role_returns_404() {
    let app = common::spawn_test_app().await;
    let (target, _) = user_no_role(&app).await;

    let resp = app
        .client
        .post(format!(
            "{}/api/v1/users/{}/roles",
            app.address, target.user_id
        ))
        .json(&json!({ "role_name": "DoesNotExist" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn revoke_nonexistent_assignment_returns_404() {
    let app = common::spawn_test_app().await;
    let (target, _) = user_no_role(&app).await;

    let resp = app
        .client
        .delete(format!(
            "{}/api/v1/users/{}/roles/{}",
            app.address,
            target.user_id,
            Uuid::new_v4()
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

// ─── Cross-org isolation ──────────────────────────────────────────────────────

#[tokio::test]
async fn cross_org_role_assign_returns_404() {
    let app = common::spawn_test_app().await;
    // principal_b is Admin in a completely DIFFERENT org.
    let principal_b = common::create_principal(&app.pool, "role-iso-b").await;

    // Org A's Admin tries to assign a role to org B's user — must 404, not 403,
    // so the existence of users in other orgs is not leaked.
    let resp = app
        .client
        .post(format!(
            "{}/api/v1/users/{}/roles",
            app.address, principal_b.user_id
        ))
        .json(&json!({ "role_name": "Reader" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404, "cross-org role assign must 404");
}

#[tokio::test]
async fn cross_org_role_list_returns_404() {
    let app = common::spawn_test_app().await;
    let principal_b = common::create_principal(&app.pool, "role-iso-c").await;
    let client_b = authed_client(&principal_b.token);

    // Org B's Admin tries to list roles of org A's user — must 404.
    let resp = client_b
        .get(format!(
            "{}/api/v1/users/{}/roles",
            app.address, app.principal.user_id
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404, "cross-org role list must 404");
}
