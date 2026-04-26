use std::collections::HashMap;

use rhai::{Engine as RhaiEngine, Scope};
use serde_json::Value as JsonValue;

use crate::error::{EngineError, Result};

pub fn evaluate_condition(expr: &str, variables: &HashMap<String, JsonValue>) -> Result<bool> {
    let engine = RhaiEngine::new();
    let mut scope = Scope::new();

    for (name, value) in variables {
        match value {
            JsonValue::Bool(b) => {
                scope.push(name.as_str(), *b);
            }
            JsonValue::Number(n) => {
                if let Some(i) = n.as_i64() {
                    scope.push(name.as_str(), i);
                } else if let Some(f) = n.as_f64() {
                    scope.push(name.as_str(), f);
                }
            }
            JsonValue::String(s) => {
                scope.push(name.as_str(), s.clone());
            }
            _ => {}
        }
    }

    engine
        .eval_expression_with_scope::<bool>(&mut scope, expr)
        .map_err(|e| EngineError::Expression(format!("Condition '{expr}': {e}")))
}
