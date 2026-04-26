use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

/// Unified error type for Takt.
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
    Database(#[from] sqlx::Error),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Unsupported BPMN element: {0}")]
    UnsupportedElement(String),
}

impl IntoResponse for EngineError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            EngineError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            EngineError::Validation(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            EngineError::Conflict(msg) => (StatusCode::CONFLICT, msg.clone()),
            EngineError::Database(e) => {
                // Never expose internal DB error details to callers
                tracing::error!(error = %e, "Database error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "A database error occurred".to_string(),
                )
            }
            EngineError::Internal(msg) => {
                tracing::error!(error = %msg, "Internal engine error");
                (StatusCode::INTERNAL_SERVER_ERROR, msg.clone())
            }
            EngineError::Parse(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            EngineError::UnsupportedElement(el) => (
                StatusCode::BAD_REQUEST,
                format!("Unsupported BPMN element: {el}"),
            ),
        };

        (status, Json(json!({ "error": message }))).into_response()
    }
}

/// Convenience Result type using EngineError
pub type Result<T> = std::result::Result<T, EngineError>;

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
}
