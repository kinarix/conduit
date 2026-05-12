//! Phase 23.1 — scoped RBAC: a single user can hold different roles in
//! different orgs, and the membership check blocks access to orgs they
//! haven't joined.
//!
//! Setup matches the plan's worked example: Alice is OrgOwner in Org A and
//! Reader in Org B. Org C exists but Alice isn't a member.

mod common;

use serde_json::json;
use uuid::Uuid;

use common::auth;

/// Inline setup for an Alice-style principal with two distinct role grants
/// in two orgs. We don't generalise `create_scoped_principal` for the
/// multi-org case — the indirection would obscure exactly what's wired up.
struct Alice {
    user_id: Uuid,
    org_a: Uuid,
    org_b: Uuid,
    org_c: Uuid,
    token: String,
}

async fn setup_alice(pool: &sqlx::PgPool) -> Alice {
    let org_a = conduit::db::orgs::insert(pool, "Org A", &format!("orga-{}", Uuid::new_v4()))
        .await
        .unwrap();
    let org_b = conduit::db::orgs::insert(pool, "Org B", &format!("orgb-{}", Uuid::new_v4()))
        .await
        .unwrap();
    let org_c = conduit::db::orgs::insert(pool, "Org C", &format!("orgc-{}", Uuid::new_v4()))
        .await
        .unwrap();

    let email = format!("alice-{}@test.local", Uuid::new_v4());
    let user = conduit::db::users::insert(pool, "internal", None, &email, None, None, None)
        .await
        .unwrap();

    // Member of A and B but NOT C.
    conduit::db::org_members::insert(pool, user.id, org_a.id, None)
        .await
        .unwrap();
    conduit::db::org_members::insert(pool, user.id, org_b.id, None)
        .await
        .unwrap();

    let owner_id = conduit::db::roles::find_builtin_by_name(pool, "OrgOwner")
        .await
        .unwrap()
        .expect("OrgOwner builtin");
    let reader_id = conduit::db::roles::find_builtin_by_name(pool, "Reader")
        .await
        .unwrap()
        .expect("Reader builtin");

    conduit::db::role_assignments::grant_org(pool, user.id, owner_id, org_a.id, None, None)
        .await
        .unwrap();
    conduit::db::role_assignments::grant_org(pool, user.id, reader_id, org_b.id, None, None)
        .await
        .unwrap();

    let token = auth::mint_jwt(user.id, org_a.id);
    Alice {
        user_id: user.id,
        org_a: org_a.id,
        org_b: org_b.id,
        org_c: org_c.id,
        token,
    }
}

#[tokio::test]
async fn owner_in_org_a_can_create_secrets() {
    let app = common::spawn_test_app().await;
    let alice = setup_alice(&app.pool).await;
    let client = auth::authed_client(&alice.token);

    let resp = client
        .post(format!(
            "{}/api/v1/orgs/{}/secrets",
            app.address, alice.org_a
        ))
        .json(&json!({ "name": "stripe", "value": "sk_live_xxx" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201, "OrgOwner must be able to create");
}

#[tokio::test]
async fn reader_in_org_b_cannot_create_secrets() {
    let app = common::spawn_test_app().await;
    let alice = setup_alice(&app.pool).await;
    let client = auth::authed_client(&alice.token);

    let resp = client
        .post(format!(
            "{}/api/v1/orgs/{}/secrets",
            app.address, alice.org_b
        ))
        .json(&json!({ "name": "stripe", "value": "sk_live_xxx" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403, "Reader must not be able to create");
}

#[tokio::test]
async fn reader_in_org_b_can_list_secrets() {
    let app = common::spawn_test_app().await;
    let alice = setup_alice(&app.pool).await;
    let client = auth::authed_client(&alice.token);

    let resp = client
        .get(format!(
            "{}/api/v1/orgs/{}/secrets",
            app.address, alice.org_b
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "Reader must be able to read");
}

#[tokio::test]
async fn non_member_in_org_c_is_blocked() {
    let app = common::spawn_test_app().await;
    let alice = setup_alice(&app.pool).await;
    let client = auth::authed_client(&alice.token);

    // Org C exists but Alice has no membership and no global role.
    let resp = client
        .get(format!(
            "{}/api/v1/orgs/{}/secrets",
            app.address, alice.org_c
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        403,
        "non-members must be blocked before reaching the resource"
    );
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(
        body["message"]
            .as_str()
            .unwrap_or("")
            .to_lowercase()
            .contains("not a member"),
        "membership-check message expected, got: {body}"
    );
}

#[tokio::test]
async fn me_reports_both_org_memberships_and_roles() {
    let app = common::spawn_test_app().await;
    let alice = setup_alice(&app.pool).await;
    let client = auth::authed_client(&alice.token);

    let resp = client
        .get(format!("{}/api/v1/me", app.address))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["user_id"], alice.user_id.to_string());
    assert_eq!(body["is_global_admin"], false);

    let orgs = body["orgs"].as_array().expect("orgs array");
    assert_eq!(orgs.len(), 2, "Alice is a member of two orgs");

    let entry_a = orgs
        .iter()
        .find(|o| o["id"] == alice.org_a.to_string())
        .expect("org A entry");
    let roles_a: Vec<&str> = entry_a["roles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(roles_a.contains(&"OrgOwner"), "got: {roles_a:?}");

    let entry_b = orgs
        .iter()
        .find(|o| o["id"] == alice.org_b.to_string())
        .expect("org B entry");
    let roles_b: Vec<&str> = entry_b["roles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(roles_b.contains(&"Reader"), "got: {roles_b:?}");
}
