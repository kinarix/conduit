/// Custom extractors that convert Axum rejection types into `EngineError::Validation`
/// so all error responses share the same `{"code", "message", "action"}` wire format.
use axum::{
    extract::{
        rejection::{JsonRejection, PathRejection, QueryRejection},
        FromRequest, FromRequestParts, OptionalFromRequest, Request,
    },
    http::request::Parts,
    response::{IntoResponse, Response},
};
use serde::{de::DeserializeOwned, Serialize};

use crate::error::EngineError;

// ─── Json ────────────────────────────────────────────────────────────────────

pub struct Json<T>(pub T);

impl<T, S> FromRequest<S> for Json<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = EngineError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match <axum::Json<T> as FromRequest<S>>::from_request(req, state).await {
            Ok(axum::Json(val)) => Ok(Json(val)),
            Err(rej) => Err(validation_from_json(rej)),
        }
    }
}

impl<T: Serialize> IntoResponse for Json<T> {
    fn into_response(self) -> Response {
        axum::Json(self.0).into_response()
    }
}

impl<T, S> OptionalFromRequest<S> for Json<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = EngineError;

    async fn from_request(req: Request, state: &S) -> Result<Option<Self>, Self::Rejection> {
        match <axum::Json<T> as OptionalFromRequest<S>>::from_request(req, state).await {
            Ok(Some(axum::Json(val))) => Ok(Some(Json(val))),
            Ok(None) => Ok(None),
            Err(rej) => Err(validation_from_json(rej)),
        }
    }
}

fn validation_from_json(rej: JsonRejection) -> EngineError {
    EngineError::Validation(match &rej {
        JsonRejection::JsonDataError(_) => {
            format!(
                "Request body has invalid field types or values: {}",
                rej.body_text()
            )
        }
        JsonRejection::JsonSyntaxError(_) => {
            format!("Request body is not valid JSON: {}", rej.body_text())
        }
        JsonRejection::MissingJsonContentType(_) => {
            "Content-Type must be application/json".to_string()
        }
        JsonRejection::BytesRejection(_) => {
            format!("Failed to read request body: {}", rej.body_text())
        }
        _ => format!("Invalid request body: {}", rej.body_text()),
    })
}

// ─── Path ────────────────────────────────────────────────────────────────────

pub struct Path<T>(pub T);

impl<T, S> FromRequestParts<S> for Path<T>
where
    T: DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = EngineError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match axum::extract::Path::<T>::from_request_parts(parts, state).await {
            Ok(axum::extract::Path(val)) => Ok(Path(val)),
            Err(rej) => Err(validation_from_path(rej)),
        }
    }
}

fn validation_from_path(rej: PathRejection) -> EngineError {
    EngineError::Validation(format!("Invalid path parameter: {}", rej.body_text()))
}

// ─── Query ───────────────────────────────────────────────────────────────────

pub struct Query<T>(pub T);

impl<T, S> FromRequestParts<S> for Query<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = EngineError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match axum::extract::Query::<T>::from_request_parts(parts, state).await {
            Ok(axum::extract::Query(val)) => Ok(Query(val)),
            Err(rej) => Err(validation_from_query(rej)),
        }
    }
}

fn validation_from_query(rej: QueryRejection) -> EngineError {
    EngineError::Validation(format!("Invalid query parameter: {}", rej.body_text()))
}
