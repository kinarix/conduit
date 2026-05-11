//! Phase 23.1 — explicit org membership.
//!
//! `org_members(user_id, org_id)` is a precondition for any org-scoped role
//! grant. This test exercises both ends of that contract:
//!
//! 1. `grant_org` rejects a non-member with a `Validation` error.
//! 2. Removing a member cascades their `org_role_assignments` rows for that
//!    org (composite FK from migration 032).
//! 3. The cascade is org-local: grants in *other* orgs are untouched.

mod common;

use uuid::Uuid;

#[tokio::test]
async fn cannot_grant_org_role_to_non_member() {
    let app = common::spawn_test_app().await;
    let pool = &app.pool;

    let org = conduit::db::orgs::insert(pool, "Org NM", &format!("nm-{}", Uuid::new_v4()))
        .await
        .unwrap();
    let user = conduit::db::users::insert(
        pool,
        "internal",
        None,
        &format!("nonmember-{}@test.local", Uuid::new_v4()),
        None,
    )
    .await
    .unwrap();
    let role_id = conduit::db::roles::find_builtin_by_name(pool, "Reader")
        .await
        .unwrap()
        .expect("Reader builtin");

    // No org_members row; grant_org must refuse.
    let err = conduit::db::role_assignments::grant_org(pool, user.id, role_id, org.id, None, None)
        .await
        .expect_err("grant_org must reject non-members");
    let msg = format!("{err}");
    assert!(
        msg.contains("not a member"),
        "expected not-a-member validation error, got: {msg}"
    );
}

#[tokio::test]
async fn removing_member_cascades_org_role_assignments() {
    let app = common::spawn_test_app().await;
    let pool = &app.pool;

    let scoped = common::create_scoped_principal(pool, "cascade", "OrgOwner").await;

    // Sanity: the grant we just created is visible.
    let before =
        conduit::db::role_assignments::list_for_user_in_org(pool, scoped.user_id, scoped.org_id)
            .await
            .unwrap();
    assert_eq!(before.len(), 1, "test setup must create exactly one grant");

    // Remove membership. Migration 032 makes this cascade the org grants for
    // (user_id, org_id).
    let removed = conduit::db::org_members::delete(pool, scoped.user_id, scoped.org_id)
        .await
        .unwrap();
    assert!(removed);

    let after =
        conduit::db::role_assignments::list_for_user_in_org(pool, scoped.user_id, scoped.org_id)
            .await
            .unwrap();
    assert!(
        after.is_empty(),
        "org_role_assignments for (user, org) must cascade on membership delete; got {} leftover",
        after.len()
    );
}

#[tokio::test]
async fn membership_cascade_is_scoped_to_one_org() {
    // Removing Alice from Org A must not touch her grants in Org B. The
    // composite FK targets (user_id, org_id), not just user_id, so other
    // org rows must survive.
    let app = common::spawn_test_app().await;
    let pool = &app.pool;

    let email = format!("multi-{}@test.local", Uuid::new_v4());
    let user = conduit::db::users::insert(pool, "internal", None, &email, None)
        .await
        .unwrap();

    let org_a = conduit::db::orgs::insert(pool, "A", &format!("a-{}", Uuid::new_v4()))
        .await
        .unwrap();
    let org_b = conduit::db::orgs::insert(pool, "B", &format!("b-{}", Uuid::new_v4()))
        .await
        .unwrap();
    conduit::db::org_members::insert(pool, user.id, org_a.id, None)
        .await
        .unwrap();
    conduit::db::org_members::insert(pool, user.id, org_b.id, None)
        .await
        .unwrap();

    let role_id = conduit::db::roles::find_builtin_by_name(pool, "OrgOwner")
        .await
        .unwrap()
        .unwrap();
    conduit::db::role_assignments::grant_org(pool, user.id, role_id, org_a.id, None, None)
        .await
        .unwrap();
    conduit::db::role_assignments::grant_org(pool, user.id, role_id, org_b.id, None, None)
        .await
        .unwrap();

    // Drop A only.
    conduit::db::org_members::delete(pool, user.id, org_a.id)
        .await
        .unwrap();

    let in_a = conduit::db::role_assignments::list_for_user_in_org(pool, user.id, org_a.id)
        .await
        .unwrap();
    let in_b = conduit::db::role_assignments::list_for_user_in_org(pool, user.id, org_b.id)
        .await
        .unwrap();
    assert!(in_a.is_empty(), "A grants cascaded");
    assert_eq!(in_b.len(), 1, "B grants must survive");
}

#[tokio::test]
async fn member_add_via_api_then_grant_succeeds() {
    // End-to-end: a platform admin adds a user to an org and then grants
    // them a role inside it. Both legs use the HTTP layer to verify the
    // routes wire up.
    let app = common::spawn_test_app().await;
    let client = app.client.clone();

    // The default principal has global PlatformAdmin and is a member of its
    // own org. Create a target org and a target user.
    let target_org =
        conduit::db::orgs::insert(&app.pool, "Target", &format!("target-{}", Uuid::new_v4()))
            .await
            .unwrap();
    let target_user = conduit::db::users::insert(
        &app.pool,
        "internal",
        None,
        &format!("target-{}@test.local", Uuid::new_v4()),
        None,
    )
    .await
    .unwrap();

    // Add membership.
    let resp = client
        .post(format!(
            "{}/api/v1/orgs/{}/members",
            app.address, target_org.id
        ))
        .json(&serde_json::json!({ "user_id": target_user.id }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);

    // Then grant a role.
    let role_id = conduit::db::roles::find_builtin_by_name(&app.pool, "Reader")
        .await
        .unwrap()
        .unwrap();
    let resp = client
        .post(format!(
            "{}/api/v1/orgs/{}/role-assignments",
            app.address, target_org.id
        ))
        .json(&serde_json::json!({ "user_id": target_user.id, "role_id": role_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);

    // Confirm in DB.
    let grants = conduit::db::role_assignments::list_for_user_in_org(
        &app.pool,
        target_user.id,
        target_org.id,
    )
    .await
    .unwrap();
    assert_eq!(grants.len(), 1);
}
