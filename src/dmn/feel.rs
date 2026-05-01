use crate::error::EngineError;
use serde_json::Value;

/// Evaluate a single DMN input-entry cell against a runtime value.
///
/// Supported syntax (subset of FEEL):
///   `-`                   wildcard — always true
///   `"string"`            string literal equality
///   `42` / `3.14`         number literal equality
///   `true` / `false`      boolean literal equality
///   `>= n`, `> n`, `<= n`, `< n`, `= v`, `!= v`   unary comparisons
///   `[lo..hi]` `(lo..hi)` `[lo..hi)` `(lo..hi]`    numeric ranges
///   `entry,entry,...`     OR list (each entry may be any of the above)
///
/// Null/missing values: a `null` JSON value compared against numeric or
/// string comparators returns `false` (it never errors).
pub fn eval_input_entry(cell: &str, value: &Value) -> Result<bool, EngineError> {
    let cell = cell.trim();

    // Wildcard
    if cell == "-" {
        return Ok(true);
    }

    // Try splitting by top-level commas (OR list).
    // We scan manually so we don't split inside string literals or ranges.
    let parts = split_or_list(cell);
    if parts.len() > 1 {
        for part in &parts {
            if eval_single_entry(part.trim(), value)? {
                return Ok(true);
            }
        }
        return Ok(false);
    }

    eval_single_entry(cell, value)
}

/// Split `cell` by commas that are NOT inside double-quoted strings.
fn split_or_list(cell: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_string = false;

    for ch in cell.chars() {
        match ch {
            '"' => {
                in_string = !in_string;
                current.push(ch);
            }
            ',' if !in_string => {
                parts.push(current.trim().to_string());
                current = String::new();
            }
            _ => current.push(ch),
        }
    }
    if !current.trim().is_empty() {
        parts.push(current.trim().to_string());
    }
    parts
}

fn eval_single_entry(cell: &str, value: &Value) -> Result<bool, EngineError> {
    // Range: [lo..hi], (lo..hi), [lo..hi), (lo..hi]
    if let Some(result) = try_range(cell, value)? {
        return Ok(result);
    }

    // Unary comparisons: >= x, > x, <= x, < x, = x, != x
    if let Some(result) = try_unary(cell, value)? {
        return Ok(result);
    }

    // String literal: "foo"
    if cell.starts_with('"') && cell.ends_with('"') && cell.len() >= 2 {
        let lit = &cell[1..cell.len() - 1];
        return Ok(value.as_str() == Some(lit));
    }

    // Boolean literal
    if cell == "true" {
        return Ok(value.as_bool() == Some(true));
    }
    if cell == "false" {
        return Ok(value.as_bool() == Some(false));
    }

    // Number literal
    if let Ok(n) = cell.parse::<f64>() {
        return Ok(numeric_eq(value, n));
    }

    Err(EngineError::DmnFeel(format!(
        "Unrecognised FEEL cell: {cell:?}"
    )))
}

fn try_range(cell: &str, value: &Value) -> Result<Option<bool>, EngineError> {
    let lower_inclusive = cell.starts_with('[');
    let lower_exclusive = cell.starts_with('(');
    if !lower_inclusive && !lower_exclusive {
        return Ok(None);
    }
    let upper_inclusive = cell.ends_with(']');
    let upper_exclusive = cell.ends_with(')');
    if !upper_inclusive && !upper_exclusive {
        return Ok(None);
    }

    let inner = &cell[1..cell.len() - 1];
    let sep = inner
        .find("..")
        .ok_or_else(|| EngineError::DmnFeel(format!("Malformed range: {cell:?}")))?;

    let lo_str = inner[..sep].trim();
    let hi_str = inner[sep + 2..].trim();

    let lo: f64 = lo_str
        .parse()
        .map_err(|_| EngineError::DmnFeel(format!("Non-numeric range bound: {lo_str:?}")))?;
    let hi: f64 = hi_str
        .parse()
        .map_err(|_| EngineError::DmnFeel(format!("Non-numeric range bound: {hi_str:?}")))?;

    let v = match as_f64(value) {
        Some(n) => n,
        None => return Ok(Some(false)),
    };

    let lo_ok = if lower_inclusive { v >= lo } else { v > lo };
    let hi_ok = if upper_inclusive { v <= hi } else { v < hi };
    Ok(Some(lo_ok && hi_ok))
}

