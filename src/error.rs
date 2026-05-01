use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

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

/// Generic message returned for any 5xx response so we never leak internal
/// details (DB errors, expression panics, etc.) to API callers. Operators see
/// the full error in logs via the `tracing::error!` calls below.
const PUBLIC_500_MESSAGE: &str =
    "An internal server error occurred. Please contact the administrator if the problem persists.";

impl EngineError {
    /// Short, stable label for this variant — used in structured logs and the
    /// JSON `code` field so callers can branch programmatically.
    fn code(&self) -> &'static str {
        match self {
            EngineError::NotFound(_) => "not_found",
            EngineError::Validation(_) => "validation",
            EngineError::Conflict(_) => "conflict",
            EngineError::Database(_) => "database",
            EngineError::Internal(_) => "internal",
            EngineError::Parse(_) => "parse",
            EngineError::UnsupportedElement(_) => "unsupported_element",
            EngineError::Expression(_) => "expression",
            EngineError::DmnParse(_) => "dmn_parse",
            EngineError::DmnFeel(_) => "dmn_feel",
            EngineError::DmnNotFound(_) => "dmn_not_found",
            EngineError::DmnNoMatch => "dmn_no_match",
            EngineError::DmnMultipleMatches => "dmn_multiple_matches",
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

    /// User-facing message. For 5xx variants, returns a generic message so
    /// internal details never leak to clients. For 4xx, returns the variant's
    /// own message which is intended to be actionable.
    fn public_message(&self) -> String {
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
            EngineError::Database(_) | EngineError::Internal(_) | EngineError::Expression(_) => {
                PUBLIC_500_MESSAGE.to_string()
            }
        }
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

        tracing::error!(
            error.code = code,
            error.status = status.as_u16(),
            error.detail = %detail,
            error.source = source.as_deref().unwrap_or(""),
            "request failed"
        );
    }
}

impl IntoResponse for EngineError {
    fn into_response(self) -> Response {
        self.log();
        let status = self.status();
        let body = json!({
            "error": self.public_message(),
            "code": self.code(),
        });
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
}
