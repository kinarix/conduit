//! Shared pagination helper used by every list endpoint.
//!
//! - Caller passes `limit` and `offset` query parameters (both optional)
//! - `limit` clamps to [1, 500] with default 100; `offset` clamps to >= 0 with default 0
//! - The response is the JSON body plus an `X-Total-Count` header carrying the
//!   unfiltered row count so callers can compute page counts without a second call.

use axum::{
    http::HeaderValue,
    response::{IntoResponse, Response},
};
use serde::Serialize;

use super::extractors::Json;

pub const DEFAULT_LIMIT: i64 = 100;
pub const MAX_LIMIT: i64 = 500;

#[derive(Debug, Clone, Copy)]
pub struct Page {
    pub limit: i64,
    pub offset: i64,
}

impl Page {
    pub fn from_query(limit: Option<i64>, offset: Option<i64>) -> Self {
        Self {
            limit: limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT),
            offset: offset.unwrap_or(0).max(0),
        }
    }
}

/// Wrap any serializable body with a `X-Total-Count` header.
pub fn with_total<T: Serialize>(body: T, total: i64) -> Response {
    let mut resp = Json(body).into_response();
    if let Ok(val) = HeaderValue::from_str(&total.to_string()) {
        resp.headers_mut().insert("X-Total-Count", val);
    }
    resp
}
