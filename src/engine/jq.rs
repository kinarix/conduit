//! Phase 16 — jq filter compilation + execution for HTTP connector transforms.
//!
//! Both the request and response of the HTTP service task are shaped by user-
//! supplied jq filters embedded in the BPMN. Filters are compiled once on first
//! use and cached, then evaluated against `serde_json::Value` input.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use jaq_interpret::{Ctx, Filter, FilterT, ParseCtx, RcIter, Val};
use serde_json::Value as JsonValue;

use crate::error::{EngineError, Result};

/// Process-wide cache of compiled jq filters keyed by source string. Filters
/// are immutable once compiled, so cloning the inner `Filter` is cheap (Arc).
#[derive(Default, Clone)]
pub struct JqCache {
    inner: Arc<RwLock<HashMap<String, Filter>>>,
}

impl JqCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Compile and execute a filter. The first execution against a given source
    /// pays the compile cost; subsequent calls are pure interpretation.
    pub fn run(&self, source: &str, input: JsonValue) -> Result<JsonValue> {
        let filter = self.compile(source)?;
        let inputs = RcIter::new(core::iter::empty());
        let mut out = filter.run((Ctx::new([], &inputs), Val::from(input)));
        let first = out
            .next()
            .ok_or_else(|| EngineError::Internal("jq filter produced no output".into()))?;
        let val =
            first.map_err(|e| EngineError::Validation(format!("jq filter runtime error: {e}")))?;
        Ok(val.into())
    }

    /// Compile a filter source without running it. Used by the parser at deploy
    /// time to fail fast on syntactically invalid filters.
    pub fn compile(&self, source: &str) -> Result<Filter> {
        if let Some(f) = self
            .inner
            .read()
            .map_err(|_| EngineError::Internal("jq cache lock poisoned".into()))?
            .get(source)
        {
            return Ok(f.clone());
        }
        let compiled = compile_filter(source)?;
        self.inner
            .write()
            .map_err(|_| EngineError::Internal("jq cache lock poisoned".into()))?
            .insert(source.to_string(), compiled.clone());
        Ok(compiled)
    }
}

/// Standalone compile path used by both [`JqCache::compile`] and the parser's
/// validation pass.
pub fn compile_filter(source: &str) -> Result<Filter> {
    let (parsed, errs) = jaq_parse::parse(source, jaq_parse::main());
    if !errs.is_empty() {
        let msg = errs
            .into_iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        return Err(EngineError::Parse(format!("jq filter parse error: {msg}")));
    }
    let parsed =
        parsed.ok_or_else(|| EngineError::Parse("jq filter parser returned no AST".into()))?;

    let mut ctx = ParseCtx::new(Vec::new());
    ctx.insert_natives(jaq_core::core());
    ctx.insert_defs(jaq_std::std());
    let filter = ctx.compile(parsed);
    if !ctx.errs.is_empty() {
        // jaq_interpret::hir::Error does not implement Debug; the Display impl
        // is the only public surface for the message.
        let msg = ctx
            .errs
            .into_iter()
            .map(|(e, _)| e.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        return Err(EngineError::Parse(format!(
            "jq filter compile error: {msg}"
        )));
    }
    Ok(filter)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn identity_filter_returns_input() {
        let cache = JqCache::new();
        let out = cache.run(".", json!({"a": 1})).unwrap();
        assert_eq!(out, json!({"a": 1}));
    }

    #[test]
    fn shape_request_envelope() {
        let cache = JqCache::new();
        let input = json!({
            "instance_id": "i-123",
            "execution_id": "e-456",
            "vars": { "amount": 1000, "customer_id": "c-789" }
        });
        let filter = r#"{
            body: { amount: .vars.amount, currency: "usd" },
            query: { idempotency_key: .instance_id },
            path:  { customer_id: .vars.customer_id }
        }"#;
        let out = cache.run(filter, input).unwrap();
        assert_eq!(out["body"]["amount"], 1000);
        assert_eq!(out["body"]["currency"], "usd");
        assert_eq!(out["query"]["idempotency_key"], "i-123");
        assert_eq!(out["path"]["customer_id"], "c-789");
    }

    #[test]
    fn extract_response_into_flat_vars() {
        let cache = JqCache::new();
        let input = json!({
            "status": 200,
            "headers": { "x-rate-limit-remaining": "59" },
            "body": { "id": "ch_abc", "status": "succeeded" }
        });
        let filter = r#"{
            charge_id:     .body.id,
            charge_status: .body.status,
            http_status:   .status,
            rate_limit:    (.headers["x-rate-limit-remaining"] | tonumber? // null)
        }"#;
        let out = cache.run(filter, input).unwrap();
        assert_eq!(out["charge_id"], "ch_abc");
        assert_eq!(out["charge_status"], "succeeded");
        assert_eq!(out["http_status"], 200);
        assert_eq!(out["rate_limit"], 59);
    }

    #[test]
    fn invalid_syntax_fails_at_compile() {
        let result = compile_filter("this is not jq syntax {{{ <-");
        assert!(result.is_err());
    }

    #[test]
    fn runtime_error_surfaces_as_validation() {
        let cache = JqCache::new();
        // `tonumber` on a non-numeric string with no `?` fallback raises.
        let result = cache.run(".body | tonumber", json!({"body": "not-a-number"}));
        assert!(result.is_err());
    }

    #[test]
    fn cache_returns_same_filter_twice() {
        let cache = JqCache::new();
        let _ = cache.run(".", json!(1)).unwrap();
        let _ = cache.run(".", json!(2)).unwrap();
        // Cache has one entry, not two.
        assert_eq!(cache.inner.read().unwrap().len(), 1);
    }
}
