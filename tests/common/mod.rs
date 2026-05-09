use axum::Router;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

use conduit::engine::{Engine, VariableInput};
use conduit::state::{AppState, GraphCache};

#[allow(dead_code)]
pub mod auth;

#[allow(dead_code)]
pub struct TestApp {
    pub address: String,
    pub pool: PgPool,
    /// A default org + internal-auth user created at app spawn. Tests that
    /// don't care about multi-tenant isolation use `principal.token` to
    /// authenticate. Tests that need cross-org checks call
    /// `create_extra_principal()` for a second principal.
    pub principal: TestPrincipal,
    /// reqwest client with the default principal's Bearer token already
    /// attached as a default header. Use `app.client.clone()` instead of
    /// `reqwest::Client::new()` for protected endpoints.
    pub client: reqwest::Client,
}

#[allow(dead_code)]
#[derive(Clone)]
pub struct TestPrincipal {
    pub user_id: Uuid,
    pub org_id: Uuid,
    pub token: String,
}

#[allow(dead_code)]
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

    // Fixed test key — deterministic so tests are reproducible.
    let test_secrets_key = [0xA5u8; 32];
    let auth_settings = conduit::auth::AuthSettings {
        jwt_keys: conduit::auth::JwtKeys::from_secret(auth::TEST_JWT_KEY),
        jwt_ttl: chrono::Duration::seconds(3600),
        jwt_issuer: auth::TEST_JWT_ISSUER.to_string(),
        tenant_isolation: conduit::config::TenantIsolation::Multi,
    };
    let state = Arc::new(AppState::new(pool.clone(), test_secrets_key, auth_settings));

    let app = Router::new()
        .merge(conduit::api::health::routes())
        .merge(conduit::api::auth::routes())
        .merge(conduit::api::orgs::routes())
        .merge(conduit::api::users::routes())
        .merge(conduit::api::process_groups::routes())
        .merge(conduit::api::deployments::routes())
        .merge(conduit::api::instances::routes())
        .merge(conduit::api::tasks::routes())
        .merge(conduit::api::external_tasks::routes())
        .merge(conduit::api::messages::routes())
        .merge(conduit::api::signals::routes())
        .merge(conduit::api::decisions::routes())
        .merge(conduit::api::process_layouts::routes())
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

    let principal = create_principal(&pool, "test-org").await;
    let client = auth::authed_client(&principal.token);

    TestApp {
        address,
        pool,
        principal,
        client,
    }
}

/// Create an org + internal-auth user + JWT directly in the DB, bypassing
/// the (now-authenticated) HTTP endpoints. Used by `spawn_test_app` for
/// the default principal and by tests that need additional principals.
#[allow(dead_code)]
pub async fn create_principal(pool: &PgPool, slug_prefix: &str) -> TestPrincipal {
    let slug = format!("{slug_prefix}-{}", Uuid::new_v4());
    let org = conduit::db::orgs::insert(pool, "Test Org", &slug)
        .await
        .expect("create org");
    let email = format!("user-{}@test.local", Uuid::new_v4());
    let user = conduit::db::users::insert(pool, org.id, "internal", None, &email, None)
        .await
        .expect("create user");
    let token = auth::mint_jwt(user.id, org.id);
    TestPrincipal {
        user_id: user.id,
        org_id: org.id,
        token,
    }
}

/// Returns the default principal's org_id. The default principal's JWT
/// is already attached to `app.client`, so HTTP calls using `app.client`
/// will be scoped to this org. Tests that specifically need a SECOND
/// org (e.g. cross-tenant isolation tests) should call `create_principal`
/// and `auth::authed_client(&p.token)` for the extra principal.
#[allow(dead_code)]
pub async fn create_test_org(app: &TestApp) -> Uuid {
    app.principal.org_id
}

/// Direct DB insert of a process group. No HTTP, no auth.
#[allow(dead_code)]
pub async fn create_test_process_group(app: &TestApp, org_id: Uuid, name: &str) -> Uuid {
    conduit::db::process_groups::insert(&app.pool, org_id, name)
        .await
        .expect("create process group")
        .id
}

/// Returns the default-principal's org plus `count` process groups under it.
#[allow(dead_code)]
pub async fn create_test_org_with_groups(app: &TestApp, count: usize) -> (Uuid, Vec<Uuid>) {
    let org_id = app.principal.org_id;
    let mut groups = Vec::with_capacity(count);
    for i in 0..count {
        let name = format!("Group {}", i + 1);
        groups.push(create_test_process_group(app, org_id, &name).await);
    }
    (org_id, groups)
}

// ─── Engine-direct helpers ────────────────────────────────────────────────────
//
// These bypass the HTTP layer and exercise the engine through `conduit::engine::Engine`
// directly. Used by engine_test, exclusive_gateway_test, script_task_test, etc.

/// Spin up a real DB pool + Engine wired to the test database.
#[allow(dead_code)]
pub async fn engine_setup() -> (PgPool, Engine) {
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

    let cache: GraphCache = Arc::new(RwLock::new(HashMap::new()));
    let engine = Engine::new(pool.clone(), cache, [0xA5u8; 32]);
    (pool, engine)
}

/// Create an org with two process groups via direct DB inserts (no HTTP).
/// Returns (org_id, [primary_group, secondary_group]).
#[allow(dead_code)]
pub async fn create_engine_org(pool: &PgPool) -> (Uuid, Vec<Uuid>) {
    let slug = format!("eng-org-{}", Uuid::new_v4());
    let org = conduit::db::orgs::insert(pool, "Engine Test Org", &slug)
        .await
        .unwrap();
    let f1 = conduit::db::process_groups::insert(pool, org.id, "Primary")
        .await
        .unwrap();
    let f2 = conduit::db::process_groups::insert(pool, org.id, "Secondary")
        .await
        .unwrap();
    (org.id, vec![f1.id, f2.id])
}

/// Per-test process_key suffix to avoid clashes when tests share a database.
#[allow(dead_code)]
pub fn unique_key(prefix: &str) -> String {
    format!("{}-{}", prefix, Uuid::new_v4())
}

/// Build a `VariableInput` with less ceremony than a struct literal.
#[allow(dead_code)]
pub fn var(name: &str, value_type: &str, value: serde_json::Value) -> VariableInput {
    VariableInput {
        name: name.to_string(),
        value_type: value_type.to_string(),
        value,
    }
}
