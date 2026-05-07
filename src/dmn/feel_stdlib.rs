use std::collections::HashMap;

use serde_json::Value as JsonValue;

use crate::engine::evaluate_expression;
use crate::error::Result;

/// Evaluate a full FEEL expression against a variable context.
///
/// Delegates to `dsntk-feel-evaluator` (the same evaluator used for gateway
/// conditions). Covers the complete FEEL standard library — numeric, string,
/// list, date/time, and context functions — as well as FEEL literals, `if`
/// expressions, `for` expressions, and `instance of`.
///
/// Used for DMN output cell expressions, PRIORITY/OUTPUT ORDER value lists,
/// and DRD chained-decision inputs.
pub fn eval_feel(expr: &str, ctx: &HashMap<String, JsonValue>) -> Result<JsonValue> {
    evaluate_expression(expr, ctx)
}
