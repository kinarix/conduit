//! Phase 23.2 — Process-group-scoped role assignments.
//!
//! Verifies the three-tier scope model:
//!   - global grants cascade to every pg
//!   - org grants cascade to every pg in the org
//!   - pg grants apply only to that pg
//! and the partition rules (org-only perms cannot be granted at pg scope,
//! cross-org tampering is blocked at the trigger level, member-delete
//! cascades pg grants).

mod common;

use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use common::auth;

/// Custom role containing ONLY pg-scopable perms — required for pg grants
/// because builtin roles include `org.read` (org-only) and the API rejects
/// pg-scope grants of any role with an org-only perm.
async fn create_pg_developer_role(pool: &PgPool, org_id: Uuid) -> Uuid {
    let name = format!("pg-dev-{}", Uuid::new_v4());
    let perms: Vec<String> = vec![
        "process.read".into(),
        "process.create".into(),
        "process.update".into(),
        "process.delete".into(),
        "process.deploy".into(),
        "process_group.read".into(),
        "process_group.update".into(),
        "instance.read".into(),
        "instance.start".into(),
    ];
    let role = conduit::db::roles::create_custom_role(pool, org_id, &name, &perms)
        .await
        .expect("create custom role");
    role.id
}

struct Scene {
    org_id: Uuid,
    pg_x: Uuid,
    pg_y: Uuid,
    alice_token: String,
    alice_user: Uuid,
    role_id: Uuid,
}

/// Build an org with two pgs and a user (`alice`) who is a member but has
/// no roles granted. Returns the role_id for the custom pg-scopable role.
async fn setup_scene(pool: &PgPool) -> Scene {
    let org = conduit::db::orgs::insert(pool, "Acme", &format!("acme-{}", Uuid::new_v4()))
        .await
        .unwrap();
    let pg_x = conduit::db::process_groups::insert(pool, org.id, "HR")
        .await
        .unwrap();
    let pg_y = conduit::db::process_groups::insert(pool, org.id, "Finance")
        .await
        .unwrap();

    let email = format!("alice-{}@test.local", Uuid::new_v4());
    let user = conduit::db::users::insert(pool, "internal", None, &email, None, None, None)
        .await
        .unwrap();
    conduit::db::org_members::insert(pool, user.id, org.id, None)
        .await
        .unwrap();

    let role_id = create_pg_developer_role(pool, org.id).await;

    let token = auth::mint_jwt(user.id, org.id);

    Scene {
        org_id: org.id,
        pg_x: pg_x.id,
        pg_y: pg_y.id,
        alice_token: token,
        alice_user: user.id,
        role_id,
    }
}

