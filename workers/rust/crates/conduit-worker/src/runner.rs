use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

use crate::client::Client;
use crate::handler::{Handler, HandlerResult};
use crate::types::ExternalTask;

/// Runner-loop tunables. Most workers can use [`RunnerConfig::default`].
#[derive(Debug, Clone)]
pub struct RunnerConfig {
    /// Identifier the engine sees on every fetch / complete / failure.
    /// Pin to a stable per-process ID (hostname + PID is fine for v1).
    pub worker_id: String,
    /// How many tasks to fetch per round. The engine clamps to <= 100.
    pub max_jobs_per_fetch: u32,
    /// Lock TTL on fetch. The handler must complete (or call extend) before this.
    pub lock_duration_secs: u32,
    /// Sleep between fetch rounds when the engine returns an empty list.
    /// Long polling (Phase 17) on the engine side will reduce this in practice.
    pub idle_backoff: Duration,
    /// Sleep between fetch rounds when fetching itself errored.
    pub error_backoff: Duration,
}

impl RunnerConfig {
    pub fn new(worker_id: impl Into<String>) -> Self {
        Self {
            worker_id: worker_id.into(),
            max_jobs_per_fetch: 10,
            lock_duration_secs: 60,
            idle_backoff: Duration::from_millis(500),
            error_backoff: Duration::from_secs(5),
        }
    }
}

/// The fetch-handle-report loop. One runner serves one topic; spawn
/// multiple runners (typically one per [`Handler`]) for multi-topic
/// fleets.
pub struct Runner {
    client: Client,
    handler: Arc<dyn Handler>,
    config: RunnerConfig,
}

impl Runner {
    pub fn new(client: Client, handler: Arc<dyn Handler>, config: RunnerConfig) -> Self {
        Self {
            client,
            handler,
            config,
        }
    }

    /// Run forever (until cancelled). Cancellation in v1 is "drop the
    /// future"; a future revision will accept a `tokio_util` cancellation
    /// token for graceful drain.
    pub async fn run(self) {
        let topic = self.handler.topic().to_string();
        loop {
            match self
                .client
                .fetch_and_lock(
                    &self.config.worker_id,
                    &topic,
                    self.config.max_jobs_per_fetch,
                    self.config.lock_duration_secs,
                )
                .await
            {
                Ok(tasks) if tasks.is_empty() => {
                    sleep(self.config.idle_backoff).await;
                }
                Ok(tasks) => {
                    for task in tasks {
                        self.dispatch(task).await;
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        topic = %topic,
                        worker_id = %self.config.worker_id,
                        error = %e,
                        "fetch_and_lock failed"
                    );
                    sleep(self.config.error_backoff).await;
                }
            }
        }
    }

    async fn dispatch(&self, task: ExternalTask) {
        let task_id = task.id;
        let span = tracing::info_span!(
            "handle_task",
            task_id = %task_id,
            instance_id = %task.instance_id,
            topic = ?task.topic,
        );
        let _enter = span.enter();

        let outcome = self.handler.handle(&task).await;
        match outcome {
            Ok(HandlerResult::Complete { variables }) => {
                if let Err(e) = self
                    .client
                    .complete(task_id, &self.config.worker_id, &variables)
                    .await
                {
                    tracing::error!(error = %e, "complete failed");
                }
            }
            Ok(HandlerResult::BpmnError {
                code,
                message,
                variables,
            }) => {
                if let Err(e) = self
                    .client
                    .bpmn_error(task_id, &self.config.worker_id, &code, &message, &variables)
                    .await
                {
                    tracing::error!(error = %e, "bpmn_error failed");
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "handler returned failure");
                if let Err(report_err) = self
                    .client
                    .failure(task_id, &self.config.worker_id, &e.message)
                    .await
                {
                    tracing::error!(error = %report_err, "failure report failed");
                }
            }
        }
    }
}
