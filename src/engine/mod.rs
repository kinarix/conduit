mod evaluator;
mod helpers;
mod token;
mod instance;
mod user_task;
mod external_task;
mod timer;
mod message;
mod signal;
mod http;
mod send_message;

pub use helpers::parse_duration;

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{EngineError, Result};
use crate::parser::ProcessGraph;
use crate::state::GraphCache;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableInput {
    pub name: String,
    pub value_type: String,
    pub value: JsonValue,
}

pub struct Engine {
    pool: PgPool,
    process_cache: GraphCache,
}

impl Engine {
    pub fn new(pool: PgPool, process_cache: GraphCache) -> Self {
        Self {
            pool,
            process_cache,
        }
    }

    async fn load_graph(&self, definition_id: Uuid) -> Result<Arc<ProcessGraph>> {
        {
            let cache = self.process_cache.read().unwrap();
            if let Some(graph) = cache.get(&definition_id) {
                return Ok(Arc::clone(graph));
            }
        }

        let row: (String,) =
            sqlx::query_as("SELECT bpmn_xml FROM process_definitions WHERE id = $1")
                .bind(definition_id)
                .fetch_optional(&self.pool)
                .await?
                .ok_or_else(|| {
                    EngineError::NotFound(format!("Process definition {definition_id} not found"))
                })?;

        let graph = crate::parser::parse(&row.0)?;
        let arc = Arc::new(graph);
        {
            let mut cache = self.process_cache.write().unwrap();
            cache.insert(definition_id, Arc::clone(&arc));
        }
        Ok(arc)
    }
}
