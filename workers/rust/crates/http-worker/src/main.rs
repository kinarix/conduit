use clap::Parser;
use conduit_worker::{Client, ClientConfig, Runner, RunnerConfig};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tracing_subscriber::EnvFilter;

mod config;
mod handler;
mod render;

use config::WorkerConfig;
use handler::HttpHandler;

#[derive(Debug, Parser)]
#[command(
    name = "http-worker",
    version,
    about = "Reference HTTP worker for Conduit (replaces <conduit:http>)"
)]
struct Args {
    /// Path to the worker YAML config (engine URL + per-topic handler config).
    #[arg(
        short,
        long,
        default_value = "worker.yaml",
        env = "CONDUIT_WORKER_CONFIG"
    )]
    config: PathBuf,

    /// Override the worker_id reported to the engine. Defaults to
    /// `http-worker-<hostname>-<pid>`.
    #[arg(long, env = "CONDUIT_WORKER_ID")]
    worker_id: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .json()
        .init();

    let args = Args::parse();
    let cfg = WorkerConfig::from_yaml_file(&args.config)?;

    let api_key = cfg
        .engine
        .api_key_env
        .as_deref()
        .and_then(|name| std::env::var(name).ok());
    let mut client_cfg =
        ClientConfig::new(&cfg.engine.url).with_request_timeout(Duration::from_secs(60));
    if let Some(k) = api_key {
        client_cfg = client_cfg.with_api_key(k);
    }
    let client = Client::new(client_cfg)?;

    let worker_id = args.worker_id.unwrap_or_else(default_worker_id);
    tracing::info!(worker_id = %worker_id, engine = %cfg.engine.url, "starting http-worker");

    let mut tasks: Vec<tokio::task::JoinHandle<()>> = Vec::with_capacity(cfg.handlers.len());
    for (topic, hc) in cfg.handlers {
        let handler = HttpHandler::new(topic.clone(), hc)?;
        let runner = Runner::new(
            client.clone(),
            Arc::new(handler),
            RunnerConfig::new(worker_id.clone()),
        );
        tracing::info!(topic = %topic, "subscribed");
        tasks.push(tokio::spawn(async move {
            runner.run().await;
        }));
    }

    if tasks.is_empty() {
        return Err("no handlers configured in worker.yaml".into());
    }
    for t in tasks {
        let _ = t.await;
    }
    Ok(())
}

fn default_worker_id() -> String {
    let host = hostname();
    let pid = std::process::id();
    format!("http-worker-{host}-{pid}")
}

fn hostname() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("HOST"))
        .unwrap_or_else(|_| "unknown".into())
}
