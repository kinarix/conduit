/// Custom extractors that convert Axum rejection types into `EngineError::Validation`
/// so all error responses share the same `{"code", "message", "action"}` wire format.
use axum::{
    extract::{
        rejection::{JsonRejection, PathRejection, QueryRejection},
        FromRequest, FromRequestParts, OptionalFromRequest, Request,
    },
    http::{header, request::Parts},
    response::{IntoResponse, Response},
};
use serde::{de::DeserializeOwned, Serialize};
use std::sync::Arc;

use crate::auth::{self, Principal, PrincipalKind};
use crate::db;
use crate::error::EngineError;
use crate::state::AppState;

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

// ─── Principal ───────────────────────────────────────────────────────────────
//
// Pulled by handlers as `principal: Principal`. Reads `Authorization: Bearer
// <token>`, dispatches to API-key or JWT verification, and resolves the
// caller's user + org. Any failure surfaces as `U401` — the body never tells
// the client *which* check failed (missing header vs bad signature vs revoked
// key vs deleted user). The token itself is never logged.

impl FromRequestParts<Arc<AppState>> for Principal {
    type Rejection = EngineError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let token = bearer_token(parts).ok_or(EngineError::Unauthenticated)?;

        // API-key path: cheap prefix lookup, then argon2 verify of the full
        // plaintext against the stored hash.
        if let Some(prefix) = auth::api_key::extract_prefix(token) {
            let row = db::api_keys::lookup_by_prefix(&state.pool, &prefix)
                .await?
                .ok_or(EngineError::Unauthenticated)?;
            if !auth::api_key::verify(token, &row.key_hash) {
                return Err(EngineError::Unauthenticated);
            }
            // Best-effort: never block the request on this update.
            let pool = state.pool.clone();
            let key_id = row.id;
            tokio::spawn(async move {
                if let Err(e) = db::api_keys::touch_last_used(&pool, key_id).await {
                    tracing::debug!(error = %e, "failed to update api_key.last_used_at");
                }
            });
            return Ok(Principal {
                user_id: row.user_id,
                org_id: row.org_id,
                email: row.email,
                kind: PrincipalKind::ApiKey,
            });
        }

        // JWT path. Decode validates signature, issuer, and expiry; we then
        // confirm the subject still exists so deleted users can't keep using
        // tokens until expiry.
        let claims = auth::jwt::decode_token(token, &state.auth.jwt_keys, &state.auth.jwt_issuer)
            .ok_or(EngineError::Unauthenticated)?;
        let user = db::users::find_by_id(&state.pool, claims.sub)
            .await?
            .ok_or(EngineError::Unauthenticated)?;
        // Defence in depth: a token whose `org` claim no longer matches the
        // user's row is treated as invalid (user moved orgs, etc.).
        if user.org_id != claims.org {
            return Err(EngineError::Unauthenticated);
        }
        Ok(Principal {
            user_id: user.id,
            org_id: user.org_id,
            email: user.email,
            kind: PrincipalKind::Jwt,
        })
    }
}

fn bearer_token(parts: &Parts) -> Option<&str> {
    let header = parts.headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    header.strip_prefix("Bearer ").map(str::trim)
}
