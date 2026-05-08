use async_trait::async_trait;
use conduit_worker::{ExternalTask, Handler, HandlerError, HandlerResult, Variable};
use reqwest::{header, Client as HttpClient, Method};
use std::str::FromStr;

use crate::config::{AuthConfig, HandlerConfig};
use crate::render::{jsonpath, render, render_json};

/// One handler instance, bound to one topic.
pub struct HttpHandler {
    topic: String,
    config: HandlerConfig,
    http: HttpClient,
    auth_header: Option<(header::HeaderName, String)>,
}

impl HttpHandler {
    pub fn new(topic: String, config: HandlerConfig) -> Result<Self, BuildError> {
        let http = HttpClient::builder()
            .timeout(config.timeout())
            .build()
            .map_err(BuildError::from)?;
        let auth_header = resolve_auth_header(config.auth.as_ref())?;
        Ok(Self {
            topic,
            config,
            http,
            auth_header,
        })
    }
}

fn resolve_auth_header(
    auth: Option<&AuthConfig>,
) -> Result<Option<(header::HeaderName, String)>, BuildError> {
    let Some(auth) = auth else {
        return Ok(None);
    };
    match auth {
        AuthConfig::Bearer { token_env } => {
            let token = std::env::var(token_env)
                .map_err(|_| BuildError::Env(format!("{token_env} (auth.token_env)")))?;
            Ok(Some((header::AUTHORIZATION, format!("Bearer {token}"))))
        }
        AuthConfig::Basic {
            user_env,
            password_env,
        } => {
            let user = std::env::var(user_env)
                .map_err(|_| BuildError::Env(format!("{user_env} (auth.user_env)")))?;
            let pass = std::env::var(password_env)
                .map_err(|_| BuildError::Env(format!("{password_env} (auth.password_env)")))?;
            let creds = base64_pretend::encode(format!("{user}:{pass}"));
            Ok(Some((header::AUTHORIZATION, format!("Basic {creds}"))))
        }
    }
}

#[async_trait]
impl Handler for HttpHandler {
    fn topic(&self) -> &str {
        &self.topic
    }

    async fn handle(&self, task: &ExternalTask) -> Result<HandlerResult, HandlerError> {
        let vars = task.variable_map();
        let task_id_str = task.id.to_string();

        let url = render(&self.config.url_template, &vars, &task_id_str);
        let method = Method::from_str(&self.config.method.to_uppercase()).map_err(|_| {
            HandlerError::new(format!("invalid HTTP method: {}", self.config.method))
        })?;

        let mut req = self.http.request(method, &url);

        for (k, v) in &self.config.headers {
            let v = render(v, &vars, &task_id_str);
            req = req.header(k.as_str(), v);
        }
        if let Some((name, value)) = &self.auth_header {
            req = req.header(name, value);
        }
        if self.config.idempotency.enabled {
            let key = render(&self.config.idempotency.key_template, &vars, &task_id_str);
            req = req.header(self.config.idempotency.header.as_str(), key);
        }

        if let Some(body) = &self.config.request_template {
            let rendered = render_json(body, &vars, &task_id_str);
            req = req.json(&rendered);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| HandlerError::new(format!("transport: {e}")))?;
        let status = resp.status();
        let body_text = resp.text().await.unwrap_or_default();

        if status.is_success() {
            let body_json: serde_json::Value =
                serde_json::from_str(&body_text).unwrap_or(serde_json::Value::Null);
            let mut out_vars = Vec::with_capacity(self.config.response_mapping.len());
            for (var_name, path) in &self.config.response_mapping {
                let value = jsonpath(path, &body_json)
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                out_vars.push(Variable::json(var_name.clone(), value));
            }
            return Ok(HandlerResult::Complete {
                variables: out_vars,
            });
        }

        if status.is_client_error() {
            if let Some(code) = &self.config.bpmn_error_on_4xx {
                return Ok(HandlerResult::BpmnError {
                    code: code.clone(),
                    message: format!("HTTP {status}: {}", truncate(&body_text, 256)),
                    variables: vec![
                        Variable::long("http_status", status.as_u16() as i64),
                        Variable::string("http_body", body_text),
                    ],
                });
            }
        }

        Err(HandlerError::new(format!(
            "HTTP {status}: {}",
            truncate(&body_text, 256)
        )))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("reqwest: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("env var not set: {0}")]
    Env(String),
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut boundary = max;
    while boundary > 0 && !s.is_char_boundary(boundary) {
        boundary -= 1;
    }
    format!("{}…", &s[..boundary])
}

// Tiny base64 stand-in so we don't pull a full dep just for Basic auth.
mod base64_pretend {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    pub fn encode(input: impl AsRef<[u8]>) -> String {
        let bytes = input.as_ref();
        let mut out = String::with_capacity((bytes.len().div_ceil(3)) * 4);
        for chunk in bytes.chunks(3) {
            let b0 = chunk[0];
            let b1 = chunk.get(1).copied().unwrap_or(0);
            let b2 = chunk.get(2).copied().unwrap_or(0);
            out.push(ALPHABET[(b0 >> 2) as usize] as char);
            out.push(ALPHABET[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
            if chunk.len() > 1 {
                out.push(ALPHABET[(((b1 & 0x0F) << 2) | (b2 >> 6)) as usize] as char);
            } else {
                out.push('=');
            }
            if chunk.len() > 2 {
                out.push(ALPHABET[(b2 & 0x3F) as usize] as char);
            } else {
                out.push('=');
            }
        }
        out
    }

    #[test]
    fn encodes_known_pairs() {
        assert_eq!(encode("user:pass"), "dXNlcjpwYXNz");
        assert_eq!(encode(""), "");
        assert_eq!(encode("a"), "YQ==");
        assert_eq!(encode("ab"), "YWI=");
        assert_eq!(encode("abc"), "YWJj");
    }
}
