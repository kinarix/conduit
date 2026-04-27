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

    Err(EngineError::DmnFeel(format!("Unrecognised FEEL cell: {cell:?}")))
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
    let sep = inner.find("..").ok_or_else(|| {
        EngineError::DmnFeel(format!("Malformed range: {cell:?}"))
    })?;

    let lo_str = inner[..sep].trim();
    let hi_str = inner[sep + 2..].trim();

    let lo: f64 = lo_str.parse().map_err(|_| {
        EngineError::DmnFeel(format!("Non-numeric range bound: {lo_str:?}"))
    })?;
    let hi: f64 = hi_str.parse().map_err(|_| {
        EngineError::DmnFeel(format!("Non-numeric range bound: {hi_str:?}"))
    })?;

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
            _ => return Err(EngineError::DmnFeel(format!("Operator {op} not valid for strings"))),
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
            _ => return Err(EngineError::DmnFeel(format!("Operator {op} not valid for booleans"))),
        });
    }

    // Numeric RHS
    let rhs_n: f64 = rhs.parse().map_err(|_| {
        EngineError::DmnFeel(format!("Cannot parse RHS as number: {rhs:?}"))
    })?;
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
