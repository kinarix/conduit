pub mod feel;

use std::collections::HashMap;

use crate::error::EngineError;

// Accepted DMN namespace URI
const DMN_NS: &str = "https://www.omg.org/spec/DMN/20191111/MODEL/";

#[derive(Debug, Clone)]
pub struct DecisionTable {
    pub decision_key: String,
    pub name: Option<String>,
    pub hit_policy: HitPolicy,
    pub inputs: Vec<InputClause>,
    pub outputs: Vec<OutputClause>,
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HitPolicy {
    Unique,
    First,
    Collect,
    RuleOrder,
}

#[derive(Debug, Clone)]
pub struct InputClause {
    pub expression: String,
}

#[derive(Debug, Clone)]
pub struct OutputClause {
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct Rule {
    pub input_entries: Vec<String>,
    pub output_entries: Vec<String>,
}

/// Parse a DMN XML string into one or more `DecisionTable` structs.
pub fn parse(xml: &str) -> Result<Vec<DecisionTable>, EngineError> {
    let doc = roxmltree::Document::parse(xml)
        .map_err(|e| EngineError::DmnParse(e.to_string()))?;

    let definitions = doc.root_element();
    if definitions.tag_name().name() != "definitions" {
        return Err(EngineError::DmnParse(
            "Root element must be <definitions>".to_string(),
        ));
    }

    let mut tables = Vec::new();

    for decision in definitions.children().filter(|n| {
        n.is_element()
            && n.tag_name().name() == "decision"
            && n.tag_name().namespace() == Some(DMN_NS)
    }) {
        let key = decision
            .attribute("id")
            .ok_or_else(|| EngineError::DmnParse("<decision> missing id attribute".to_string()))?
            .to_string();
        let name = decision.attribute("name").map(|s| s.to_string());

        let table_node = decision
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "decisionTable")
            .ok_or_else(|| {
                EngineError::DmnParse(format!("Decision '{key}' has no <decisionTable>"))
            })?;

        let hit_policy = match table_node.attribute("hitPolicy").unwrap_or("UNIQUE") {
            "UNIQUE" => HitPolicy::Unique,
            "FIRST" => HitPolicy::First,
            "COLLECT" => HitPolicy::Collect,
            "RULE ORDER" | "RULE_ORDER" => HitPolicy::RuleOrder,
            other => {
                return Err(EngineError::DmnParse(format!(
                    "Unknown hit policy: {other}"
                )))
            }
        };

        let mut inputs = Vec::new();
        let mut outputs = Vec::new();
        let mut rules = Vec::new();

        for child in table_node.children().filter(|n| n.is_element()) {
            match child.tag_name().name() {
                "input" => {
                    let expr_node = child
                        .children()
                        .find(|n| n.is_element() && n.tag_name().name() == "inputExpression")
                        .ok_or_else(|| {
                            EngineError::DmnParse("<input> missing <inputExpression>".to_string())
                        })?;
                    let text = expr_node
                        .children()
                        .find(|n| n.is_element() && n.tag_name().name() == "text")
                        .and_then(|n| n.text())
                        .unwrap_or("")
                        .to_string();
                    inputs.push(InputClause { expression: text });
                }
                "output" => {
                    let name = child
                        .attribute("name")
                        .ok_or_else(|| {
                            EngineError::DmnParse("<output> missing name attribute".to_string())
                        })?
                        .to_string();
                    outputs.push(OutputClause { name });
                }
                "rule" => {
                    let input_entries: Vec<String> = child
                        .children()
                        .filter(|n| n.is_element() && n.tag_name().name() == "inputEntry")
                        .map(|n| {
                            n.children()
                                .find(|c| c.is_element() && c.tag_name().name() == "text")
                                .and_then(|c| c.text())
                                .unwrap_or("-")
                                .to_string()
                        })
                        .collect();
                    let output_entries: Vec<String> = child
                        .children()
                        .filter(|n| n.is_element() && n.tag_name().name() == "outputEntry")
                        .map(|n| {
                            n.children()
                                .find(|c| c.is_element() && c.tag_name().name() == "text")
                                .and_then(|c| c.text())
                                .unwrap_or("")
                                .to_string()
                        })
                        .collect();
                    rules.push(Rule {
                        input_entries,
                        output_entries,
                    });
                }
                _ => {}
            }
        }

        if outputs.is_empty() {
            return Err(EngineError::DmnParse(format!(
                "Decision '{key}' has no output columns"
            )));
        }

        tables.push(DecisionTable {
            decision_key: key,
            name,
            hit_policy,
            inputs,
            outputs,
            rules,
        });
    }

    if tables.is_empty() {
        return Err(EngineError::DmnParse(
            "No <decision> elements found in DMN file".to_string(),
        ));
    }

    Ok(tables)
}

/// Evaluate a decision table against an input context.
///
/// Returns a map of output-column-name → value.
/// For COLLECT/RULE_ORDER, each output column value is a JSON array.
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
            1 => Ok(build_scalar_output(table, matched[0])?),
            _ => Err(EngineError::DmnMultipleMatches),
        },
        HitPolicy::First => {
            let rule = matched.into_iter().next().ok_or(EngineError::DmnNoMatch)?;
            Ok(build_scalar_output(table, rule)?)
        }
        HitPolicy::Collect | HitPolicy::RuleOrder => {
            if matched.is_empty() {
                return Err(EngineError::DmnNoMatch);
            }
            Ok(build_list_output(table, &matched)?)
        }
    }
}

fn build_scalar_output(
    table: &DecisionTable,
    rule: &Rule,
) -> Result<HashMap<String, serde_json::Value>, EngineError> {
    let mut out = HashMap::new();
    for (i, col) in table.outputs.iter().enumerate() {
        let raw = rule.output_entries.get(i).map(|s| s.as_str()).unwrap_or("");
        out.insert(col.name.clone(), parse_output_literal(raw)?);
    }
    Ok(out)
}

fn build_list_output(
    table: &DecisionTable,
    rules: &[&Rule],
) -> Result<HashMap<String, serde_json::Value>, EngineError> {
    let mut out: HashMap<String, Vec<serde_json::Value>> = HashMap::new();
    for col in &table.outputs {
        out.insert(col.name.clone(), Vec::new());
    }
    for rule in rules {
        for (i, col) in table.outputs.iter().enumerate() {
            let raw = rule.output_entries.get(i).map(|s| s.as_str()).unwrap_or("");
            let v = parse_output_literal(raw)?;
            out.get_mut(&col.name).unwrap().push(v);
        }
    }
    Ok(out
        .into_iter()
        .map(|(k, v)| (k, serde_json::Value::Array(v)))
        .collect())
}

fn parse_output_literal(raw: &str) -> Result<serde_json::Value, EngineError> {
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
    Err(EngineError::DmnParse(format!(
        "Cannot parse output literal: {raw:?}"
    )))
}
