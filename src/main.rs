use axum::{http::StatusCode, Router};
use conduit::{
    api, config, db, error::assert_error_codes_complete, leader::LeaderElector, state::AppState,
};
use metrics_exporter_prometheus::PrometheusBuilder;
use std::sync::Arc;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = config::Config::from_env()?;

    let registry =
        tracing_subscriber::registry().with(tracing_subscriber::EnvFilter::new(&config.log_level));
    if std::env::var("CONDUIT_LOG_FORMAT").as_deref() == Ok("json") {
        registry
            .with(tracing_subscriber::fmt::layer().json())
            .init();
    } else {
        registry.with(tracing_subscriber::fmt::layer()).init();
    }

    assert_error_codes_complete();
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "Starting Conduit");

    let pool = db::connect(&config).await?;
    tracing::info!("Database connected");

    sqlx::migrate!("./migrations").run(&pool).await?;
    tracing::info!("Migrations applied");

    conduit::auth::bootstrap::run_if_needed(&pool, &config).await?;

    // Install the Prometheus recorder as the global metrics backend.
    // The handle is stored in AppState so the /metrics endpoint can render it.
    let prometheus_handle = PrometheusBuilder::new()
        .install_recorder()
        .expect("failed to install Prometheus recorder");

    let auth = conduit::auth::AuthSettings::from_config(&config);
    let mut state = AppState::new(pool, config.secrets_key, auth);
    state.prometheus_handle = Some(prometheus_handle);

    let token = CancellationToken::new();

    // Start leader election so timer/send-message executors only fire on
    // the leader — external-task fetch-and-lock is safe on all replicas
    // because it already uses FOR UPDATE SKIP LOCKED.
    let elector = LeaderElector::start(state.pool.clone(), token.clone()).await;
    state.leader = Some(Arc::new(elector));

    let state = Arc::new(state);
    let mut executors: JoinSet<()> = JoinSet::new();

    // Background timer executor: polls for due timer jobs every second.
    // Only fires when this instance is the leader to avoid duplicate work.
    let (t, s) = (token.clone(), Arc::clone(&state));
    executors.spawn(async move {
        loop {
            tokio::select! {
                biased;
                _ = t.cancelled() => break,
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(1)) => {}
            }
            if !s.is_leader() {
                continue;
            }
            match s.engine.fire_due_timer_jobs().await {
                Ok(n) if n > 0 => {
                    tracing::debug!(fired = n, "Timer jobs fired");
                    metrics::counter!("conduit_timer_jobs_fired_total").increment(n as u64);
                }
                Err(e) => {
                    tracing::error!(error = %e, "Timer executor error");
                    metrics::counter!("conduit_executor_errors_total", "executor" => "timer")
                        .increment(1);
                }
                _ => {}
            }
        }
        tracing::info!("Timer executor stopped");
    });

    // Background HTTP service task executor.
    let (t, s) = (token.clone(), Arc::clone(&state));
    executors.spawn(async move {
        loop {
            tokio::select! {
                biased;
                _ = t.cancelled() => break,
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(1)) => {}
            }
            match s.engine.fire_due_http_tasks().await {
                Ok(n) if n > 0 => {
                    tracing::debug!(fired = n, "HTTP service tasks fired");
                    metrics::counter!("conduit_http_tasks_fired_total").increment(n as u64);
                }
                Err(e) => {
                    tracing::error!(error = %e, "HTTP executor error");
                    metrics::counter!("conduit_executor_errors_total", "executor" => "http")
                        .increment(1);
                }
                _ => {}
            }
        }
        tracing::info!("HTTP task executor stopped");
    });

    // Background send-message executor.
    let (t, s) = (token.clone(), Arc::clone(&state));
    executors.spawn(async move {
        loop {
            tokio::select! {
                biased;
                _ = t.cancelled() => break,
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(1)) => {}
            }
            if !s.is_leader() { continue; }
            match s.engine.fire_due_send_message_jobs().await {
                Ok(n) if n > 0 => {
                    tracing::debug!(fired = n, "Send message jobs fired");
                    metrics::counter!("conduit_send_message_jobs_fired_total").increment(n as u64);
                }
                Err(e) => {
                    tracing::error!(error = %e, "Send message executor error");
                    metrics::counter!("conduit_executor_errors_total", "executor" => "send_message").increment(1);
                }
                _ => {}
            }
        }
        tracing::info!("Send-message executor stopped");
    });

    // Background timer-start executor.
    let (t, s) = (token.clone(), Arc::clone(&state));
    executors.spawn(async move {
        loop {
            tokio::select! {
                biased;
                _ = t.cancelled() => break,
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(1)) => {}
            }
            if !s.is_leader() {
                continue;
            }
            match s.engine.fire_due_timer_start_triggers().await {
                Ok(n) if n > 0 => {
                    tracing::debug!(fired = n, "Timer start triggers fired");
                    metrics::counter!("conduit_timer_start_triggers_fired_total")
                        .increment(n as u64);
                }
                Err(e) => {
                    tracing::error!(error = %e, "Timer start executor error");
                    metrics::counter!("conduit_executor_errors_total", "executor" => "timer_start")
                        .increment(1);
                }
                _ => {}
            }
        }
        tracing::info!("Timer-start executor stopped");
    });

    let app = Router::new()
        .merge(api::health::routes())
        .merge(api::auth::routes())
        .merge(api::admin::routes())
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
        .merge(api::process_layouts::routes())
        .merge(api::roles::routes())
        .merge(api::secrets::routes())
        .fallback(|| async {
            (
                StatusCode::NOT_FOUND,
                axum::Json(serde_json::json!({
                    "code": "U404",
                    "message": "The requested endpoint does not exist.",
                    "action": "Check the API URL and HTTP method."
                })),
            )
        })
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::very_permissive())
        .with_state(state);

    let addr = format!("{}:{}", config.server_host, config.server_port);
    tracing::info!(address = %addr, "Server listening");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    let shutdown_token = token.clone();
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let ctrl_c = tokio::signal::ctrl_c();
            #[cfg(unix)]
            {
                let mut sigterm =
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                        .expect("failed to install SIGTERM handler");
                tokio::select! {
                    _ = ctrl_c => {},
                    _ = sigterm.recv() => {},
                }
            }
            #[cfg(not(unix))]
            let _ = ctrl_c.await;

            tracing::info!("Shutdown signal received — draining executors");
            shutdown_token.cancel();
        })
        .await?;

    // Wait up to 30 s for background executors to finish their current iteration.
    let drain = tokio::time::timeout(tokio::time::Duration::from_secs(30), async move {
        while executors.join_next().await.is_some() {}
    });
    if drain.await.is_err() {
        tracing::warn!("Executor drain timed out after 30 s");
    }

    tracing::info!("Conduit stopped");
    Ok(())
}