fn try_unary(cell: &str, value: &Value) -> Result<Option<bool>, EngineError> {
    // Two-char operators first
    for op in &[">=", "<=", "!="] {
        if let Some(rest) = cell.strip_prefix(op) {
            let rhs = rest.trim();
            return Ok(Some(compare_op(op, rhs, value)?));
        }
    }
    // Single-char operators
    for op in &[">", "<", "="] {
        if let Some(rest) = cell.strip_prefix(op) {
            let rhs = rest.trim();
            return Ok(Some(compare_op(op, rhs, value)?));
        }
    }
    Ok(None)
}

fn compare_op(op: &str, rhs: &str, value: &Value) -> Result<bool, EngineError> {
    // String RHS
    if rhs.starts_with('"') && rhs.ends_with('"') && rhs.len() >= 2 {
        let lit = &rhs[1..rhs.len() - 1];
        let lhs = match value.as_str() {
            Some(s) => s,
            None => return Ok(false),
        };
        return Ok(match op {
            "=" => lhs == lit,
            "!=" => lhs != lit,
            _ => {
                return Err(EngineError::DmnFeel(format!(
                    "Operator {op} not valid for strings"
                )))
            }
        });
    }

    // Boolean RHS
    if rhs == "true" || rhs == "false" {
        let rhs_bool = rhs == "true";
        let lhs = match value.as_bool() {
            Some(b) => b,
            None => return Ok(false),
        };
        return Ok(match op {
            "=" => lhs == rhs_bool,
            "!=" => lhs != rhs_bool,
            _ => {
                return Err(EngineError::DmnFeel(format!(
                    "Operator {op} not valid for booleans"
                )))
            }
        });
    }

    // Numeric RHS
    let rhs_n: f64 = rhs
        .parse()
        .map_err(|_| EngineError::DmnFeel(format!("Cannot parse RHS as number: {rhs:?}")))?;
    let lhs_n = match as_f64(value) {
        Some(n) => n,
        None => return Ok(false),
    };
    Ok(match op {
        ">=" => lhs_n >= rhs_n,
        ">" => lhs_n > rhs_n,
        "<=" => lhs_n <= rhs_n,
        "<" => lhs_n < rhs_n,
        "=" => (lhs_n - rhs_n).abs() < f64::EPSILON,
        "!=" => (lhs_n - rhs_n).abs() >= f64::EPSILON,
        _ => return Err(EngineError::DmnFeel(format!("Unknown operator: {op}"))),
    })
}

fn as_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        _ => None,
    }
}

