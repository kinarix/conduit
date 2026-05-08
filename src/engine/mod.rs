mod evaluator;
mod external_task;
mod helpers;
mod http;
mod instance;
mod jq;
mod message;
mod send_message;
mod signal;
mod timer;
mod token;
mod user_task;

pub use evaluator::evaluate_expression;
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
    /// Reused across HTTP service-task calls so the connection pool isn't
    /// rebuilt per request. Per-task `timeoutMs` is applied via
    /// `RequestBuilder::timeout()` rather than the client builder.
    http_client: reqwest::Client,
    /// Compiled jq filter cache for HTTP request/response transforms.
    jq_cache: jq::JqCache,
    /// ChaCha20-Poly1305 master key used to decrypt secrets at HTTP fire time.
    /// Cloned from `AppState::secrets_key`.
    secrets_key: [u8; 32],
}

impl Engine {
    pub fn new(pool: PgPool, process_cache: GraphCache, secrets_key: [u8; 32]) -> Self {
        Self {
            pool,
            process_cache,
            http_client: reqwest::Client::new(),
            jq_cache: jq::JqCache::new(),
            secrets_key,
        }
    }

    async fn load_graph(&self, definition_id: Uuid) -> Result<Arc<ProcessGraph>> {
        {
            let cache = self
                .process_cache
                .read()
                .map_err(|_| EngineError::Internal("process cache lock poisoned".to_string()))?;
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
            let mut cache = self
                .process_cache
                .write()
                .map_err(|_| EngineError::Internal("process cache lock poisoned".to_string()))?;
            cache.insert(definition_id, Arc::clone(&arc));
        }
        Ok(arc)
    }
}
