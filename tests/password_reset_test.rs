//! Phase 23.2a — password management. Covers:
//!   - self-service change (`POST /api/v1/auth/change-password`)
//!   - org-scoped admin reset (`POST /api/v1/orgs/{org_id}/users/{user_id}/reset-password`)
//!   - global admin reset (`POST /api/v1/admin/users/{user_id}/reset-password`)
//!
//! Hierarchy: a non-global caller cannot reset the password of a platform
//! admin who happens to be a member of the same org.

mod common;

use serde_json::json;
use uuid::Uuid;

use common::auth;

/// Helper: insert an internal-auth user directly + add them as a member of
/// the given org. Returns (user_id, plaintext_password). The hash uses
/// argon2id via the production helper so /auth/login works against the row.
async fn seed_member(
    pool: &sqlx::PgPool,
    org_id: Uuid,
    prefix: &str,
    initial_password: &str,
) -> Uuid {
    let email = format!("{prefix}-{}@test.local", Uuid::new_v4());
    let hash = conduit::auth::password::hash(initial_password).expect("hash");
    let user = conduit::db::users::insert(pool, "internal", None, &email, Some(&hash), None, None)
        .await
        .expect("insert user");
    conduit::db::org_members::insert(pool, user.id, org_id, None)
        .await
        .expect("add member");
    user.id
}

/// Log in via /auth/login. Returns the response status + body so the
/// caller can assert on either.
async fn login_status(app: &common::TestApp, email: &str, password: &str) -> reqwest::StatusCode {
    let resp = reqwest::Client::new()
        .post(format!("{}/api/v1/auth/login", app.address))
        .json(&json!({ "email": email, "password": password }))
        .send()
        .await
        .unwrap();
    resp.status()
}

async fn email_of(pool: &sqlx::PgPool, user_id: Uuid) -> String {
    let user = conduit::db::users::find_by_id(pool, user_id)
        .await
        .unwrap()
        .unwrap();
    user.email
}

