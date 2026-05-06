//! Boolean expression evaluator for sequence-flow conditions on
//! ExclusiveGateway / InclusiveGateway nodes.
//!
//! Language: FEEL (Friendly Enough Expression Language) — the boolean
//! expression language defined by the DMN 1.5 specification, evaluated via
//! `dsntk-feel-evaluator`. This unifies gateway conditions with DMN cells
//! and aligns Conduit with the BPMN/DMN family that Camunda 8 / Zeebe also use.
//!
//! Process variables (loaded as JSON from the `variables` table) are converted
//! into FEEL values; the expression is parsed and evaluated against a scope
//! containing those variables. A null result (missing variable, indeterminate
//! comparison) surfaces as `Err` so the gateway marks the instance as error
//! rather than silently skipping the path. Non-boolean results are also errors.

use std::collections::HashMap;
use std::str::FromStr;

use dsntk_feel::context::FeelContext;
use dsntk_feel::values::Value as FeelValue;
use dsntk_feel::{FeelNumber, FeelScope, Name};
use dsntk_feel_evaluator::evaluate as feel_evaluate;
use dsntk_feel_parser::{parse_expression, parse_textual_expression};
use serde_json::Value as JsonValue;

use crate::error::{EngineError, Result};

pub fn evaluate_condition(expr: &str, variables: &HashMap<String, JsonValue>) -> Result<bool> {
    let mut root_ctx = FeelContext::default();
    for (name, value) in variables {
        root_ctx.set_entry(&Name::from(name.as_str()), json_to_feel(value));
    }

    let scope = FeelScope::default();
    scope.push(root_ctx);

    let ast = parse_textual_expression(&scope, expr, false)
        .map_err(|e| EngineError::Expression(format!("FEEL parse '{expr}': {e:?}")))?;

    match feel_evaluate(&scope, &ast) {
        FeelValue::Boolean(b) => Ok(b),
        FeelValue::Null(_) => Err(EngineError::Expression(format!(
            "FEEL '{expr}' evaluated to null — variable may be undefined or comparison is indeterminate"
        ))),
        other => Err(EngineError::Expression(format!(
            "FEEL '{expr}' did not produce a boolean (got {other:?})"
        ))),
    }
}

pub fn evaluate_expression(
    expr: &str,
    variables: &HashMap<String, JsonValue>,
) -> Result<JsonValue> {
    let mut root_ctx = FeelContext::default();
    for (name, value) in variables {
        root_ctx.set_entry(&Name::from(name.as_str()), json_to_feel(value));
    }

    let scope = FeelScope::default();
    scope.push(root_ctx);

    let ast = parse_expression(&scope, expr, false)
        .map_err(|e| EngineError::Expression(format!("FEEL parse '{expr}': {e:?}")))?;

    let result = feel_evaluate(&scope, &ast);
    Ok(feel_to_json(result))
}

fn feel_to_json(value: FeelValue) -> JsonValue {
    match value {
        FeelValue::Boolean(b) => JsonValue::Bool(b),
        FeelValue::String(s) => JsonValue::String(s),
        FeelValue::Number(n) => {
            let s = n.to_string();
            if let Ok(i) = s.parse::<i64>() {
                JsonValue::Number(i.into())
            } else if let Ok(f) = s.parse::<f64>() {
                serde_json::Number::from_f64(f)
                    .map(JsonValue::Number)
                    .unwrap_or(JsonValue::Null)
            } else {
                JsonValue::Null
            }
        }
        FeelValue::List(items) => {
            JsonValue::Array(items.iter().map(|v| feel_to_json(v.clone())).collect())
        }
        FeelValue::Context(ctx) => {
            let mut map = serde_json::Map::new();
            for (name, value) in ctx.iter() {
                map.insert(name.to_string(), feel_to_json(value.clone()));
            }
            JsonValue::Object(map)
        }
        _ => JsonValue::Null,
    }
}

