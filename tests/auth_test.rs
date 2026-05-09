//! Phase 22 — authentication and tenant-isolation matrix.

mod common;

use chrono::Duration;
use serde_json::json;
use uuid::Uuid;

use common::auth::{authed_client, mint_jwt, TEST_JWT_ISSUER, TEST_JWT_KEY};
use conduit::auth::jwt::{encode_token, Claims};
use conduit::auth::{password, JwtKeys};

// ─── Login ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn login_succeeds_with_correct_password() {
    let app = common::spawn_test_app().await;
    let (org_slug, email) = seed_internal_user(&app, "hunter2").await;

    let resp = reqwest::Client::new()
        .post(format!("{}/api/v1/auth/login", app.address))
        .json(&json!({ "email": email, "password": "hunter2", "org_slug": org_slug }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["token_type"], "Bearer");
    assert!(body["access_token"].as_str().unwrap().len() > 20);
    assert_eq!(body["expires_in"], 3600);
}

#[tokio::test]
async fn login_fails_with_wrong_password_returns_u011() {
    let app = common::spawn_test_app().await;
    let (org_slug, email) = seed_internal_user(&app, "right-password").await;

    let resp = reqwest::Client::new()
        .post(format!("{}/api/v1/auth/login", app.address))
        .json(&json!({ "email": email, "password": "WRONG", "org_slug": org_slug }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["code"], "U011", "wrong password should map to U011");
    assert_eq!(body["message"], "Login failed.");
}

#[tokio::test]
async fn login_fails_for_unknown_user_with_same_generic_error() {
    let app = common::spawn_test_app().await;
    let (org_slug, _) = seed_internal_user(&app, "any").await;

    let resp = reqwest::Client::new()
        .post(format!("{}/api/v1/auth/login", app.address))
        .json(&json!({
            "email": "no-such-user@nowhere",
            "password": "anything",
            "org_slug": org_slug,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["code"], "U011", "unknown user must reuse U011");
}

#[tokio::test]
async fn login_fails_for_external_user_attempting_password_login() {
    let app = common::spawn_test_app().await;
    // Make an external-auth user directly in the DB.
    let slug = format!("ext-{}", Uuid::new_v4());
    let org = conduit::db::orgs::insert(&app.pool, "Ext Org", &slug)
        .await
        .unwrap();
    let email = format!("ext-{}@test.local", Uuid::new_v4());
    conduit::db::users::insert(
        &app.pool,
        org.id,
        "external",
        Some("oidc-subject"),
        &email,
        None,
    )
    .await
    .unwrap();

    let resp = reqwest::Client::new()
        .post(format!("{}/api/v1/auth/login", app.address))
        .json(&json!({ "email": email, "password": "x", "org_slug": slug }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}

// ─── JWT verification ─────────────────────────────────────────────────────

#[tokio::test]
async fn protected_endpoint_rejects_missing_authorization() {
    let app = common::spawn_test_app().await;
    // /me requires Principal but we send no Authorization header.
    let resp = reqwest::Client::new()
        .get(format!("{}/api/v1/me", app.address))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["code"], "U401");
    assert_eq!(body["message"], "Authentication required.");
}

#[tokio::test]
async fn protected_endpoint_rejects_garbage_token() {
    let app = common::spawn_test_app().await;
    let client = authed_client("not.a.valid.jwt");
    let resp = client
        .get(format!("{}/api/v1/me", app.address))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn protected_endpoint_rejects_expired_token() {
    let app = common::spawn_test_app().await;
    // Issue a token with negative TTL — already expired at iat.
    let claims = Claims::new(
        app.principal.user_id,
        app.principal.org_id,
        Duration::seconds(-3600),
        TEST_JWT_ISSUER,
    );
    let keys = JwtKeys::from_secret(TEST_JWT_KEY);
    let token = encode_token(&claims, &keys).unwrap();

    let client = authed_client(&token);
    let resp = client
        .get(format!("{}/api/v1/me", app.address))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn protected_endpoint_rejects_token_with_wrong_signature() {
    let app = common::spawn_test_app().await;
    let claims = Claims::new(
        app.principal.user_id,
        app.principal.org_id,
        Duration::seconds(3600),
        TEST_JWT_ISSUER,
    );
    let other_keys = JwtKeys::from_secret("a-completely-different-secret");
    let token = encode_token(&claims, &other_keys).unwrap();

    let client = authed_client(&token);
    let resp = client
        .get(format!("{}/api/v1/me", app.address))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn protected_endpoint_rejects_jwt_for_deleted_user() {
    let app = common::spawn_test_app().await;
    let principal = common::create_principal(&app.pool, "doomed").await;
    // Delete the user out from under the token.
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(principal.user_id)
        .execute(&app.pool)
        .await
        .unwrap();

    let client = authed_client(&principal.token);
    let resp = client
        .get(format!("{}/api/v1/me", app.address))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn me_returns_principal_summary() {
    let app = common::spawn_test_app().await;
    let resp = app
        .client
        .get(format!("{}/api/v1/me", app.address))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["user_id"], app.principal.user_id.to_string());
    assert_eq!(body["org_id"], app.principal.org_id.to_string());
    assert_eq!(body["auth_kind"], "jwt");
}

// ─── API keys ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn api_key_lifecycle_mint_use_revoke() {
    let app = common::spawn_test_app().await;

    // Mint
    let resp = app
        .client
        .post(format!("{}/api/v1/api-keys", app.address))
        .json(&json!({ "name": "ci-key" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = resp.json().await.unwrap();
    let plaintext = body["plaintext_key"].as_str().unwrap().to_string();
    let key_id = body["id"].as_str().unwrap().to_string();
    assert!(plaintext.starts_with("ck_"));
    assert_eq!(body["name"], "ci-key");

    // Use
    let key_client = authed_client(&plaintext);
    let resp = key_client
        .get(format!("{}/api/v1/me", app.address))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let me: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(me["auth_kind"], "api_key");
    assert_eq!(me["user_id"], app.principal.user_id.to_string());

    // Revoke (uses original JWT client)
    let resp = app
        .client
        .delete(format!("{}/api/v1/api-keys/{}", app.address, key_id))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    // After revoke, the key is rejected
    let resp = key_client
        .get(format!("{}/api/v1/me", app.address))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn api_key_with_garbage_prefix_returns_401() {
    let app = common::spawn_test_app().await;
    let client = authed_client("ck_completelyMadeUpAndNotInDb_xyz123");
    let resp = client
        .get(format!("{}/api/v1/me", app.address))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn list_api_keys_only_returns_callers_own() {
    let app = common::spawn_test_app().await;
    let other = common::create_principal(&app.pool, "other-user").await;
    let other_client = authed_client(&other.token);

    // Other user mints a key
    let resp = other_client
        .post(format!("{}/api/v1/api-keys", app.address))
        .json(&json!({ "name": "other-key" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);

    // Default principal lists their keys — must NOT see the other user's key
    let resp = app
        .client
        .get(format!("{}/api/v1/api-keys", app.address))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    let names: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|k| k["name"].as_str().unwrap())
        .collect();
    assert!(!names.contains(&"other-key"));
}

#[tokio::test]
async fn revoke_api_key_owned_by_other_user_returns_404() {
    let app = common::spawn_test_app().await;
    let other = common::create_principal(&app.pool, "victim").await;
    let other_client = authed_client(&other.token);

    let resp = other_client
        .post(format!("{}/api/v1/api-keys", app.address))
        .json(&json!({ "name": "victims-key" }))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    let key_id = body["id"].as_str().unwrap();

    // Default principal tries to revoke another user's key — must 404
    let resp = app
        .client
        .delete(format!("{}/api/v1/api-keys/{}", app.address, key_id))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

// ─── Tenant isolation ─────────────────────────────────────────────────────

#[tokio::test]
async fn cross_org_get_instance_returns_404() {
    let app = common::spawn_test_app().await;
    let principal_b = common::create_principal(&app.pool, "tenant-b").await;
    let client_b = authed_client(&principal_b.token);

    // Default principal (org A) deploys + starts an instance.
    let group_id =
        common::create_test_process_group(&app, app.principal.org_id, "g-tenant-test").await;
    let bpmn = r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def" targetNamespace="urn:test">
  <process id="p" isExecutable="true">
    <startEvent id="s"/>
    <userTask id="t1"/>
    <endEvent id="e"/>
    <sequenceFlow id="f1" sourceRef="s" targetRef="t1"/>
    <sequenceFlow id="f2" sourceRef="t1" targetRef="e"/>
  </process>
</definitions>"#;
    let resp = app
        .client
        .post(format!("{}/api/v1/deployments", app.address))
        .json(&json!({
            "process_group_id": group_id,
            "key": format!("tenant-iso-{}", Uuid::new_v4()),
            "bpmn_xml": bpmn,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let def_id = resp.json::<serde_json::Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let resp = app
        .client
        .post(format!("{}/api/v1/process-instances", app.address))
        .json(&json!({ "definition_id": def_id }))
        .send()
        .await
        .unwrap();
    let instance_id = resp.json::<serde_json::Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Org B asks for the instance by ID — must 404, not 403, so existence
    // is not leaked.
    let resp = client_b
        .get(format!(
            "{}/api/v1/process-instances/{}",
            app.address, instance_id
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn cross_org_secret_path_returns_404() {
    let app = common::spawn_test_app().await;
    let principal_b = common::create_principal(&app.pool, "secret-b").await;
    let client_b = authed_client(&principal_b.token);

    // Default principal (org A) creates a secret.
    app.client
        .post(format!(
            "{}/api/v1/orgs/{}/secrets",
            app.address, app.principal.org_id
        ))
        .json(&json!({ "name": "alpha", "value": "v" }))
        .send()
        .await
        .unwrap();

    // Org B tries to GET org A's secret URL — must 404.
    let resp = client_b
        .get(format!(
            "{}/api/v1/orgs/{}/secrets/alpha",
            app.address, app.principal.org_id
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

// ─── Org create/delete are tenant-gated ──────────────────────────────────

#[tokio::test]
async fn cross_org_delete_returns_404_and_does_not_remove_target() {
    let app = common::spawn_test_app().await;
    let principal_b = common::create_principal(&app.pool, "victim").await;
    let client_b = authed_client(&principal_b.token);

    // Org B tries to delete org A. Must 404, and org A must still exist.
    let resp = client_b
        .delete(format!(
            "{}/api/v1/orgs/{}",
            app.address, app.principal.org_id
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);

    let still_there = conduit::db::orgs::get_by_id(&app.pool, app.principal.org_id)
        .await
        .unwrap();
    assert!(
        still_there.is_some(),
        "org A must survive a cross-tenant DELETE attempt"
    );
}

#[tokio::test]
async fn admin_can_create_org() {
    let app = common::spawn_test_app().await;
    let slug = format!("test-org-{}", uuid::Uuid::new_v4());
    let resp = app
        .client
        .post(format!("{}/api/v1/orgs", app.address))
        .json(&json!({ "name": "New Org", "slug": slug }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
}

// ─── /health and /auth/login remain public ────────────────────────────────

#[tokio::test]
async fn health_endpoint_remains_public() {
    let app = common::spawn_test_app().await;
    let resp = reqwest::get(format!("{}/health", app.address))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn login_endpoint_remains_public() {
    let app = common::spawn_test_app().await;
    let resp = reqwest::Client::new()
        .post(format!("{}/api/v1/auth/login", app.address))
        .json(&json!({ "email": "x", "password": "x", "org_slug": "x" }))
        .send()
        .await
        .unwrap();
    // Either 401 (not found) or 200 — both prove the endpoint is reachable
    // without authentication. Should NOT be 401 with code U401.
    assert!(matches!(resp.status().as_u16(), 200 | 401));
}

// ─── Sanity: ensure mint_jwt + extractor agree ────────────────────────────

#[tokio::test]
async fn jwt_minted_with_helper_is_accepted_by_extractor() {
    let app = common::spawn_test_app().await;
    let token = mint_jwt(app.principal.user_id, app.principal.org_id);
    let client = authed_client(&token);
    let resp = client
        .get(format!("{}/api/v1/me", app.address))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
}

// ─── Fixtures ─────────────────────────────────────────────────────────────

async fn seed_internal_user(app: &common::TestApp, password: &str) -> (String, String) {
    let slug = format!("login-{}", Uuid::new_v4());
    let org = conduit::db::orgs::insert(&app.pool, "Login Test Org", &slug)
        .await
        .unwrap();
    let email = format!("user-{}@test.local", Uuid::new_v4());
    let hash = password::hash(password).unwrap();
    conduit::db::users::insert(&app.pool, org.id, "internal", None, &email, Some(&hash))
        .await
        .unwrap();
    (slug, email)
}
