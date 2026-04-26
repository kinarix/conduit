use crate::parser::ProcessGraph;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

pub type GraphCache = Arc<RwLock<HashMap<Uuid, Arc<ProcessGraph>>>>;

pub struct AppState {
    pub pool: PgPool,
    pub process_cache: GraphCache,
}

impl AppState {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            process_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}
