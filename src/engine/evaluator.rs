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

#[cfg(test)]
mod tests {
    use super::evaluate_condition;
    use serde_json::{json, Value};
    use std::collections::HashMap;

    fn vars(pairs: &[(&str, Value)]) -> HashMap<String, Value> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
    }

    #[test]
    fn integer_gt_true() {
        assert!(evaluate_condition("x > 5", &vars(&[("x", json!(10))])).unwrap());
    }

    #[test]
    fn integer_gt_false() {
        assert!(!evaluate_condition("x > 5", &vars(&[("x", json!(3))])).unwrap());
    }

    #[test]
    fn integer_eq() {
        assert!(evaluate_condition("x == 42", &vars(&[("x", json!(42))])).unwrap());
    }

    #[test]
    fn float_comparison() {
        assert!(evaluate_condition("score >= 7.5", &vars(&[("score", json!(8.0))])).unwrap());
    }

    #[test]
    fn bool_variable_true() {
        assert!(evaluate_condition("flag", &vars(&[("flag", json!(true))])).unwrap());
    }

    #[test]
    fn bool_variable_false() {
        assert!(!evaluate_condition("flag", &vars(&[("flag", json!(false))])).unwrap());
    }

    #[test]
    fn string_equality() {
        assert!(evaluate_condition(r#"name == "alice""#, &vars(&[("name", json!("alice"))])).unwrap());
    }

    #[test]
    fn string_inequality() {
        assert!(evaluate_condition(r#"name != "bob""#, &vars(&[("name", json!("alice"))])).unwrap());
    }

    #[test]
    fn compound_and_condition() {
        let v = vars(&[("x", json!(10)), ("y", json!(20))]);
        assert!(evaluate_condition("x < y && x > 5", &v).unwrap());
    }

    #[test]
    fn no_variables_literal_true() {
        assert!(evaluate_condition("true", &HashMap::new()).unwrap());
    }

    #[test]
    fn no_variables_literal_false() {
        assert!(!evaluate_condition("false", &HashMap::new()).unwrap());
    }

    #[test]
    fn invalid_expression_returns_err() {
        assert!(evaluate_condition("x @@@", &vars(&[("x", json!(1))])).is_err());
    }

    #[test]
    fn non_bool_expression_returns_err() {
        assert!(evaluate_condition("1 + 1", &HashMap::new()).is_err());
    }

    #[test]
    fn null_and_array_variables_ignored() {
        // null/array JSON values are not pushed into scope, so the expression
        // must not panic — it will produce an error if the variable is referenced
        let v = vars(&[("x", json!(null)), ("arr", json!([1, 2]))]);
        // fall back to literal true without referencing the null/array vars
        assert!(evaluate_condition("true", &v).unwrap());
    }
}
