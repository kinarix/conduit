use reqwest::{header, Client as HttpClient, StatusCode};
use serde::Serialize;
use std::time::Duration;
use thiserror::Error;
use uuid::Uuid;

use crate::types::{ExternalTask, Variable};

/// Connection settings for talking to a Conduit engine.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub base_url: String,
    pub api_key: Option<String>,
    pub request_timeout: Duration,
}

impl ClientConfig {
    /// Construct a config pointing at the given base URL with sensible
    /// defaults: 30s request timeout, no auth.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            api_key: None,
            request_timeout: Duration::from_secs(30),
        }
    }

    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    pub fn with_request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }
}

/// Typed wrapper over the engine's `/api/v1/external-tasks/*` endpoints.
#[derive(Debug, Clone)]
pub struct Client {
    http: HttpClient,
    base_url: String,
}

impl Client {
    pub fn new(config: ClientConfig) -> Result<Self, ClientError> {
        let mut headers = header::HeaderMap::new();
        if let Some(key) = &config.api_key {
            let mut v = header::HeaderValue::from_str(&format!("Bearer {key}"))
                .map_err(|_| ClientError::InvalidConfig("api_key contains non-ASCII".into()))?;
            v.set_sensitive(true);
            headers.insert(header::AUTHORIZATION, v);
        }
        let http = HttpClient::builder()
            .default_headers(headers)
            .timeout(config.request_timeout)
            .build()
            .map_err(ClientError::from)?;
        Ok(Self {
            http,
            base_url: config.base_url.trim_end_matches('/').to_string(),
        })
    }

    /// `POST /api/v1/external-tasks/fetch-and-lock`.
    pub async fn fetch_and_lock(
        &self,
        worker_id: &str,
        topic: &str,
        max_jobs: u32,
        lock_duration_secs: u32,
    ) -> Result<Vec<ExternalTask>, ClientError> {
        let body = FetchAndLockRequest {
            worker_id,
            topic: Some(topic),
            max_jobs,
            lock_duration_secs,
        };
        let resp = self
            .http
            .post(format!(
                "{}/api/v1/external-tasks/fetch-and-lock",
                self.base_url
            ))
            .json(&body)
            .send()
            .await?;
        let resp = expect_2xx(resp).await?;
        Ok(resp.json::<Vec<ExternalTask>>().await?)
    }

    /// `POST /api/v1/external-tasks/{id}/complete`.
    pub async fn complete(
        &self,
        task_id: Uuid,
        worker_id: &str,
        variables: &[Variable],
    ) -> Result<(), ClientError> {
        let body = CompleteRequest {
            worker_id,
            variables,
        };
        let resp = self
            .http
            .post(format!(
                "{}/api/v1/external-tasks/{}/complete",
                self.base_url, task_id
            ))
            .json(&body)
            .send()
            .await?;
        expect_2xx(resp).await?;
        Ok(())
    }

    /// `POST /api/v1/external-tasks/{id}/failure`.
    pub async fn failure(
        &self,
        task_id: Uuid,
        worker_id: &str,
        error_message: &str,
    ) -> Result<(), ClientError> {
        let body = FailureRequest {
            worker_id,
            error_message,
        };
        let resp = self
            .http
            .post(format!(
                "{}/api/v1/external-tasks/{}/failure",
                self.base_url, task_id
            ))
            .json(&body)
            .send()
            .await?;
        expect_2xx(resp).await?;
        Ok(())
    }

    /// `POST /api/v1/external-tasks/{id}/bpmn-error`.
    pub async fn bpmn_error(
        &self,
        task_id: Uuid,
        worker_id: &str,
        error_code: &str,
        error_message: &str,
        variables: &[Variable],
    ) -> Result<(), ClientError> {
        let body = BpmnErrorRequest {
            worker_id,
            error_code,
            error_message,
            variables,
        };
        let resp = self
            .http
            .post(format!(
                "{}/api/v1/external-tasks/{}/bpmn-error",
                self.base_url, task_id
            ))
            .json(&body)
            .send()
            .await?;
        expect_2xx(resp).await?;
        Ok(())
    }

    /// `POST /api/v1/external-tasks/{id}/extend-lock`.
    pub async fn extend_lock(
        &self,
        task_id: Uuid,
        worker_id: &str,
        lock_duration_secs: u32,
    ) -> Result<(), ClientError> {
        let body = ExtendLockRequest {
            worker_id,
            lock_duration_secs,
        };
        let resp = self
            .http
            .post(format!(
                "{}/api/v1/external-tasks/{}/extend-lock",
                self.base_url, task_id
            ))
            .json(&body)
            .send()
            .await?;
        expect_2xx(resp).await?;
        Ok(())
    }
}

async fn expect_2xx(resp: reqwest::Response) -> Result<reqwest::Response, ClientError> {
    let status = resp.status();
    if status.is_success() {
        return Ok(resp);
    }
    let body = resp.text().await.unwrap_or_default();
    Err(ClientError::Http { status, body })
}

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("transport error: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("invalid config: {0}")]
    InvalidConfig(String),
    #[error("engine returned {status}: {body}")]
    Http { status: StatusCode, body: String },
}

#[derive(Serialize)]
struct FetchAndLockRequest<'a> {
    worker_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    topic: Option<&'a str>,
    max_jobs: u32,
    lock_duration_secs: u32,
}

#[derive(Serialize)]
struct CompleteRequest<'a> {
    worker_id: &'a str,
    variables: &'a [Variable],
}

#[derive(Serialize)]
struct FailureRequest<'a> {
    worker_id: &'a str,
    error_message: &'a str,
}

#[derive(Serialize)]
struct BpmnErrorRequest<'a> {
    worker_id: &'a str,
    error_code: &'a str,
    error_message: &'a str,
    variables: &'a [Variable],
}

#[derive(Serialize)]
struct ExtendLockRequest<'a> {
    worker_id: &'a str,
    lock_duration_secs: u32,
}
