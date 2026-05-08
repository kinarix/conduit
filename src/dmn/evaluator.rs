//! Decision-table evaluation: hit-policy dispatch and output construction.

use std::collections::HashMap;

use super::{feel, feel_stdlib, CollectAggregator, DecisionTable, HitPolicy, Rule};
use crate::error::EngineError;

/// Evaluate a decision table against an input context.
///
/// Returns a map of output-column-name → value.
/// For COLLECT (no aggregator) and RULE_ORDER, each output column value is a JSON array.
/// For COLLECT+SUM/MIN/MAX, each output column value is an aggregated scalar.
/// For COLLECT+COUNT, returns `{"count": N}` (or the first output column name).
pub fn evaluate(
    table: &DecisionTable,
    ctx: &HashMap<String, serde_json::Value>,
) -> Result<HashMap<String, serde_json::Value>, EngineError> {
    let null = serde_json::Value::Null;

    // Collect all matching rules
    let mut matched: Vec<&Rule> = Vec::new();

    'outer: for rule in &table.rules {
        for (i, input_clause) in table.inputs.iter().enumerate() {
            let cell = rule.input_entries.get(i).map(|s| s.as_str()).unwrap_or("-");
            let value = ctx.get(&input_clause.expression).unwrap_or(&null);
            if !feel::eval_input_entry(cell, value)? {
                continue 'outer;
            }
        }
        matched.push(rule);
    }

    match table.hit_policy {
        HitPolicy::Unique => match matched.len() {
            0 => Err(EngineError::DmnNoMatch),
            1 => build_scalar_output(table, matched[0], ctx),
            _ => Err(EngineError::DmnMultipleMatches),
        },
        HitPolicy::First => {
            let rule = matched.into_iter().next().ok_or(EngineError::DmnNoMatch)?;
            build_scalar_output(table, rule, ctx)
        }
        HitPolicy::Collect => {
            if matched.is_empty() {
                return Err(EngineError::DmnNoMatch);
            }
            match &table.collect_aggregator {
                None => build_list_output(table, &matched, ctx),
                Some(agg) => build_aggregate_output(table, &matched, ctx, agg),
            }
        }
        HitPolicy::RuleOrder => {
            if matched.is_empty() {
                return Err(EngineError::DmnNoMatch);
            }
            build_list_output(table, &matched, ctx)
        }
        HitPolicy::Any => {
            if matched.is_empty() {
                return Err(EngineError::DmnNoMatch);
            }
            let first_out = build_scalar_output(table, matched[0], ctx)?;
            for rule in &matched[1..] {
                let out = build_scalar_output(table, rule, ctx)?;
                if out != first_out {
                    return Err(EngineError::DmnFeel(
                        "ANY hit policy: matching rules produce different outputs".to_string(),
                    ));
                }
            }
            Ok(first_out)
        }
        HitPolicy::Priority => {
            if matched.is_empty() {
                return Err(EngineError::DmnNoMatch);
            }
            let sorted = sort_by_priority(table, &matched, ctx)?;
            let rule = sorted.into_iter().next().ok_or(EngineError::DmnNoMatch)?;
            build_scalar_output(table, rule, ctx)
        }
        HitPolicy::OutputOrder => {
            if matched.is_empty() {
                return Err(EngineError::DmnNoMatch);
            }
            let sorted = sort_by_priority(table, &matched, ctx)?;
            build_list_output(table, &sorted, ctx)
        }
    }
}