// ───────────────────────────────────────────────────────────────────────────
// 1. PG-only grant gates correctly.
// ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn pg_only_grant_allows_target_pg_and_blocks_others() {
    let app = common::spawn_test_app().await;
    let s = setup_scene(&app.pool).await;

    conduit::db::role_assignments::grant_process_group(
        &app.pool,
        s.alice_user,
        s.role_id,
        s.pg_x,
        None,
    )
    .await
    .expect("grant pg scope");

    let client = auth::authed_client(&s.alice_token);

    // Rename pg_x → 200 (Alice has process_group.update via pg grant)
    let resp = client
        .put(format!(
            "{}/api/v1/orgs/{}/process-groups/{}",
            app.address, s.org_id, s.pg_x
        ))
        .json(&json!({ "name": "HR (renamed)" }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "pg-scoped grant should permit update in granted pg"
    );

    // Rename pg_y → 403
    let resp = client
        .put(format!(
            "{}/api/v1/orgs/{}/process-groups/{}",
            app.address, s.org_id, s.pg_y
        ))
        .json(&json!({ "name": "Finance (renamed)" }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        403,
        "pg-scoped grant must NOT permit update in another pg"
    );

    // /me should expose the pg grant under orgs[].pg_roles so the UI can
    // render "<role> in <pg>" without an extra round-trip.
    let me = client
        .get(format!("{}/api/v1/me", app.address))
        .send()
        .await
        .unwrap();
    assert_eq!(me.status(), 200);
    let body: serde_json::Value = me.json().await.unwrap();
    let orgs = body["orgs"].as_array().expect("orgs array");
    let entry = orgs
        .iter()
        .find(|o| o["id"].as_str() == Some(&s.org_id.to_string()))
        .expect("org entry for s.org_id");
    let pg_roles = entry["pg_roles"].as_array().expect("orgs[].pg_roles array");
    assert_eq!(pg_roles.len(), 1, "exactly one pg grant should appear");
    assert_eq!(
        pg_roles[0]["process_group_id"].as_str(),
        Some(s.pg_x.to_string().as_str())
    );
    assert!(
        pg_roles[0]["role_name"]
            .as_str()
            .unwrap_or("")
            .starts_with("pg-dev-"),
        "expected pg-dev custom role, got {}",
        pg_roles[0]["role_name"]
    );

    // List endpoints must filter rows to the caller's effective pg
    // scope. With process_group.read held only in pg_x, listing process
    // groups should return exactly pg_x — not pg_y.
    let resp = client
        .get(format!(
            "{}/api/v1/orgs/{}/process-groups",
            app.address, s.org_id
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    let items = body["items"]
        .as_array()
        .or_else(|| body.as_array())
        .expect("list response items");
    let visible: Vec<String> = items
        .iter()
        .filter_map(|r| r["id"].as_str().map(|s| s.to_string()))
        .collect();
    assert!(
        visible.contains(&s.pg_x.to_string()),
        "pg_x should be visible: {visible:?}"
    );
    assert!(
        !visible.contains(&s.pg_y.to_string()),
        "pg_y must NOT be visible: {visible:?}"
    );
}

// ───────────────────────────────────────────────────────────────────────────
// 1b. Member with no role grants gets 200 [] from list endpoints (not 403).
// ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn member_without_grants_list_returns_empty_not_forbidden() {
    let app = common::spawn_test_app().await;
    let s = setup_scene(&app.pool).await;
    // Alice is a member of Acme but has zero role assignments.
    let client = auth::authed_client(&s.alice_token);

    let resp = client
        .get(format!(
            "{}/api/v1/orgs/{}/process-groups",
            app.address, s.org_id
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "list endpoint with empty effective scope must return 200 [], not 403"
    );
    let body: serde_json::Value = resp.json().await.unwrap();
    let items = body["items"]
        .as_array()
        .or_else(|| body.as_array())
        .expect("list response items");
    assert!(items.is_empty(), "items should be empty: {items:?}");
}

// ───────────────────────────────────────────────────────────────────────────
// 2. Org-level grant cascades into every pg.
// ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn org_level_grant_cascades_into_every_pg() {
    let app = common::spawn_test_app().await;
    let s = setup_scene(&app.pool).await;

    // Grant the same custom role at org scope — should cascade into every pg.
    conduit::db::role_assignments::grant_org(
        &app.pool,
        s.alice_user,
        s.role_id,
        s.org_id,
        None,
        None,
    )
    .await
    .expect("grant org scope");

    let client = auth::authed_client(&s.alice_token);

    for pg in [s.pg_x, s.pg_y] {
        let resp = client
            .put(format!(
                "{}/api/v1/orgs/{}/process-groups/{}",
                app.address, s.org_id, pg
            ))
            .json(&json!({ "name": format!("renamed-{}", pg) }))
            .send()
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            200,
            "org-scoped grant should cascade into pg {pg}"
        );
    }
}

// ───────────────────────────────────────────────────────────────────────────
// 3. Mixed: read at org scope, update at single-pg scope.
// ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn mixed_org_read_plus_pg_update() {
    let app = common::spawn_test_app().await;
    let s = setup_scene(&app.pool).await;

    // Custom read-only role at org scope (pg-scopable subset of Reader).
    let read_role = conduit::db::roles::create_custom_role(
        &app.pool,
        s.org_id,
        &format!("pg-reader-{}", Uuid::new_v4()),
        &[
            "process.read".into(),
            "process_group.read".into(),
            "instance.read".into(),
        ],
    )
    .await
    .unwrap();
    conduit::db::role_assignments::grant_org(
        &app.pool,
        s.alice_user,
        read_role.id,
        s.org_id,
        None,
        None,
    )
    .await
    .unwrap();

    // Pg-only update in pg_x.
    conduit::db::role_assignments::grant_process_group(
        &app.pool,
        s.alice_user,
        s.role_id,
        s.pg_x,
        None,
    )
    .await
    .unwrap();

    let client = auth::authed_client(&s.alice_token);

    // List process groups — should return both (read cascades from org).
    let resp = client
        .get(format!(
            "{}/api/v1/orgs/{}/process-groups",
            app.address, s.org_id
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(
        body.as_array().unwrap().len(),
        2,
        "org-level read sees every pg"
    );

    // Update in pg_x → 200; in pg_y → 403.
    let r_x = client
        .put(format!(
            "{}/api/v1/orgs/{}/process-groups/{}",
            app.address, s.org_id, s.pg_x
        ))
        .json(&json!({ "name": "HR'" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r_x.status(), 200);

    let r_y = client
        .put(format!(
            "{}/api/v1/orgs/{}/process-groups/{}",
            app.address, s.org_id, s.pg_y
        ))
        .json(&json!({ "name": "Fin'" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r_y.status(), 403);
}

// ───────────────────────────────────────────────────────────────────────────
// 4. Granting a role with an org-only perm at pg scope is rejected.
// ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn pg_grant_rejected_for_role_with_org_only_perm() {
    let app = common::spawn_test_app().await;
    let s = setup_scene(&app.pool).await;

    // Role contains secret.read_plaintext which is org-only.
    let bad_role = conduit::db::roles::create_custom_role(
        &app.pool,
        s.org_id,
        &format!("bad-pg-{}", Uuid::new_v4()),
        &[
            "process.read".into(),
            "secret.read_plaintext".into(), // org-only — must reject pg grant
        ],
    )
    .await
    .unwrap();

    let err = conduit::db::role_assignments::grant_process_group(
        &app.pool,
        s.alice_user,
        bad_role.id,
        s.pg_x,
        None,
    )
    .await
    .expect_err("pg grant of an org-only perm must fail");

    let msg = format!("{err:?}");
    assert!(
        msg.contains("cannot be granted at process-group scope"),
        "expected pg-scope rejection message, got: {msg}"
    );
}

// ───────────────────────────────────────────────────────────────────────────
// 5. Removing a user from the org cascades their pg-level grants too.
// ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn member_delete_cascades_pg_grants() {
    let app = common::spawn_test_app().await;
    let s = setup_scene(&app.pool).await;

    conduit::db::role_assignments::grant_process_group(
        &app.pool,
        s.alice_user,
        s.role_id,
        s.pg_x,
        None,
    )
    .await
    .unwrap();

    let before =
        conduit::db::role_assignments::list_pg_for_user_in_org(&app.pool, s.alice_user, s.org_id)
            .await
            .unwrap();
    assert_eq!(before.len(), 1);

    // Remove membership — cascading FK to org_members should wipe the grant.
    conduit::db::org_members::delete(&app.pool, s.alice_user, s.org_id)
        .await
        .unwrap();

    let after =
        conduit::db::role_assignments::list_pg_for_user_in_org(&app.pool, s.alice_user, s.org_id)
            .await
            .unwrap();
    assert_eq!(
        after.len(),
        0,
        "removing membership must cascade pg-level grants"
    );
}

// ───────────────────────────────────────────────────────────────────────────
// 6. Cross-org membership check: pg in Org B, user only in Org A.
// ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn pg_grant_rejects_non_member_of_pg_org() {
    let app = common::spawn_test_app().await;
    let s = setup_scene(&app.pool).await;

    // Second org with its own pg. Alice is NOT a member of it.
    let org_b = conduit::db::orgs::insert(&app.pool, "Other", &format!("other-{}", Uuid::new_v4()))
        .await
        .unwrap();
    let pg_b = conduit::db::process_groups::insert(&app.pool, org_b.id, "Stuff")
        .await
        .unwrap();
    let role_b = create_pg_developer_role(&app.pool, org_b.id).await;

    let err = conduit::db::role_assignments::grant_process_group(
        &app.pool,
        s.alice_user,
        role_b,
        pg_b.id,
        None,
    )
    .await
    .expect_err("grant should fail: user is not a member of the pg's org");

    let msg = format!("{err:?}");
    assert!(
        msg.contains("not a member"),
        "expected membership error, got: {msg}"
    );
}

// ───────────────────────────────────────────────────────────────────────────
// 7. Trigger catches cross-org tampering at the DB level.
// ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn trigger_blocks_pg_assignment_with_mismatched_org() {
    let app = common::spawn_test_app().await;
    let s = setup_scene(&app.pool).await;

    // Make a second org + pg; insert role assignment claiming pg in org_b
    // but org_id = s.org_id. The pg-org-consistency trigger must raise.
    let org_b = conduit::db::orgs::insert(&app.pool, "Other", &format!("other-{}", Uuid::new_v4()))
        .await
        .unwrap();
    let pg_b = conduit::db::process_groups::insert(&app.pool, org_b.id, "Stuff")
        .await
        .unwrap();
    // Alice must be a member of s.org_id (composite FK) — she already is.

    let result = sqlx::query(
        "INSERT INTO process_group_role_assignments (user_id, role_id, process_group_id, org_id) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(s.alice_user)
    .bind(s.role_id)
    .bind(pg_b.id) // pg in org_b
    .bind(s.org_id) // … but claims s.org_id — mismatch
    .execute(&app.pool)
    .await;

    assert!(result.is_err(), "trigger must reject cross-org INSERT");
    let msg = format!("{:?}", result.unwrap_err());
    assert!(
        msg.contains("does not belong to org_id"),
        "expected trigger message, got: {msg}"
    );
}
