//! `conduit-worker` — library for building external workers against the
//! [Conduit](https://github.com/kinarix/conduit) BPMN engine.
//!
//! A worker subscribes to a topic, fetches tasks via
//! `/api/v1/external-tasks/fetch-and-lock`, runs whatever side effect it
//! owns, and reports the result back with `/complete`, `/failure`, or
//! `/bpmn-error`. This crate handles the loop, the lock-extension
//! bookkeeping, and the JSON wire format. Implement [`Handler`] for the
//! actual work.
//!
//! See `crates/http-worker/` for a full reference handler that replaces the
//! deprecated `<conduit:http>` element.

mod client;
mod handler;
mod runner;
mod types;

pub use client::{Client, ClientConfig};
pub use handler::{Handler, HandlerError, HandlerResult};
pub use runner::{Runner, RunnerConfig};
pub use types::{ExternalTask, Variable, VariableValue};

/// Attribute macro: turn an `async fn` into a `Handler` struct.
///
/// ```ignore
/// #[handler(topic = "http.call")]
/// async fn http_call(task: &ExternalTask) -> Result<HandlerResult, HandlerError> { /* ... */ }
/// // generates: pub struct HttpCallHandler; impl Handler for HttpCallHandler { ... }
/// ```
pub use conduit_worker_macros::handler;

/// Items the generated code from `#[handler]` reaches for. Not part of the
/// public API — the names here are subject to change without notice.
#[doc(hidden)]
pub mod __macro_support {
    pub use async_trait::async_trait;
}
