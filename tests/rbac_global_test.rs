//! Phase 23.1 — global PlatformAdmin: bypasses the org membership check and
//! holds every catalog permission across every org.
//!
//! The plan calls this out explicitly (section 4: "Global admins are allowed
//! to access any org's scoped routes even without an explicit `org_members`
//! row"). This test pins the behaviour so a future refactor that re-adds the
//! membership check fails loudly.

mod common;

use serde_json::json;
use uuid::Uuid;

use common::auth;

struct Carol {
    user_id: Uuid,
    token: String,
}

async fn setup_carol(pool: &sqlx::PgPool) -> Carol {
    let email = format!("carol-{}@test.local", Uuid::new_v4());
    let user = conduit::db::users::insert(pool, "internal", None, &email, None, None, None)
        .await
        .unwrap();
    let granted =
        conduit::db::role_assignments::grant_global_by_name(pool, user.id, "PlatformAdmin", None)
            .await
            .unwrap();
    assert!(granted, "PlatformAdmin builtin must exist");
    let token = auth::mint_jwt(user.id, Uuid::nil());
    Carol {
        user_id: user.id,
        token,
    }
}

#[tokio::test]
async fn global_admin_can_create_org() {
    let app = common::spawn_test_app().await;
    let carol = setup_carol(&app.pool).await;
    let client = auth::authed_client(&carol.token);

    let slug = format!("global-create-{}", Uuid::new_v4());
    let resp = client
        .post(format!("{}/api/v1/orgs", app.address))
        .json(&json!({ "name": "Org D", "slug": slug }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["slug"], slug);
}

#[tokio::test]
async fn global_admin_bypasses_membership_check() {
    let app = common::spawn_test_app().await;
    let carol = setup_carol(&app.pool).await;
    let client = auth::authed_client(&carol.token);

    // Create an org out-of-band; do NOT add Carol as a member.
    let target_org = conduit::db::orgs::insert(
        &app.pool,
        "Foreign Org",
        &format!("foreign-{}", Uuid::new_v4()),
    )
    .await
    .unwrap();
    assert!(
        !conduit::db::org_members::exists(&app.pool, carol.user_id, target_org.id)
            .await
            .unwrap(),
        "test precondition: Carol is not a member"
    );

    // She should still reach org-scoped routes inside it.
    let resp = client
        .get(format!(
            "{}/api/v1/orgs/{}/secrets",
            app.address, target_org.id
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "global admin must bypass the membership check"
    );

    // And write in it.
    let resp = client
        .post(format!(
            "{}/api/v1/orgs/{}/secrets",
            app.address, target_org.id
        ))
        .json(&json!({ "name": "k", "value": "v" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201, "global admin holds every permission");
}

#[tokio::test]
async fn me_marks_global_admin() {
    let app = common::spawn_test_app().await;
    let carol = setup_carol(&app.pool).await;
    let client = auth::authed_client(&carol.token);

    let resp = client
        .get(format!("{}/api/v1/me", app.address))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["is_global_admin"], true);
    let roles: Vec<&str> = body["global_roles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(roles.contains(&"PlatformAdmin"), "got: {roles:?}");
}
