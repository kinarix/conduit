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
}

impl AppState {
    pub fn new(pool: PgPool) -> Self {
        let process_cache: GraphCache = Arc::new(RwLock::new(HashMap::new()));
        let engine = Engine::new(pool.clone(), Arc::clone(&process_cache));
        Self {
            pool,
            process_cache,
            engine,
        }
    }
}
