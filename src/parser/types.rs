use std::collections::HashMap;

/// Phase 16 — declarative HTTP connector config attached to a `<serviceTask>`
/// via `<extensionElements><conduit:http>`. Round-trips through `jobs.config`
/// (JSONB) so re-deploying a definition cannot mutate in-flight calls.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HttpConfig {
    /// HTTP method. Default: `POST`.
    #[serde(default = "default_method")]
    pub method: String,
    /// Per-task timeout. `None` falls back to the global reqwest default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    /// Authentication strategy. Default: `none`.
    #[serde(default)]
    pub auth: HttpAuth,
    /// Name of an org-scoped secret to attach as the credential. Required for
    /// `basic`/`bearer`/`api_key` auth, ignored for `none`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_ref: Option<String>,
    /// For `api_key` auth: which header to set (e.g. `X-API-Key`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key_header: Option<String>,
    /// jq filter source. Input doc: `{instance_id, execution_id, vars}`.
    /// Output doc: `{body?, headers?, query?, path?}`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_transform: Option<String>,
    /// jq filter source. Input doc: `{status, headers, body}`.
    /// Output doc: flat `{var_name: value, ...}` to upsert into instance vars.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_transform: Option<String>,
    /// jq expression evaluated against `{status, headers, body}` for every
    /// response. If it returns a non-empty string, that string is treated as a
    /// BPMN error code and the token is routed to the matching
    /// `BoundaryErrorEvent` instead of the normal completion path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code_expression: Option<String>,
    /// Retry policy. Defaults to no retries.
    #[serde(default)]
    pub retry: RetryPolicy,
}

fn default_method() -> String {
    "POST".to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HttpAuth {
    #[default]
    None,
    Basic,
    Bearer,
    ApiKey,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RetryPolicy {
    #[serde(default)]
    pub max: u32,
    #[serde(default = "default_backoff_ms")]
    pub backoff_ms: u64,
    #[serde(default = "default_multiplier")]
    pub multiplier: f64,
    /// Comma-free list of conditions: any of `4xx`, `5xx`, `timeout`, `network`.
    /// Empty = retry on `5xx` + `timeout` + `network` (the safe defaults).
    #[serde(default)]
    pub retry_on: Vec<String>,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max: 0,
            backoff_ms: default_backoff_ms(),
            multiplier: default_multiplier(),
            retry_on: Vec::new(),
        }
    }
}

fn default_backoff_ms() -> u64 {
    1000
}

fn default_multiplier() -> f64 {
    2.0
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum TimerSpec {
    Duration(String),
    Cycle(String),
    Date(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum FlowNodeKind {
    StartEvent,
    TimerStartEvent {
        timer: TimerSpec,
    },
    MessageStartEvent {
        message_name: String,
    },
    EndEvent,
    UserTask,
    ServiceTask {
        topic: Option<String>,
        url: Option<String>,
        /// Phase 16: full HTTP connector config sourced from
        /// `<extensionElements><conduit:http>...</conduit:http></extensionElements>`.
        /// `None` for legacy URL-only tasks and external-worker tasks.
        http: Option<HttpConfig>,
    },
    ExclusiveGateway {
        default_flow: Option<String>,
    },
    InclusiveGateway {
        default_flow: Option<String>,
    },
    ParallelGateway,
    IntermediateTimerCatchEvent {
        timer: TimerSpec,
    },
    IntermediateMessageCatchEvent {
        message_name: String,
        correlation_key_expr: Option<String>,
    },
    ReceiveTask {
        message_name: String,
        correlation_key_expr: Option<String>,
    },
    BoundaryTimerEvent {
        timer: TimerSpec,
        attached_to: String,
        cancelling: bool,
    },
    SignalStartEvent {
        signal_name: String,
    },
    IntermediateSignalCatchEvent {
        signal_name: String,
    },
    BoundarySignalEvent {
        signal_name: String,
        attached_to: String,
        cancelling: bool,
    },
    BoundaryErrorEvent {
        error_code: Option<String>,
        attached_to: String,
        cancelling: bool,
    },
    SubProcess {
        sub_graph: Box<ProcessGraph>,
    },
    BusinessRuleTask {
        decision_ref: String,
    },
    SendTask {
        message_name: String,
    },
    ScriptTask {
        script: String,
        result_variable: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct FlowNode {
    pub id: String,
    pub name: Option<String>,
    pub kind: FlowNodeKind,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SequenceFlow {
    pub id: String,
    pub source_ref: String,
    pub target_ref: String,
    pub condition: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProcessGraph {
    pub process_id: String,
    pub process_name: Option<String>,
    pub nodes: HashMap<String, FlowNode>,
    pub flows: Vec<SequenceFlow>,
    /// outgoing[source_id] = [target_id, ...]
    pub outgoing: HashMap<String, Vec<String>>,
    /// incoming[target_id] = [source_id, ...]
    pub incoming: HashMap<String, Vec<String>>,
    /// attached_to[host_element_id] = [boundary_event_ids, ...]
    pub attached_to: HashMap<String, Vec<String>>,
    /// JSON Schema for validating start_instance input variables.
    pub input_schema: Option<serde_json::Value>,
}
