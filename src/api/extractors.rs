/// Custom extractors that convert Axum rejection types into `EngineError::Validation`
/// so all error responses share the same `{"code", "message", "action"}` wire format.
use axum::{
    extract::{
        rejection::{JsonRejection, PathRejection, QueryRejection},
        FromRequest, FromRequestParts, OptionalFromRequest, Request,
    },
    http::{header, request::Parts},
    response::{IntoResponse, Response},
    RequestPartsExt,
};
use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

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
// caller's user. When the request path contains `{org_id}` (under the
// `/api/v1/orgs/{org_id}/...` nested router) the extractor also:
//   1. Parses the `org_id` from the path
//   2. Confirms membership (or bypasses for global admins)
//   3. Loads org-scoped permissions and merges with the user's global ones
// On global routes (no `{org_id}` in path) only global permissions load.
//
// Any auth failure surfaces as `U401` — the body never tells the client
// *which* check failed. The token itself is never logged.

impl FromRequestParts<Arc<AppState>> for Principal {
    type Rejection = EngineError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let token = bearer_token(parts).ok_or(EngineError::Unauthenticated)?;

        // Resolve the caller's user + kind from either an API key or a JWT.
        let (user_id, email, kind) = if let Some(prefix) = auth::api_key::extract_prefix(token) {
            let row = db::api_keys::lookup_by_prefix(&state.pool, &prefix)
                .await?
                .ok_or(EngineError::Unauthenticated)?;
            if !auth::api_key::verify(token, &row.key_hash) {
                return Err(EngineError::Unauthenticated);
            }
            let pool = state.pool.clone();
            let key_id = row.id;
            tokio::spawn(async move {
                if let Err(e) = db::api_keys::touch_last_used(&pool, key_id).await {
                    tracing::debug!(error = %e, "failed to update api_key.last_used_at");
                }
            });
            (row.user_id, row.email, PrincipalKind::ApiKey)
        } else {
            let claims =
                auth::jwt::decode_token(token, &state.auth.jwt_keys, &state.auth.jwt_issuer)
                    .ok_or(EngineError::Unauthenticated)?;
            let user = db::users::find_by_id(&state.pool, claims.sub)
                .await?
                .ok_or(EngineError::Unauthenticated)?;
            (user.id, user.email, PrincipalKind::Jwt)
        };

        // Detect optional `{org_id}` in the matched path.
        let current_org_id = extract_org_id_from_path(parts).await?;

        let is_global_admin = db::role_assignments::is_global_admin(&state.pool, user_id).await?;

        // Membership check (skipped for global admins).
        if let Some(org_id) = current_org_id {
            if !is_global_admin {
                let member = db::org_members::exists(&state.pool, user_id, org_id).await?;
                if !member {
                    return Err(EngineError::Forbidden(format!(
                        "not a member of org {org_id}"
                    )));
                }
            }
        }

        let permissions =
            db::role_assignments::load_all_permissions(&state.pool, user_id, current_org_id)
                .await?;

        // Pg-scoped grants only matter on org-scoped routes for non-global
        // admins — a global admin already has every perm cascading from the
        // global grant, and global routes don't act on a single pg.
        let pg_permissions = match current_org_id {
            Some(org_id) if !is_global_admin => {
                db::role_assignments::load_pg_permissions_for_user_in_org(
                    &state.pool,
                    user_id,
                    org_id,
                )
                .await?
            }
            _ => HashMap::new(),
        };

        Ok(Principal {
            user_id,
            email,
            kind,
            current_org_id,
            org_id: current_org_id.unwrap_or(Uuid::nil()),
            is_global_admin,
            permissions,
            pg_permissions,
        })
    }
}

/// Pull `{org_id}` from the matched path, if the route declares one.
/// Returns `None` if the path doesn't have an `org_id` placeholder.
/// Returns `Err(Validation)` if the placeholder exists but its value
/// isn't a valid UUID.
async fn extract_org_id_from_path(parts: &mut Parts) -> Result<Option<Uuid>, EngineError> {
    // axum::extract::RawPathParams gives us a cloneable view that we can
    // read without consuming. (Plain `Path<HashMap<_, _>>` is also
    // multi-use because it caches inside extensions.)
    let params: axum::extract::RawPathParams = parts
        .extract()
        .await
        .map_err(|_| EngineError::Validation("invalid path parameters".to_string()))?;

    let mut map: HashMap<&str, &str> = HashMap::new();
    for (k, v) in params.iter() {
        map.insert(k, v);
    }

    let Some(raw) = map.get("org_id") else {
        return Ok(None);
    };
    let parsed = Uuid::parse_str(raw)
        .map_err(|_| EngineError::Validation(format!("invalid org_id `{raw}`")))?;
    Ok(Some(parsed))
}

fn bearer_token(parts: &Parts) -> Option<&str> {
    let header = parts.headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    header.strip_prefix("Bearer ").map(str::trim)
}
