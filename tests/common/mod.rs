use axum::Router;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use conduit::state::AppState;

#[allow(dead_code)]
pub struct TestApp {
    pub address: String,
    pub pool: PgPool,
}

pub async fn spawn_test_app() -> TestApp {
    let database_url = std::env::var("TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .expect("TEST_DATABASE_URL or DATABASE_URL must be set for integration tests");

    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to test database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    // Fixed test key — deterministic so tests are reproducible. Real
    // deployments load this from CONDUIT_SECRETS_KEY env.
    let test_secrets_key = [0xA5u8; 32];
    let state = Arc::new(AppState::new(pool.clone(), test_secrets_key));

    let app = Router::new()
        .merge(conduit::api::health::routes())
        .merge(conduit::api::orgs::routes())
        .merge(conduit::api::users::routes())
        .merge(conduit::api::process_groups::routes())
        .merge(conduit::api::deployments::routes())
        .merge(conduit::api::instances::routes())
        .merge(conduit::api::tasks::routes())
        .merge(conduit::api::external_tasks::routes())
        .merge(conduit::api::decisions::routes())
        .merge(conduit::api::secrets::routes())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind test port");

    let port = listener.local_addr().unwrap().port();
    let address = format!("http://127.0.0.1:{}", port);

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    TestApp { address, pool }
}

#[allow(dead_code)]
pub async fn create_test_org(app: &TestApp) -> Uuid {
    let client = reqwest::Client::new();
    let slug = format!("test-org-{}", Uuid::new_v4());
    let resp = client
        .post(format!("{}/api/v1/orgs", app.address))
        .json(&serde_json::json!({ "name": "Test Org", "slug": slug }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201, "create_test_org failed");
    let body: serde_json::Value = resp.json().await.unwrap();
    Uuid::parse_str(body["id"].as_str().unwrap()).unwrap()
}

/// Create a process group via the HTTP API. Used by integration tests that
/// exercise the deploy endpoints.
#[allow(dead_code)]
pub async fn create_test_process_group(app: &TestApp, org_id: Uuid, name: &str) -> Uuid {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/api/v1/process-groups", app.address))
        .json(&serde_json::json!({ "org_id": org_id, "name": name }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        201,
        "create_test_process_group failed: {:?}",
        resp.text().await
    );
    let body: serde_json::Value = resp.json().await.unwrap();
    Uuid::parse_str(body["id"].as_str().unwrap()).unwrap()
}

/// Create an org plus `count` process groups. Returns (org_id, group_ids).
/// Tests that only need a single primary group typically destructure as
/// `(org_id, groups)` and pass `groups[0]` to deployments.
#[allow(dead_code)]
pub async fn create_test_org_with_groups(app: &TestApp, count: usize) -> (Uuid, Vec<Uuid>) {
    let org_id = create_test_org(app).await;
    let mut groups = Vec::with_capacity(count);
    for i in 0..count {
        let name = format!("Group {}", i + 1);
        groups.push(create_test_process_group(app, org_id, &name).await);
    }
    (org_id, groups)
}