fn json_to_feel(value: &JsonValue) -> FeelValue {
    match value {
        JsonValue::Null => FeelValue::Null(None),
        JsonValue::Bool(b) => FeelValue::Boolean(*b),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                FeelValue::Number(FeelNumber::from(i))
            } else if let Some(f) = n.as_f64() {
                // FeelNumber has no From<f64>; round-trip via string.
                match FeelNumber::from_str(&f.to_string()) {
                    Ok(fn_) => FeelValue::Number(fn_),
                    Err(_) => FeelValue::Null(None),
                }
            } else {
                FeelValue::Null(None)
            }
        }
        JsonValue::String(s) => FeelValue::String(s.clone()),
        JsonValue::Array(items) => {
            let values: Vec<FeelValue> = items.iter().map(json_to_feel).collect();
            FeelValue::List(values)
        }
        JsonValue::Object(map) => {
            let mut ctx = FeelContext::default();
            for (k, v) in map {
                ctx.set_entry(&Name::from(k.as_str()), json_to_feel(v));
            }
            FeelValue::Context(ctx)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::evaluate_condition;
    use serde_json::{json, Value};
    use std::collections::HashMap;

    fn vars(pairs: &[(&str, Value)]) -> HashMap<String, Value> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
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
    fn integer_eq_uses_single_equals() {
        // FEEL uses `=` for equality, not `==`.
        assert!(evaluate_condition("x = 42", &vars(&[("x", json!(42))])).unwrap());
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
        assert!(
            evaluate_condition(r#"name = "alice""#, &vars(&[("name", json!("alice"))])).unwrap()
        );
    }

    #[test]
    fn string_inequality() {
        assert!(
            evaluate_condition(r#"name != "bob""#, &vars(&[("name", json!("alice"))])).unwrap()
        );
    }

    #[test]
    fn compound_and_condition() {
        let v = vars(&[("x", json!(10)), ("y", json!(20))]);
        assert!(evaluate_condition("x < y and x > 5", &v).unwrap());
    }

    #[test]
    fn compound_or_condition() {
        let v = vars(&[("status", json!("pending"))]);
        assert!(evaluate_condition(r#"status = "approved" or status = "pending""#, &v).unwrap());
    }

    #[test]
    fn negation_with_not() {
        assert!(evaluate_condition("not(flag)", &vars(&[("flag", json!(false))])).unwrap());
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
    fn undefined_variable_errors_not_silent_false() {
        // Undefined names produce FEEL Null, surfaced as Err — never silent false.
        let v = vars(&[("y", json!(1))]);
        assert!(evaluate_condition("x > 5", &v).is_err());
    }

    #[test]
    fn array_count_via_builtin() {
        // FEEL: count(list), not list.len().
        let v = vars(&[("items", json!([1, 2, 3]))]);
        assert!(evaluate_condition("count(items) > 0", &v).unwrap());
        assert!(evaluate_condition("count(items) = 3", &v).unwrap());
    }

    #[test]
    fn empty_array_count() {
        let v = vars(&[("items", json!([]))]);
        assert!(!evaluate_condition("count(items) > 0", &v).unwrap());
    }

    #[test]
    fn object_field_access() {
        let v = vars(&[("user", json!({"tier": "gold", "age": 30}))]);
        assert!(evaluate_condition(r#"user.tier = "gold""#, &v).unwrap());
        assert!(evaluate_condition("user.age >= 18", &v).unwrap());
    }

    #[test]
    fn nested_object_access() {
        let v = vars(&[("ctx", json!({"user": {"role": "admin"}}))]);
        assert!(evaluate_condition(r#"ctx.user.role = "admin""#, &v).unwrap());
    }

    #[test]
    fn array_indexing_one_based() {
        // FEEL list indices start at 1.
        let v = vars(&[("scores", json!([85, 92, 71]))]);
        assert!(evaluate_condition("scores[1] = 85", &v).unwrap());
        assert!(evaluate_condition("scores[2] > 90", &v).unwrap());
    }

    #[test]
    fn evaluate_expression_context_literal() {
        use super::evaluate_expression;
        let v = vars(&[("amount", json!(2000))]);
        let result = evaluate_expression(
            r#"{ fee: amount * 0.05, tier: if amount > 1000 then "premium" else "standard" }"#,
            &v,
        )
        .unwrap();
        eprintln!("context literal result: {:?}", result);
        assert!(result.is_object(), "expected Object, got: {:?}", result);
        let obj = result.as_object().unwrap();
        assert_eq!(obj["fee"], json!(100));
        assert_eq!(obj["tier"], json!("premium"));
    }

    #[test]
    fn evaluate_expression_scalar_addition() {
        use super::evaluate_expression;
        let v = vars(&[("amount", json!(500)), ("shipping", json!(25))]);
        let result = evaluate_expression("amount + shipping", &v).unwrap();
        eprintln!("scalar addition result: {:?}", result);
        assert_eq!(result, json!(525));
    }
}
