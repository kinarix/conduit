use axum::Router;
use sqlx::PgPool;
use std::sync::Arc;

use conduit::state::AppState;

/// A running test application with its bound address.
pub struct TestApp {
    pub address: String,
    pub pool: PgPool,
}

/// Spawn a test application backed by the shared test database.
///
/// Uses TEST_DATABASE_URL or DATABASE_URL from the environment.
/// Runs all migrations before returning.
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
        .merge(conduit::api::deployments::routes())
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