fn numeric_eq(v: &Value, n: f64) -> bool {
    match as_f64(v) {
        Some(x) => (x - n).abs() < f64::EPSILON,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::eval_input_entry;
    use serde_json::{json, Value};

    // ── wildcard ──────────────────────────────────────────────────────────────

    #[test]
    fn wildcard_always_true() {
        assert!(eval_input_entry("-", &json!(42)).unwrap());
        assert!(eval_input_entry("-", &json!("hello")).unwrap());
        assert!(eval_input_entry("-", &Value::Null).unwrap());
    }

    // ── string literals ───────────────────────────────────────────────────────

    #[test]
    fn string_literal_match() {
        assert!(eval_input_entry(r#""approved""#, &json!("approved")).unwrap());
    }

    #[test]
    fn string_literal_no_match() {
        assert!(!eval_input_entry(r#""approved""#, &json!("rejected")).unwrap());
    }

    #[test]
    fn string_literal_against_number_is_false() {
        assert!(!eval_input_entry(r#""42""#, &json!(42)).unwrap());
    }

    // ── number literals ───────────────────────────────────────────────────────

    #[test]
    fn number_literal_match() {
        assert!(eval_input_entry("42", &json!(42)).unwrap());
    }

    #[test]
    fn number_literal_float_match() {
        assert!(eval_input_entry("3.14", &json!(3.14)).unwrap());
    }

    #[test]
    fn number_literal_no_match() {
        assert!(!eval_input_entry("42", &json!(43)).unwrap());
    }

    // ── boolean literals ──────────────────────────────────────────────────────

    #[test]
    fn bool_true_match() {
        assert!(eval_input_entry("true", &json!(true)).unwrap());
    }

    #[test]
    fn bool_false_match() {
        assert!(eval_input_entry("false", &json!(false)).unwrap());
    }

    #[test]
    fn bool_true_no_match() {
        assert!(!eval_input_entry("true", &json!(false)).unwrap());
    }

    // ── unary comparisons ─────────────────────────────────────────────────────

    #[test]
    fn unary_gte_true() {
        assert!(eval_input_entry(">= 10", &json!(10)).unwrap());
        assert!(eval_input_entry(">= 10", &json!(15)).unwrap());
    }

    #[test]
    fn unary_gte_false() {
        assert!(!eval_input_entry(">= 10", &json!(9)).unwrap());
    }

    #[test]
    fn unary_gt() {
        assert!(eval_input_entry("> 5", &json!(6)).unwrap());
        assert!(!eval_input_entry("> 5", &json!(5)).unwrap());
    }

    #[test]
    fn unary_lte() {
        assert!(eval_input_entry("<= 5", &json!(5)).unwrap());
        assert!(!eval_input_entry("<= 5", &json!(6)).unwrap());
    }

    #[test]
    fn unary_lt() {
        assert!(eval_input_entry("< 5", &json!(4)).unwrap());
        assert!(!eval_input_entry("< 5", &json!(5)).unwrap());
    }

    #[test]
    fn unary_eq() {
        assert!(eval_input_entry("= 7", &json!(7)).unwrap());
        assert!(!eval_input_entry("= 7", &json!(8)).unwrap());
    }

    #[test]
    fn unary_neq() {
        assert!(eval_input_entry("!= 7", &json!(8)).unwrap());
        assert!(!eval_input_entry("!= 7", &json!(7)).unwrap());
    }

    #[test]
    fn unary_string_eq() {
        assert!(eval_input_entry(r#"= "yes""#, &json!("yes")).unwrap());
        assert!(!eval_input_entry(r#"= "yes""#, &json!("no")).unwrap());
    }

    #[test]
    fn unary_string_neq() {
        assert!(eval_input_entry(r#"!= "foo""#, &json!("bar")).unwrap());
        assert!(!eval_input_entry(r#"!= "foo""#, &json!("foo")).unwrap());
    }

    // ── numeric ranges ────────────────────────────────────────────────────────

    #[test]
    fn range_closed_inclusive_inside() {
        assert!(eval_input_entry("[1..10]", &json!(5)).unwrap());
        assert!(eval_input_entry("[1..10]", &json!(1)).unwrap());
        assert!(eval_input_entry("[1..10]", &json!(10)).unwrap());
    }

    #[test]
    fn range_closed_inclusive_outside() {
        assert!(!eval_input_entry("[1..10]", &json!(0)).unwrap());
        assert!(!eval_input_entry("[1..10]", &json!(11)).unwrap());
    }

    #[test]
    fn range_open_exclusive() {
        assert!(eval_input_entry("(1..10)", &json!(5)).unwrap());
        assert!(!eval_input_entry("(1..10)", &json!(1)).unwrap());
        assert!(!eval_input_entry("(1..10)", &json!(10)).unwrap());
    }

    #[test]
    fn range_half_open_lower_exclusive() {
        assert!(eval_input_entry("(1..10]", &json!(10)).unwrap());
        assert!(!eval_input_entry("(1..10]", &json!(1)).unwrap());
    }

    #[test]
    fn range_half_open_upper_exclusive() {
        assert!(eval_input_entry("[1..10)", &json!(1)).unwrap());
        assert!(!eval_input_entry("[1..10)", &json!(10)).unwrap());
    }

    #[test]
    fn range_null_value_is_false() {
        assert!(!eval_input_entry("[1..10]", &Value::Null).unwrap());
    }

    // ── OR lists ──────────────────────────────────────────────────────────────

    #[test]
    fn or_list_number_match() {
        assert!(eval_input_entry("1,2,3", &json!(2)).unwrap());
    }

    #[test]
    fn or_list_number_no_match() {
        assert!(!eval_input_entry("1,2,3", &json!(4)).unwrap());
    }

    #[test]
    fn or_list_string_match() {
        assert!(eval_input_entry(r#""low","medium","high""#, &json!("medium")).unwrap());
    }

    #[test]
    fn or_list_string_no_match() {
        assert!(!eval_input_entry(r#""low","medium","high""#, &json!("critical")).unwrap());
    }

    #[test]
    fn or_list_mixed_ranges() {
        assert!(eval_input_entry("[1..5],[10..15]", &json!(12)).unwrap());
        assert!(!eval_input_entry("[1..5],[10..15]", &json!(7)).unwrap());
    }

    // ── null/missing values ───────────────────────────────────────────────────

    #[test]
    fn null_against_number_comparator_is_false() {
        assert!(!eval_input_entry("> 0", &Value::Null).unwrap());
    }

    #[test]
    fn null_against_string_literal_is_false() {
        assert!(!eval_input_entry(r#""foo""#, &Value::Null).unwrap());
    }

    // ── error cases ───────────────────────────────────────────────────────────

    #[test]
    fn unrecognised_cell_is_err() {
        assert!(eval_input_entry("???", &json!(1)).is_err());
    }
}
