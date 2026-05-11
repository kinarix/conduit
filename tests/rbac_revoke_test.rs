//! Phase 23.1 — revoking a role takes effect on the next request. Permissions
//! are loaded fresh per request by the Principal extractor, so there's no
//! cache to invalidate, just a DB row to remove.
//!
//! Two layers to cover:
//! 1. Revoke the role grant only → user remains a member with zero perms,
//!    next request is 403 on `permission required`.
//! 2. Revoke the role grant AND remove membership → next request is 403
//!    on `not a member`.

mod common;

use serde_json::json;

use common::auth;

#[tokio::test]
async fn revoking_role_immediately_blocks_writes() {
    let app = common::spawn_test_app().await;
    let alice = common::create_scoped_principal(&app.pool, "revoke", "OrgOwner").await;
    let client = auth::authed_client(&alice.token);

    // Baseline: she can write.
    let resp = client
        .post(format!(
            "{}/api/v1/orgs/{}/secrets",
            app.address, alice.org_id
        ))
        .json(&json!({ "name": "before", "value": "v" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);

    // Revoke every org grant Alice holds inside this org.
    let grants =
        conduit::db::role_assignments::list_for_user_in_org(&app.pool, alice.user_id, alice.org_id)
            .await
            .unwrap();
    assert!(
        !grants.is_empty(),
        "precondition: Alice has at least one grant"
    );
    for g in grants {
        let revoked =
            conduit::db::role_assignments::revoke_org_by_id(&app.pool, g.id, alice.org_id)
                .await
                .unwrap();
        assert!(revoked);
    }

    // Next write fails. She is still a member, so the extractor allows the
    // request through but the permission check rejects it.
    let resp = client
        .post(format!(
            "{}/api/v1/orgs/{}/secrets",
            app.address, alice.org_id
        ))
        .json(&json!({ "name": "after", "value": "v" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
    let body: serde_json::Value = resp.json().await.unwrap();
    let msg = body["message"].as_str().unwrap_or("");
    assert!(
        msg.to_lowercase().contains("permission required"),
        "expected permission-required message, got: {msg}"
    );
}

#[tokio::test]
async fn revoking_membership_blocks_with_not_a_member() {
    let app = common::spawn_test_app().await;
    let alice = common::create_scoped_principal(&app.pool, "revoke-mem", "OrgOwner").await;
    let client = auth::authed_client(&alice.token);

    // Removing the membership cascades the org role-assignment via migration
    // 032's composite FK (org_role_assignments → org_members on
    // (user_id, org_id) ON DELETE CASCADE).
    let removed = conduit::db::org_members::delete(&app.pool, alice.user_id, alice.org_id)
        .await
        .unwrap();
    assert!(removed);

    // Sanity: the cascade actually fired.
    let remaining =
        conduit::db::role_assignments::list_for_user_in_org(&app.pool, alice.user_id, alice.org_id)
            .await
            .unwrap();
    assert!(
        remaining.is_empty(),
        "membership delete must cascade org_role_assignments, got {} leftover",
        remaining.len()
    );

    let resp = client
        .get(format!(
            "{}/api/v1/orgs/{}/secrets",
            app.address, alice.org_id
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
    let body: serde_json::Value = resp.json().await.unwrap();
    let msg = body["message"].as_str().unwrap_or("");
    assert!(
        msg.to_lowercase().contains("not a member"),
        "expected not-a-member message, got: {msg}"
    );
}

#[tokio::test]
async fn unrelated_orgs_unaffected_by_revoke() {
    // Sanity check: revoking Alice's grant in Org X must not affect a
    // different user Bob's grant in Org Y. Same DB row scope.
    let app = common::spawn_test_app().await;
    let alice = common::create_scoped_principal(&app.pool, "ru-a", "OrgOwner").await;
    let bob = common::create_scoped_principal(&app.pool, "ru-b", "OrgOwner").await;
    let bob_client = auth::authed_client(&bob.token);

    let alice_grants =
        conduit::db::role_assignments::list_for_user_in_org(&app.pool, alice.user_id, alice.org_id)
            .await
            .unwrap();
    for g in alice_grants {
        conduit::db::role_assignments::revoke_org_by_id(&app.pool, g.id, alice.org_id)
            .await
            .unwrap();
    }

    let resp = bob_client
        .post(format!(
            "{}/api/v1/orgs/{}/secrets",
            app.address, bob.org_id
        ))
        .json(&json!({ "name": "bob", "value": "v" }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        201,
        "Bob's perms in his own org are untouched"
    );
}
