//! Phase 23.1 — bootstrap flow: the first user created on a fresh DB
//! receives global `PlatformAdmin` and can log in + create orgs without any
//! pre-existing org membership.
//!
//! The test database is shared across the test suite, so we can't simulate
//! "no users in DB" by truncating (other tests would break). Instead we
//! replicate the bootstrap function's actions on a *new* email + password
//! and verify the resulting principal has the same end-state the bootstrap
//! path produces. This exercises every observable contract:
//!   - the global PlatformAdmin grant works,
//!   - login by email + password (no org slug) succeeds,
//!   - the resulting JWT is treated as a global admin by /me,
//!   - the admin can create an org *without* being a member of one first.

mod common;

use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn bootstrap_admin_is_global_platform_admin_and_can_create_org() {
    let app = common::spawn_test_app().await;
    let client = reqwest::Client::new(); // unauthenticated for login.

    // Mirror the bootstrap function's actions verbatim.
    let email = format!("boot-{}@test.local", Uuid::new_v4());
    let password = "bootstrap-secret-pw";
    let hash = conduit::auth::password::hash(password).expect("hash");
    let user =
        conduit::db::users::insert(&app.pool, "internal", None, &email, Some(&hash), None, None)
            .await
            .expect("insert");
    let granted = conduit::db::role_assignments::grant_global_by_name(
        &app.pool,
        user.id,
        "PlatformAdmin",
        Some(user.id),
    )
    .await
    .expect("grant");
    assert!(granted, "built-in PlatformAdmin role must be seeded");

    // No org membership yet.
    let orgs_for_user = conduit::db::orgs::list_for_user(&app.pool, user.id)
        .await
        .unwrap();
    assert!(
        orgs_for_user.is_empty(),
        "bootstrap admin is not a member of any org by default"
    );

    // Login flow — email + password, no slug.
    let login_resp = client
        .post(format!("{}/api/v1/auth/login", app.address))
        .json(&json!({ "email": email, "password": password }))
        .send()
        .await
        .unwrap();
    assert_eq!(login_resp.status(), 200);
    let login_body: serde_json::Value = login_resp.json().await.unwrap();
    let token = login_body["access_token"]
        .as_str()
        .expect("access_token")
        .to_string();
    let authed = common::auth::authed_client(&token);

    // /me reports global admin.
    let me_resp = authed
        .get(format!("{}/api/v1/me", app.address))
        .send()
        .await
        .unwrap();
    assert_eq!(me_resp.status(), 200);
    let me: serde_json::Value = me_resp.json().await.unwrap();
    assert_eq!(me["is_global_admin"], true);
    assert!(
        me["orgs"].as_array().unwrap().is_empty(),
        "bootstrap admin starts with zero org memberships"
    );

    // Can create an org despite being a non-member.
    let slug = format!("boot-org-{}", Uuid::new_v4());
    let create_resp = authed
        .post(format!("{}/api/v1/orgs", app.address))
        .json(&json!({ "name": "Bootstrap Org", "slug": slug }))
        .send()
        .await
        .unwrap();
    assert_eq!(create_resp.status(), 201);
    let new_org: serde_json::Value = create_resp.json().await.unwrap();
    let new_org_id = Uuid::parse_str(new_org["id"].as_str().unwrap()).unwrap();

    // Creating the org enrolled the creator as OrgOwner (see api::orgs::create).
    let after: serde_json::Value = authed
        .get(format!("{}/api/v1/me", app.address))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let orgs = after["orgs"].as_array().unwrap();
    assert_eq!(orgs.len(), 1, "creator becomes a member of the new org");
    assert_eq!(orgs[0]["id"], new_org_id.to_string());
    let roles: Vec<&str> = orgs[0]["roles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(
        roles.contains(&"OrgOwner"),
        "creator must be granted OrgOwner; got {roles:?}"
    );
}

#[tokio::test]
async fn run_if_needed_is_a_noop_when_users_exist() {
    // The shared test DB always has users (spawn_test_app seeds one). Call
    // run_if_needed and assert it's a no-op: no new user inserted, no new
    // global grant created.
    let app = common::spawn_test_app().await;

    let users_before = conduit::db::users::count(&app.pool).await.unwrap();
    let globals_before = conduit::db::role_assignments::list_global(&app.pool)
        .await
        .unwrap()
        .len();

    // Build a Config that *would* try to bootstrap if the gate were open.
    let cfg = conduit::config::Config {
        database_url: String::new(),
        auth_provider: conduit::config::AuthProvider::Internal,
        server_host: String::new(),
        server_port: 0,
        log_level: String::new(),
        db_min_connections: 0,
        db_max_connections: 0,
        db_acquire_timeout_secs: 0,
        db_statement_timeout_ms: 0,
        job_executor_poll_ms: 0,
        job_executor_batch_size: 0,
        job_lock_duration_secs: 0,
        secrets_key: [0; 32],
        jwt_signing_key: String::new(),
        jwt_ttl_seconds: 0,
        jwt_issuer: String::new(),
        tenant_isolation: conduit::config::TenantIsolation::Multi,
        bootstrap_admin_email: Some(format!("noop-{}@test.local", Uuid::new_v4())),
        bootstrap_admin_password: Some("anything".into()),
        bootstrap_admin_org_slug: None,
    };
    conduit::auth::bootstrap::run_if_needed(&app.pool, &cfg)
        .await
        .expect("bootstrap must succeed as a no-op");

    let users_after = conduit::db::users::count(&app.pool).await.unwrap();
    let globals_after = conduit::db::role_assignments::list_global(&app.pool)
        .await
        .unwrap()
        .len();
    assert_eq!(users_before, users_after, "no new user must be inserted");
    assert_eq!(
        globals_before, globals_after,
        "no new global role grant must be created"
    );
}
