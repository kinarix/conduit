use std::collections::HashMap;
use std::sync::OnceLock;

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::json;

// ─── Error-code registry (loaded once from error_codes.toml) ─────────────────

#[derive(Deserialize)]
struct ErrorCodeEntry {
    display: String,
    action: Option<String>,
    debug: Option<String>,
}

static ERROR_CODES: OnceLock<HashMap<String, ErrorCodeEntry>> = OnceLock::new();

fn codes() -> &'static HashMap<String, ErrorCodeEntry> {
    ERROR_CODES.get_or_init(|| {
        const RAW: &str = include_str!("error_codes.toml");
        toml::from_str(RAW).expect("error_codes.toml is malformed")
    })
}

/// Call once at startup to assert every variant's code() has an entry in the
/// TOML registry. Panics if any code is missing, catching enum/TOML drift early.
pub fn assert_error_codes_complete() {
    let map = codes();
    let required = [
        "U001", "U002", "U003", "U004", "U005", "U006", "U007", "U008", "U009", "S001", "S002",
        "S003",
    ];
    for code in required {
        assert!(
            map.contains_key(code),
            "error_codes.toml is missing entry for code '{code}'"
        );
    }
}

// ─── Unified error type ───────────────────────────────────────────────────────

/// Unified error type for Conduit.
/// All errors convert to appropriate HTTP responses via IntoResponse.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Database error: {0}")]
    Database(#[source] sqlx::Error),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Unsupported BPMN element: {0}")]
    UnsupportedElement(String),

    #[error("Expression error: {0}")]
    Expression(String),

    #[error("DMN parse error: {0}")]
    DmnParse(String),

    #[error("FEEL evaluation error: {0}")]
    DmnFeel(String),

    #[error("Decision not found: {0}")]
    DmnNotFound(String),

    #[error("No DMN rule matched")]
    DmnNoMatch,

    #[error("Multiple DMN rules matched (UNIQUE hit policy)")]
    DmnMultipleMatches,
}

