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

    let state = Arc::new(AppState::new(pool.clone()));

    let app = Router::new()
        .merge(conduit::api::health::routes())
        .merge(conduit::api::orgs::routes())
        .merge(conduit::api::users::routes())
        .merge(conduit::api::deployments::routes())
        .merge(conduit::api::instances::routes())
        .merge(conduit::api::tasks::routes())
        .merge(conduit::api::external_tasks::routes())
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
