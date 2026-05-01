use crate::engine::Engine;
use crate::parser::ProcessGraph;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

pub type GraphCache = Arc<RwLock<HashMap<Uuid, Arc<ProcessGraph>>>>;

pub struct AppState {
    pub pool: PgPool,
    pub process_cache: GraphCache,
    pub engine: Engine,
    /// ChaCha20-Poly1305 master key for `secrets` table values. 32 bytes.
    pub secrets_key: [u8; 32],
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
        }
    }
}
