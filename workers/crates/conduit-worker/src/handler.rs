use async_trait::async_trait;

use crate::types::{ExternalTask, Variable};

/// What a handler returns after running its side effect.
#[derive(Debug, Clone)]
pub enum HandlerResult {
    /// Task succeeded — engine completes the task and merges variables.
    Complete { variables: Vec<Variable> },
    /// Task hit a *business* failure that the BPMN expects via a
    /// `boundaryEvent error` — the engine throws the BPMN error.
    BpmnError {
        code: String,
        message: String,
        variables: Vec<Variable>,
    },
}

impl HandlerResult {
    /// Convenience: complete with no variable updates.
    pub fn ok() -> Self {
        Self::Complete {
            variables: Vec::new(),
        }
    }

    /// Convenience: complete with the given variables.
    pub fn complete(variables: impl Into<Vec<Variable>>) -> Self {
        Self::Complete {
            variables: variables.into(),
        }
    }

    /// Convenience: throw a BPMN error with the given code.
    pub fn bpmn_error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::BpmnError {
            code: code.into(),
            message: message.into(),
            variables: Vec::new(),
        }
    }
}

/// What a handler returns when it cannot complete the task and wants
/// the engine to retry (or, if retries are exhausted, mark it failed).
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct HandlerError {
    pub message: String,
}

impl HandlerError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl From<&str> for HandlerError {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for HandlerError {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

/// Implement this trait for each topic your worker fleet handles.
///
/// The runner invokes [`Handler::handle`] once per locked task, then
/// reports the result back to the engine. Handlers MUST be idempotent
/// under retry — see `docs/idempotency-store.md` in this repo for the
/// pattern (Idempotency-Key header derived from `task.id`, plus a small
/// dedupe table for handlers that need replay-safe writes).
#[async_trait]
pub trait Handler: Send + Sync {
    /// The topic this handler subscribes to.
    fn topic(&self) -> &str;

    /// Run the side effect. Return `Ok(HandlerResult)` on success or a
    /// completed-with-BPMN-error; return `Err(HandlerError)` to ask the
    /// engine to fail-and-retry.
    async fn handle(&self, task: &ExternalTask) -> Result<HandlerResult, HandlerError>;
}