#[tokio::test]
async fn self_service_change_password_succeeds_and_old_rejected() {
    let app = common::spawn_test_app().await;
    let admin = common::create_scoped_principal(&app.pool, "self-pw", "OrgAdmin").await;
    // Bake a known initial password on the user row so we can verify the
    // before/after state via /auth/login.
    let hash = conduit::auth::password::hash("initial-pw").unwrap();
    conduit::db::users::set_password_hash(&app.pool, admin.user_id, &hash)
        .await
        .unwrap();
    let email = email_of(&app.pool, admin.user_id).await;

    let client = auth::authed_client(&admin.token);
    let resp = client
        .post(format!("{}/api/v1/auth/change-password", app.address))
        .json(&json!({
            "current_password": "initial-pw",
            "new_password": "brand-new-pw"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    assert_eq!(login_status(&app, &email, "brand-new-pw").await, 200);
    assert_eq!(login_status(&app, &email, "initial-pw").await, 401);
}

#[tokio::test]
async fn self_service_wrong_current_password_returns_u011() {
    let app = common::spawn_test_app().await;
    let admin = common::create_scoped_principal(&app.pool, "self-bad", "OrgAdmin").await;
    let hash = conduit::auth::password::hash("real-pw").unwrap();
    conduit::db::users::set_password_hash(&app.pool, admin.user_id, &hash)
        .await
        .unwrap();
    let email = email_of(&app.pool, admin.user_id).await;

    let client = auth::authed_client(&admin.token);
    let resp = client
        .post(format!("{}/api/v1/auth/change-password", app.address))
        .json(&json!({
            "current_password": "wrong-pw",
            "new_password": "doesn-t-matter"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["code"], "U011");

    // Password did not change.
    assert_eq!(login_status(&app, &email, "real-pw").await, 200);
}

#[tokio::test]
async fn self_service_rejected_for_external_auth_user() {
    let app = common::spawn_test_app().await;
    // External-auth user — no password_hash. Issue a JWT directly so we
    // exercise the change-password endpoint with a non-internal principal.
    let email = format!("ext-{}@test.local", Uuid::new_v4());
    let user = conduit::db::users::insert(
        &app.pool,
        "external",
        Some("ext-id"),
        &email,
        None,
        None,
        None,
    )
    .await
    .unwrap();
    let org = conduit::db::orgs::insert(&app.pool, "Ext Org", &format!("ext-{}", Uuid::new_v4()))
        .await
        .unwrap();
    conduit::db::org_members::insert(&app.pool, user.id, org.id, None)
        .await
        .unwrap();
    let token = auth::mint_jwt(user.id, org.id);
    let client = auth::authed_client(&token);

    let resp = client
        .post(format!("{}/api/v1/auth/change-password", app.address))
        .json(&json!({
            "current_password": "anything",
            "new_password": "doesnt-matter"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["code"], "U001");
}

#[tokio::test]
async fn org_admin_resets_member_password() {
    let app = common::spawn_test_app().await;
    let admin = common::create_scoped_principal(&app.pool, "oa-reset", "OrgAdmin").await;
    let target = seed_member(&app.pool, admin.org_id, "oa-target", "old-pw").await;
    let target_email = email_of(&app.pool, target).await;

    let client = auth::authed_client(&admin.token);
    let resp = client
        .post(format!(
            "{}/api/v1/orgs/{}/users/{}/reset-password",
            app.address, admin.org_id, target
        ))
        .json(&json!({ "new_password": "freshly-reset" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    assert_eq!(
        login_status(&app, &target_email, "freshly-reset").await,
        200
    );
    assert_eq!(login_status(&app, &target_email, "old-pw").await, 401);
}

#[tokio::test]
async fn org_admin_denied_for_non_member_target() {
    let app = common::spawn_test_app().await;
    let admin = common::create_scoped_principal(&app.pool, "oa-nonmember", "OrgAdmin").await;

    // Target lives in a different org.
    let other =
        conduit::db::orgs::insert(&app.pool, "Other Org", &format!("other-{}", Uuid::new_v4()))
            .await
            .unwrap();
    let target = seed_member(&app.pool, other.id, "oa-foreigner", "x").await;

    let client = auth::authed_client(&admin.token);
    let resp = client
        .post(format!(
            "{}/api/v1/orgs/{}/users/{}/reset-password",
            app.address, admin.org_id, target
        ))
        .json(&json!({ "new_password": "wont-matter" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["code"], "U002");
}

#[tokio::test]
async fn org_admin_denied_for_platform_admin_target() {
    let app = common::spawn_test_app().await;
    let admin = common::create_scoped_principal(&app.pool, "oa-vs-pa", "OrgAdmin").await;

    // Create a platform admin who is ALSO a member of admin.org_id.
    let pa_email = format!("pa-{}@test.local", Uuid::new_v4());
    let pa_hash = conduit::auth::password::hash("pa-pw").unwrap();
    let pa = conduit::db::users::insert(
        &app.pool,
        "internal",
        None,
        &pa_email,
        Some(&pa_hash),
        None,
        None,
    )
    .await
    .unwrap();
    conduit::db::org_members::insert(&app.pool, pa.id, admin.org_id, None)
        .await
        .unwrap();
    conduit::db::role_assignments::grant_global_by_name(&app.pool, pa.id, "PlatformAdmin", None)
        .await
        .unwrap();

    let client = auth::authed_client(&admin.token);
    let resp = client
        .post(format!(
            "{}/api/v1/orgs/{}/users/{}/reset-password",
            app.address, admin.org_id, pa.id
        ))
        .json(&json!({ "new_password": "wont-matter" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["code"], "U403");
    let msg = body["message"].as_str().unwrap_or("");
    assert!(
        msg.to_lowercase().contains("platform admin"),
        "expected hierarchy error message, got: {msg}"
    );

    // PA's old password still works.
    assert_eq!(login_status(&app, &pa_email, "pa-pw").await, 200);
}

#[tokio::test]
async fn platform_admin_resets_via_global_endpoint() {
    let app = common::spawn_test_app().await;
    // `app.principal` is a global PlatformAdmin (see common::create_principal).
    let target_org =
        conduit::db::orgs::insert(&app.pool, "Target Org", &format!("gt-{}", Uuid::new_v4()))
            .await
            .unwrap();
    let target = seed_member(&app.pool, target_org.id, "gt-target", "old-pw").await;
    let target_email = email_of(&app.pool, target).await;

    let resp = app
        .client
        .post(format!(
            "{}/api/v1/admin/users/{}/reset-password",
            app.address, target
        ))
        .json(&json!({ "new_password": "global-reset" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);
    assert_eq!(login_status(&app, &target_email, "global-reset").await, 200);
}

#[tokio::test]
async fn org_admin_cannot_call_global_endpoint() {
    let app = common::spawn_test_app().await;
    let admin = common::create_scoped_principal(&app.pool, "oa-vs-global", "OrgAdmin").await;
    let target = seed_member(&app.pool, admin.org_id, "oa-target", "old-pw").await;

    let client = auth::authed_client(&admin.token);
    let resp = client
        .post(format!(
            "{}/api/v1/admin/users/{}/reset-password",
            app.address, target
        ))
        .json(&json!({ "new_password": "nope" }))
        .send()
        .await
        .unwrap();
    // OrgAdmin holds user.reset_password at ORG scope, not global. The flat
    // endpoint requires the global grant.
    assert_eq!(resp.status(), 403);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["code"], "U403");
}

#[tokio::test]
async fn org_admin_rejects_short_new_password() {
    let app = common::spawn_test_app().await;
    let admin = common::create_scoped_principal(&app.pool, "oa-short", "OrgAdmin").await;
    let target = seed_member(&app.pool, admin.org_id, "oa-target", "old-pw").await;
    let target_email = email_of(&app.pool, target).await;

    let client = auth::authed_client(&admin.token);
    let resp = client
        .post(format!(
            "{}/api/v1/orgs/{}/users/{}/reset-password",
            app.address, admin.org_id, target
        ))
        .json(&json!({ "new_password": "short" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["code"], "U001");

    // Old password still works.
    assert_eq!(login_status(&app, &target_email, "old-pw").await, 200);
}
