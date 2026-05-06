use crate::engine::Engine;
use crate::leader::LeaderElector;
use crate::parser::ProcessGraph;
use metrics_exporter_prometheus::PrometheusHandle;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Instant;
use uuid::Uuid;

pub type GraphCache = Arc<RwLock<HashMap<Uuid, Arc<ProcessGraph>>>>;

pub struct AppState {
    pub pool: PgPool,
    pub process_cache: GraphCache,
    pub engine: Engine,
    /// ChaCha20-Poly1305 master key for `secrets` table values. 32 bytes.
    pub secrets_key: [u8; 32],
    /// Process start time for uptime reporting.
    pub started_at: Instant,
    /// Leader election handle. `None` in single-instance (no election) mode.
    pub leader: Option<Arc<LeaderElector>>,
    /// Prometheus metrics handle. Used by `GET /metrics`.
    pub prometheus_handle: Option<PrometheusHandle>,
}

impl AppState {
    pub fn new(pool: PgPool, secrets_key: [u8; 32]) -> Self {
        let process_cache: GraphCache = Arc::new(RwLock::new(HashMap::new()));
        let engine = Engine::new(pool.clone(), Arc::clone(&process_cache), secrets_key);
        Self {
            pool,
            process_cache,
            engine,
            secrets_key,
            started_at: Instant::now(),
            leader: None,
            prometheus_handle: None,
        }
    }

    /// Returns `true` if this instance is the current cluster leader, or
    /// `true` if leader election is not configured (single-instance mode).
    pub fn is_leader(&self) -> bool {
        self.leader.as_ref().is_none_or(|l| l.is_leader())
    }
}