impl EngineError {
    /// Structured error code returned in API responses and displayed in the UI.
    ///
    /// Code registry:
    ///   U-prefix = user/client errors (4xx) — actionable by the caller.
    ///   S-prefix = system errors (5xx) — requires operator attention.
    ///
    ///   U001 — Validation      Bad input supplied by the caller
    ///   U002 — NotFound        Requested resource does not exist
    ///   U003 — Conflict        Duplicate / unique constraint violated
    ///   U004 — Parse           Invalid BPMN XML submitted by the caller
    ///   U005 — UnsupportedEl   BPMN element not implemented
    ///   U006 — DmnParse        Invalid DMN XML submitted by the caller
    ///   U007 — DmnFeel         FEEL expression in DMN produced an error
    ///   U008 — DmnNoMatch      No DMN rule matched the input
    ///   U009 — DmnMultiple     Multiple DMN rules matched (UNIQUE hit policy)
    ///
    ///   S001 — Database        Unexpected database-level failure
    ///   S002 — Internal        Unexpected internal server error
    ///   S003 — Expression      Runtime FEEL expression evaluation failure
    fn code(&self) -> &'static str {
        match self {
            EngineError::Validation(_) => "U001",
            EngineError::NotFound(_) | EngineError::DmnNotFound(_) => "U002",
            EngineError::Conflict(_) => "U003",
            EngineError::Parse(_) => "U004",
            EngineError::UnsupportedElement(_) => "U005",
            EngineError::DmnParse(_) => "U006",
            EngineError::DmnFeel(_) => "U007",
            EngineError::DmnNoMatch => "U008",
            EngineError::DmnMultipleMatches => "U009",
            EngineError::Database(_) => "S001",
            EngineError::Internal(_) => "S002",
            EngineError::Expression(_) => "S003",
        }
    }

    /// HTTP status mapped from variant.
    fn status(&self) -> StatusCode {
        match self {
            EngineError::NotFound(_) | EngineError::DmnNotFound(_) => StatusCode::NOT_FOUND,
            EngineError::Validation(_)
            | EngineError::Parse(_)
            | EngineError::UnsupportedElement(_)
            | EngineError::DmnParse(_)
            | EngineError::DmnFeel(_) => StatusCode::BAD_REQUEST,
            EngineError::Conflict(_) => StatusCode::CONFLICT,
            EngineError::DmnNoMatch | EngineError::DmnMultipleMatches => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
            EngineError::Database(_) | EngineError::Internal(_) | EngineError::Expression(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }

    /// Client-facing message. U-codes surface the variant's runtime detail
    /// (already actionable). S-codes return the generic display from TOML so
    /// internal details never reach the client.
    fn client_message(&self) -> String {
        match self {
            EngineError::NotFound(msg)
            | EngineError::Validation(msg)
            | EngineError::Conflict(msg)
            | EngineError::Parse(msg)
            | EngineError::DmnParse(msg)
            | EngineError::DmnFeel(msg)
            | EngineError::DmnNotFound(msg) => msg.clone(),
            EngineError::UnsupportedElement(el) => format!("Unsupported BPMN element: {el}"),
            EngineError::DmnNoMatch => "No DMN rule matched the input".to_string(),
            EngineError::DmnMultipleMatches => {
                "Multiple DMN rules matched (UNIQUE hit policy violated)".to_string()
            }
            // S-codes: use the generic display from the registry.
            EngineError::Database(_) | EngineError::Internal(_) | EngineError::Expression(_) => {
                codes()
                    .get(self.code())
                    .map(|e| e.display.clone())
                    .unwrap_or_else(|| "An internal server error occurred.".to_string())
            }
        }
    }

    /// Optional user-action hint from the TOML registry (sent to client).
    fn client_action(&self) -> Option<String> {
        codes().get(self.code()).and_then(|e| e.action.clone())
    }

    /// Emit a single structured log line for this error at `error` level with
    /// every available field (variant code, HTTP status, formatted detail, and
    /// source chain when present). Both 5xx and 4xx are logged at `error` per
    /// the project's error-handling policy.
    fn log(&self) {
        let status = self.status();
        let code = self.code();
        let detail = self.to_string();
        let source = std::error::Error::source(self).map(|s| s.to_string());
        let debug_hint = codes()
            .get(code)
            .and_then(|e| e.debug.as_deref())
            .unwrap_or("");

        tracing::error!(
            error.code = code,
            error.status = status.as_u16(),
            error.detail = %detail,
            error.source = source.as_deref().unwrap_or(""),
            error.debug = debug_hint,
            "request failed"
        );
    }
}

impl IntoResponse for EngineError {
    fn into_response(self) -> Response {
        self.log();
        let status = self.status();
        let action = self.client_action();
        let code = self.code();
        let message = self.client_message();

        let mut body = json!({
            "code": code,
            "message": message,
        });

        if let Some(act) = action {
            body["action"] = serde_json::Value::String(act);
        }

        (status, Json(body)).into_response()
    }
}

/// Convenience Result type using EngineError
pub type Result<T> = std::result::Result<T, EngineError>;

/// Map PostgreSQL constraint violations and well-known sqlx errors into the
/// appropriate user-facing EngineError variant. Anything we cannot classify
/// stays as Database (which logs and returns 500).
impl From<sqlx::Error> for EngineError {
    fn from(err: sqlx::Error) -> Self {
        if let sqlx::Error::Database(db_err) = &err {
            if let Some(code) = db_err.code() {
                let constraint = db_err.constraint().unwrap_or("");
                let detail = db_err.message().to_string();
                return match code.as_ref() {
                    // unique_violation
                    "23505" => EngineError::Conflict(if constraint.is_empty() {
                        format!("Resource already exists: {detail}")
                    } else {
                        format!("Resource already exists (constraint '{constraint}')")
                    }),
                    // foreign_key_violation
                    "23503" => EngineError::Validation(if constraint.is_empty() {
                        format!("Referenced resource does not exist: {detail}")
                    } else {
                        format!("Referenced resource does not exist (constraint '{constraint}')")
                    }),
                    // not_null_violation
                    "23502" => {
                        EngineError::Validation(format!("Required field is missing: {detail}"))
                    }
                    // check_violation
                    "23514" => EngineError::Validation(if constraint.is_empty() {
                        format!("Value failed validation: {detail}")
                    } else {
                        format!("Value failed validation (constraint '{constraint}')")
                    }),
                    // exclusion_violation
                    "23P01" => EngineError::Conflict("Exclusion constraint violated".to_string()),
                    // serialization_failure / deadlock_detected — transient
                    "40001" | "40P01" => EngineError::Conflict(
                        "The request conflicted with another concurrent update. Please retry."
                            .to_string(),
                    ),
                    _ => EngineError::Database(err),
                };
            }
        }
        EngineError::Database(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn not_found_returns_404() {
        let err = EngineError::NotFound("Process not found".to_string());
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn validation_returns_400() {
        let err = EngineError::Validation("Invalid input".to_string());
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn internal_returns_500() {
        let err = EngineError::Internal("Something broke".to_string());
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn conflict_returns_409() {
        let err = EngineError::Conflict("Slug taken".to_string());
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn dmn_no_match_returns_422() {
        let err = EngineError::DmnNoMatch;
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[test]
    fn error_codes_registry_is_complete() {
        assert_error_codes_complete();
    }
}
