use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

/// Top-level `worker.yaml` schema.
#[derive(Debug, Clone, Deserialize)]
pub struct WorkerConfig {
    pub engine: EngineConfig,
    /// Per-topic handler config. Each key is a BPMN `<conduit:taskTopic>` value.
    pub handlers: BTreeMap<String, HandlerConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EngineConfig {
    pub url: String,
    /// Name of the env var that holds the engine API key (so the secret
    /// itself doesn't sit in the YAML file).
    pub api_key_env: Option<String>,
}

/// Configuration for one `http.call` topic. Templates use a tiny mustache
/// subset — `{{var:NAME}}` interpolates the task variable named `NAME`.
#[derive(Debug, Clone, Deserialize)]
pub struct HandlerConfig {
    pub url_template: String,
    #[serde(default = "default_method")]
    pub method: String,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    /// JSON body template — keys may be literal, values may contain `{{var:...}}`.
    #[serde(default)]
    pub request_template: Option<serde_json::Value>,
    /// Map response JSON paths to process variables.
    /// `{ order_id: "$.id" }` means: write `task variable order_id = response_json.id`.
    #[serde(default)]
    pub response_mapping: BTreeMap<String, String>,
    #[serde(default)]
    pub auth: Option<AuthConfig>,
    #[serde(default)]
    pub idempotency: IdempotencyConfig,
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
    /// BPMN error code to throw when the response is a 4xx (other than the
    /// configured `success_status` set). If unset, 4xx becomes a transient
    /// failure and the engine retries.
    #[serde(default)]
    pub bpmn_error_on_4xx: Option<String>,
}

impl HandlerConfig {
    pub fn timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_secs)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum AuthConfig {
    Bearer {
        token_env: String,
    },
    Basic {
        user_env: String,
        password_env: String,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub struct IdempotencyConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Header name to send the idempotency key under. RFC draft + Stripe
    /// convention is `Idempotency-Key`; some APIs prefer `X-Idempotency-Key`.
    #[serde(default = "default_idempotency_header")]
    pub header: String,
    /// Mustache template for the key value. Default `task-{{task_id}}`
    /// is collision-free across the engine fleet because task ids are uuids.
    #[serde(default = "default_idempotency_key_template")]
    pub key_template: String,
}

impl Default for IdempotencyConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            header: default_idempotency_header(),
            key_template: default_idempotency_key_template(),
        }
    }
}

fn default_method() -> String {
    "POST".into()
}
fn default_true() -> bool {
    true
}
fn default_idempotency_header() -> String {
    "Idempotency-Key".into()
}
fn default_idempotency_key_template() -> String {
    "task-{{task_id}}".into()
}
fn default_timeout_secs() -> u64 {
    30
}

impl WorkerConfig {
    pub fn from_yaml_file(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let s = std::fs::read_to_string(path.as_ref()).map_err(ConfigError::Io)?;
        let c: WorkerConfig = serde_yaml::from_str(&s).map_err(ConfigError::Parse)?;
        Ok(c)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("read worker config: {0}")]
    Io(std::io::Error),
    #[error("parse worker config: {0}")]
    Parse(serde_yaml::Error),
}