/// Sort matched rules by the priority of their first output column's value.
/// Rules whose output value appears earlier in `output_values` sort first.
/// Rules with no priority match sort last (stable).
fn sort_by_priority<'a>(
    table: &DecisionTable,
    rules: &[&'a Rule],
    ctx: &HashMap<String, serde_json::Value>,
) -> Result<Vec<&'a Rule>, EngineError> {
    let priority_list: &[String] = table
        .outputs
        .first()
        .map(|o| o.output_values.as_slice())
        .unwrap_or(&[]);

    // Evaluate output value for each rule (first output column)
    let mut with_priority: Vec<(usize, usize, &'a Rule)> = Vec::new();
    for (orig_idx, rule) in rules.iter().enumerate() {
        let rule: &'a Rule = rule;
        let raw = rule
            .output_entries
            .first()
            .map(|s| s.as_str())
            .unwrap_or("");
        let val = parse_output_value(raw, ctx)?;
        let val_str = match &val {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        let priority = priority_list
            .iter()
            .position(|p| p == &val_str)
            .unwrap_or(usize::MAX);
        with_priority.push((priority, orig_idx, rule));
    }

    // Sort by priority rank (stable for equal ranks — preserve document order)
    with_priority.sort_by_key(|(prio, orig_idx, _)| (*prio, *orig_idx));
    Ok(with_priority.into_iter().map(|(_, _, r)| r).collect())
}

fn build_scalar_output(
    table: &DecisionTable,
    rule: &Rule,
    ctx: &HashMap<String, serde_json::Value>,
) -> Result<HashMap<String, serde_json::Value>, EngineError> {
    let mut out = HashMap::new();
    for (i, col) in table.outputs.iter().enumerate() {
        let raw = rule.output_entries.get(i).map(|s| s.as_str()).unwrap_or("");
        out.insert(col.name.clone(), parse_output_value(raw, ctx)?);
    }
    Ok(out)
}

fn build_list_output(
    table: &DecisionTable,
    rules: &[&Rule],
    ctx: &HashMap<String, serde_json::Value>,
) -> Result<HashMap<String, serde_json::Value>, EngineError> {
    let mut out: HashMap<String, Vec<serde_json::Value>> = HashMap::new();
    for col in &table.outputs {
        out.insert(col.name.clone(), Vec::new());
    }
    for rule in rules {
        for (i, col) in table.outputs.iter().enumerate() {
            let raw = rule.output_entries.get(i).map(|s| s.as_str()).unwrap_or("");
            let v = parse_output_value(raw, ctx)?;
            out.entry(col.name.clone()).or_default().push(v);
        }
    }
    Ok(out
        .into_iter()
        .map(|(k, v)| (k, serde_json::Value::Array(v)))
        .collect())
}

fn build_aggregate_output(
    table: &DecisionTable,
    rules: &[&Rule],
    ctx: &HashMap<String, serde_json::Value>,
    agg: &CollectAggregator,
) -> Result<HashMap<String, serde_json::Value>, EngineError> {
    let mut out = HashMap::new();

    if matches!(agg, CollectAggregator::Count) {
        // COUNT: one entry per output column; value is the count of matched rules
        for col in &table.outputs {
            out.insert(col.name.clone(), serde_json::json!(rules.len() as i64));
        }
        return Ok(out);
    }

    for (i, col) in table.outputs.iter().enumerate() {
        let mut nums: Vec<f64> = Vec::new();
        for rule in rules {
            let raw = rule.output_entries.get(i).map(|s| s.as_str()).unwrap_or("");
            let v = parse_output_value(raw, ctx)?;
            match &v {
                serde_json::Value::Number(n) => {
                    if let Some(f) = n.as_f64() {
                        nums.push(f);
                    }
                }
                other => {
                    return Err(EngineError::DmnFeel(format!(
                        "COLLECT aggregator requires numeric outputs; got {other:?} for column '{}'",
                        col.name
                    )));
                }
            }
        }
        if nums.is_empty() {
            out.insert(col.name.clone(), serde_json::Value::Null);
            continue;
        }
        let result = match agg {
            CollectAggregator::Sum => nums.iter().sum::<f64>(),
            CollectAggregator::Min => nums.iter().cloned().fold(f64::INFINITY, f64::min),
            CollectAggregator::Max => nums.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
            CollectAggregator::Count => unreachable!(),
        };
        let json_val = if result.fract() == 0.0 && result.abs() < i64::MAX as f64 {
            serde_json::json!(result as i64)
        } else {
            serde_json::Number::from_f64(result)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null)
        };
        out.insert(col.name.clone(), json_val);
    }

    Ok(out)
}

/// Parse a DMN output cell value. Tries FEEL literals first; if the string
/// doesn't parse as a literal, delegates to the full FEEL evaluator so that
/// output cells may contain expressions (e.g. `amount * 0.1`, `"tier_" + id`).
fn parse_output_value(
    raw: &str,
    ctx: &HashMap<String, serde_json::Value>,
) -> Result<serde_json::Value, EngineError> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(serde_json::Value::Null);
    }
    if raw.starts_with('"') && raw.ends_with('"') && raw.len() >= 2 {
        return Ok(serde_json::Value::String(raw[1..raw.len() - 1].to_string()));
    }
    if raw == "true" {
        return Ok(serde_json::Value::Bool(true));
    }
    if raw == "false" {
        return Ok(serde_json::Value::Bool(false));
    }
    if raw == "null" {
        return Ok(serde_json::Value::Null);
    }
    if let Ok(n) = raw.parse::<i64>() {
        return Ok(serde_json::json!(n));
    }
    if let Ok(f) = raw.parse::<f64>() {
        return Ok(serde_json::json!(f));
    }
    // Fall back to full FEEL evaluation (expressions in output cells)
    feel_stdlib::eval_feel(raw, ctx)
}
