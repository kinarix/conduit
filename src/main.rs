use axum::Router;
use conduit::{api, config, db, state::AppState};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = config::Config::from_env()?;

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(&config.log_level))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!(version = env!("CARGO_PKG_VERSION"), "Starting Conduit");

    let pool = db::connect(&config).await?;
    tracing::info!("Database connected");

    sqlx::migrate!("./migrations").run(&pool).await?;
    tracing::info!("Migrations applied");

    let state = Arc::new(AppState::new(pool, config.secrets_key));

    // Background timer executor: polls for due timer jobs every second.
    let executor_state = Arc::clone(&state);
    tokio::spawn(async move {
        loop {
            match executor_state.engine.fire_due_timer_jobs().await {
                Ok(n) if n > 0 => tracing::debug!(fired = n, "Timer jobs fired"),
                Err(e) => tracing::error!(error = %e, "Timer executor error"),
                _ => {}
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    });

    // Background HTTP service task executor: calls out to configured URLs every second.
    let http_executor_state = Arc::clone(&state);
    tokio::spawn(async move {
        loop {
            match http_executor_state.engine.fire_due_http_tasks().await {
                Ok(n) if n > 0 => tracing::debug!(fired = n, "HTTP service tasks fired"),
                Err(e) => tracing::error!(error = %e, "HTTP executor error"),
                _ => {}
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    });

    // Background send-message executor: delivers queued SendTask messages every second.
    let send_msg_state = Arc::clone(&state);
    tokio::spawn(async move {
        loop {
            match send_msg_state.engine.fire_due_send_message_jobs().await {
                Ok(n) if n > 0 => tracing::debug!(fired = n, "Send message jobs fired"),
                Err(e) => tracing::error!(error = %e, "Send message executor error"),
                _ => {}
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    });

    // Background timer-start executor: fires due timer-start triggers every second.
    let timer_start_state = Arc::clone(&state);
    tokio::spawn(async move {
        loop {
            match timer_start_state
                .engine
                .fire_due_timer_start_triggers()
                .await
            {
                Ok(n) if n > 0 => tracing::debug!(fired = n, "Timer start triggers fired"),
                Err(e) => tracing::error!(error = %e, "Timer start executor error"),
                _ => {}
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    });

    let app = Router::new()
        .merge(api::health::routes())
        .merge(api::orgs::routes())
        .merge(api::users::routes())
        .merge(api::deployments::routes())
        .merge(api::instances::routes())
        .merge(api::tasks::routes())
        .merge(api::external_tasks::routes())
        .merge(api::messages::routes())
        .merge(api::signals::routes())
        .merge(api::decisions::routes())
        .merge(api::process_groups::routes())
        .merge(api::secrets::routes())
        .layer(CorsLayer::very_permissive())
        .with_state(state);

    let addr = format!("{}:{}", config.server_host, config.server_port);
    tracing::info!(address = %addr, "Server listening");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
