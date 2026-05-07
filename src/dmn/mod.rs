pub mod feel;
pub mod feel_stdlib;

use std::collections::HashMap;

use crate::error::EngineError;

// Accepted DMN namespace URI
const DMN_NS: &str = "https://www.omg.org/spec/DMN/20191111/MODEL/";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DecisionTable {
    pub decision_key: String,
    pub name: Option<String>,
    pub hit_policy: HitPolicy,
    /// Set when `hit_policy` is `Collect` with an aggregation attribute.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collect_aggregator: Option<CollectAggregator>,
    pub inputs: Vec<InputClause>,
    pub outputs: Vec<OutputClause>,
    pub rules: Vec<Rule>,
    /// Keys of other decisions that this table takes as inputs (DRD edges).
    #[serde(default)]
    pub required_decisions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HitPolicy {
    Unique,
    First,
    Collect,
    RuleOrder,
    Any,
    Priority,
    OutputOrder,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CollectAggregator {
    Sum,
    Min,
    Max,
    Count,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InputClause {
    pub expression: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OutputClause {
    pub name: String,
    /// Ordered list of allowed output values for PRIORITY / OUTPUT ORDER policies.
    /// Earlier values have higher priority.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub output_values: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Rule {
    pub input_entries: Vec<String>,
    pub output_entries: Vec<String>,
}

// ─── Parser ──────────────────────────────────────────────────────────────────

/// Parse a DMN XML string into one or more `DecisionTable` structs.
pub fn parse(xml: &str) -> Result<Vec<DecisionTable>, EngineError> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| EngineError::DmnParse(e.to_string()))?;

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

        // DRD: collect required-decision references
        let required_decisions: Vec<String> = decision
            .children()
            .filter(|n| n.is_element() && n.tag_name().name() == "informationRequirement")
            .flat_map(|ir| {
                ir.children()
                    .filter(|c| c.is_element() && c.tag_name().name() == "requiredDecision")
                    .filter_map(|c| c.attribute("href"))
                    .map(|href| href.trim_start_matches('#').to_string())
                    .collect::<Vec<_>>()
            })
            .collect();

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
            "ANY" => HitPolicy::Any,
            "PRIORITY" => HitPolicy::Priority,
            "OUTPUT ORDER" | "OUTPUT_ORDER" => HitPolicy::OutputOrder,
            other => {
                return Err(EngineError::DmnParse(format!(
                    "Unknown hit policy: {other}"
                )))
            }
        };

        let collect_aggregator = match table_node.attribute("aggregation") {
            Some("SUM") => Some(CollectAggregator::Sum),
            Some("MIN") => Some(CollectAggregator::Min),
            Some("MAX") => Some(CollectAggregator::Max),
            Some("COUNT") => Some(CollectAggregator::Count),
            _ => None,
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
                    let out_name = child
                        .attribute("name")
                        .ok_or_else(|| {
                            EngineError::DmnParse("<output> missing name attribute".to_string())
                        })?
                        .to_string();
                    // Parse outputValues for PRIORITY / OUTPUT ORDER
                    let output_values = child
                        .children()
                        .find(|n| n.is_element() && n.tag_name().name() == "outputValues")
                        .and_then(|ov| {
                            ov.children()
                                .find(|c| c.is_element() && c.tag_name().name() == "text")
                                .and_then(|t| t.text())
                        })
                        .map(parse_output_values_list)
                        .unwrap_or_default();
                    outputs.push(OutputClause {
                        name: out_name,
                        output_values,
                    });
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
            collect_aggregator,
            inputs,
            outputs,
            rules,
            required_decisions,
        });
    }

    if tables.is_empty() {
        return Err(EngineError::DmnParse(
            "No <decision> elements found in DMN file".to_string(),
        ));
    }

    Ok(tables)
}

/// Parse `"\"gold\",\"silver\",\"bronze\""` into `["gold", "silver", "bronze"]`.
fn parse_output_values_list(s: &str) -> Vec<String> {
    let s = s.trim();
    feel::split_or_list(s)
        .into_iter()
        .map(|part| {
            let p = part.trim();
            if p.starts_with('"') && p.ends_with('"') && p.len() >= 2 {
                p[1..p.len() - 1].to_string()
            } else {
                p.to_string()
            }
        })
        .collect()
}

// ─── Evaluator ───────────────────────────────────────────────────────────────

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
        let rule: &'a Rule = *rule;
        let raw = rule.output_entries.first().map(|s| s.as_str()).unwrap_or("");
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
            out.get_mut(&col.name).unwrap().push(v);
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
            out.insert(
                col.name.clone(),
                serde_json::json!(rules.len() as i64),
            );
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

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::{evaluate, parse, CollectAggregator, HitPolicy};
    use serde_json::json;
    use std::collections::HashMap;

    const DMN_NS: &str = "https://www.omg.org/spec/DMN/20191111/MODEL/";

    // ── DMN fixture builders ──────────────────────────────────────────────────

    fn age_category_dmn() -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="{ns}" id="def1" name="AgeCategory">
  <decision xmlns="{ns}" id="ageCategory" name="Age Category">
    <decisionTable id="dt1" hitPolicy="UNIQUE">
      <input id="i1"><inputExpression id="ie1"><text>age</text></inputExpression></input>
      <output id="o1" name="category"/>
      <rule id="r1">
        <inputEntry id="in1"><text>&gt;= 18</text></inputEntry>
        <outputEntry id="out1"><text>"adult"</text></outputEntry>
      </rule>
      <rule id="r2">
        <inputEntry id="in2"><text>&lt; 18</text></inputEntry>
        <outputEntry id="out2"><text>"minor"</text></outputEntry>
      </rule>
    </decisionTable>
  </decision>
</definitions>"#,
            ns = DMN_NS
        )
    }

    fn collect_dmn() -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="{ns}" id="def1" name="Collect">
  <decision xmlns="{ns}" id="scores" name="Scores">
    <decisionTable id="dt1" hitPolicy="COLLECT">
      <input id="i1"><inputExpression id="ie1"><text>flag</text></inputExpression></input>
      <output id="o1" name="score"/>
      <rule id="r1">
        <inputEntry id="in1"><text>-</text></inputEntry>
        <outputEntry id="out1"><text>10</text></outputEntry>
      </rule>
      <rule id="r2">
        <inputEntry id="in2"><text>-</text></inputEntry>
        <outputEntry id="out2"><text>20</text></outputEntry>
      </rule>
    </decisionTable>
  </decision>
</definitions>"#,
            ns = DMN_NS
        )
    }

    fn first_dmn() -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="{ns}" id="def1" name="First">
  <decision xmlns="{ns}" id="result" name="Result">
    <decisionTable id="dt1" hitPolicy="FIRST">
      <input id="i1"><inputExpression id="ie1"><text>x</text></inputExpression></input>
      <output id="o1" name="out"/>
      <rule id="r1">
        <inputEntry id="in1"><text>&gt; 0</text></inputEntry>
        <outputEntry id="out1"><text>"positive"</text></outputEntry>
      </rule>
      <rule id="r2">
        <inputEntry id="in2"><text>-</text></inputEntry>
        <outputEntry id="out2"><text>"other"</text></outputEntry>
      </rule>
    </decisionTable>
  </decision>
</definitions>"#,
            ns = DMN_NS
        )
    }

    fn collect_sum_dmn() -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="{ns}" id="def1" name="CollectSum">
  <decision xmlns="{ns}" id="bonus" name="Bonus">
    <decisionTable id="dt1" hitPolicy="COLLECT" aggregation="SUM">
      <input id="i1"><inputExpression><text>flag</text></inputExpression></input>
      <output id="o1" name="amount"/>
      <rule id="r1">
        <inputEntry><text>-</text></inputEntry>
        <outputEntry><text>100</text></outputEntry>
      </rule>
      <rule id="r2">
        <inputEntry><text>-</text></inputEntry>
        <outputEntry><text>50</text></outputEntry>
      </rule>
    </decisionTable>
  </decision>
</definitions>"#,
            ns = DMN_NS
        )
    }

    fn collect_count_dmn() -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="{ns}" id="def1" name="CollectCount">
  <decision xmlns="{ns}" id="matchCount" name="Match Count">
    <decisionTable id="dt1" hitPolicy="COLLECT" aggregation="COUNT">
      <input id="i1"><inputExpression><text>flag</text></inputExpression></input>
      <output id="o1" name="count"/>
      <rule id="r1">
        <inputEntry><text>-</text></inputEntry>
        <outputEntry><text>1</text></outputEntry>
      </rule>
      <rule id="r2">
        <inputEntry><text>-</text></inputEntry>
        <outputEntry><text>1</text></outputEntry>
      </rule>
      <rule id="r3">
        <inputEntry><text>-</text></inputEntry>
        <outputEntry><text>1</text></outputEntry>
      </rule>
    </decisionTable>
  </decision>
</definitions>"#,
            ns = DMN_NS
        )
    }

    fn any_dmn() -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="{ns}" id="def1" name="Any">
  <decision xmlns="{ns}" id="tier" name="Tier">
    <decisionTable id="dt1" hitPolicy="ANY">
      <input id="i1"><inputExpression><text>score</text></inputExpression></input>
      <output id="o1" name="level"/>
      <rule id="r1">
        <inputEntry><text>&gt;= 80</text></inputEntry>
        <outputEntry><text>"high"</text></outputEntry>
      </rule>
      <rule id="r2">
        <inputEntry><text>&gt;= 75</text></inputEntry>
        <outputEntry><text>"high"</text></outputEntry>
      </rule>
    </decisionTable>
  </decision>
</definitions>"#,
            ns = DMN_NS
        )
    }

    fn priority_dmn() -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="{ns}" id="def1" name="Priority">
  <decision xmlns="{ns}" id="risk" name="Risk">
    <decisionTable id="dt1" hitPolicy="PRIORITY">
      <input id="i1"><inputExpression><text>score</text></inputExpression></input>
      <output id="o1" name="level">
        <outputValues><text>"high","medium","low"</text></outputValues>
      </output>
      <rule id="r1">
        <inputEntry><text>&gt;= 50</text></inputEntry>
        <outputEntry><text>"medium"</text></outputEntry>
      </rule>
      <rule id="r2">
        <inputEntry><text>&gt;= 80</text></inputEntry>
        <outputEntry><text>"high"</text></outputEntry>
      </rule>
    </decisionTable>
  </decision>
</definitions>"#,
            ns = DMN_NS
        )
    }

    fn output_order_dmn() -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="{ns}" id="def1" name="OutputOrder">
  <decision xmlns="{ns}" id="risk" name="Risk">
    <decisionTable id="dt1" hitPolicy="OUTPUT ORDER">
      <input id="i1"><inputExpression><text>score</text></inputExpression></input>
      <output id="o1" name="level">
        <outputValues><text>"high","medium","low"</text></outputValues>
      </output>
      <rule id="r1">
        <inputEntry><text>&gt;= 50</text></inputEntry>
        <outputEntry><text>"medium"</text></outputEntry>
      </rule>
      <rule id="r2">
        <inputEntry><text>&gt;= 80</text></inputEntry>
        <outputEntry><text>"high"</text></outputEntry>
      </rule>
    </decisionTable>
  </decision>
</definitions>"#,
            ns = DMN_NS
        )
    }

    fn drd_dmn() -> String {
        format!(
            r##"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="{ns}" id="def1" name="DRD">
  <decision xmlns="{ns}" id="finalDecision" name="Final">
    <informationRequirement>
      <requiredDecision href="#subDecision"/>
    </informationRequirement>
    <decisionTable id="dt1" hitPolicy="FIRST">
      <input id="i1"><inputExpression><text>x</text></inputExpression></input>
      <output id="o1" name="out"/>
      <rule id="r1">
        <inputEntry><text>-</text></inputEntry>
        <outputEntry><text>"ok"</text></outputEntry>
      </rule>
    </decisionTable>
  </decision>
  <decision xmlns="{ns}" id="subDecision" name="Sub">
    <decisionTable id="dt2" hitPolicy="FIRST">
      <input id="i2"><inputExpression><text>y</text></inputExpression></input>
      <output id="o2" name="sub_out"/>
      <rule id="r2">
        <inputEntry><text>-</text></inputEntry>
        <outputEntry><text>"sub"</text></outputEntry>
      </rule>
    </decisionTable>
  </decision>
</definitions>"##,
            ns = DMN_NS
        )
    }

    // ── parse ─────────────────────────────────────────────────────────────────

    #[test]
    fn parse_valid_dmn() {
        let tables = parse(&age_category_dmn()).unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].decision_key, "ageCategory");
        assert_eq!(tables[0].inputs.len(), 1);
        assert_eq!(tables[0].outputs.len(), 1);
        assert_eq!(tables[0].rules.len(), 2);
    }

    #[test]
    fn parse_bad_xml_is_err() {
        assert!(parse("<not-valid").is_err());
    }

    #[test]
    fn parse_wrong_root_is_err() {
        assert!(parse(r#"<root xmlns="x"/>"#).is_err());
    }

    #[test]
    fn parse_no_decisions_is_err() {
        let xml = format!(r#"<?xml version="1.0"?><definitions xmlns="{}"/>"#, DMN_NS);
        assert!(parse(&xml).is_err());
    }

    #[test]
    fn parse_unknown_hit_policy_is_err() {
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="{ns}" id="d">
  <decision xmlns="{ns}" id="dec1">
    <decisionTable id="dt1" hitPolicy="BOGUS">
      <output id="o1" name="out"/>
    </decisionTable>
  </decision>
</definitions>"#,
            ns = DMN_NS
        );
        assert!(parse(&xml).is_err());
    }

    #[test]
    fn parse_collect_sum_aggregator() {
        let tables = parse(&collect_sum_dmn()).unwrap();
        assert_eq!(tables[0].hit_policy, HitPolicy::Collect);
        assert_eq!(
            tables[0].collect_aggregator,
            Some(CollectAggregator::Sum)
        );
    }

    #[test]
    fn parse_collect_count_aggregator() {
        let tables = parse(&collect_count_dmn()).unwrap();
        assert_eq!(
            tables[0].collect_aggregator,
            Some(CollectAggregator::Count)
        );
    }

    #[test]
    fn parse_priority_hit_policy() {
        let tables = parse(&priority_dmn()).unwrap();
        assert_eq!(tables[0].hit_policy, HitPolicy::Priority);
        assert_eq!(
            tables[0].outputs[0].output_values,
            vec!["high", "medium", "low"]
        );
    }

    #[test]
    fn parse_output_order_hit_policy() {
        let tables = parse(&output_order_dmn()).unwrap();
        assert_eq!(tables[0].hit_policy, HitPolicy::OutputOrder);
    }

    #[test]
    fn parse_any_hit_policy() {
        let tables = parse(&any_dmn()).unwrap();
        assert_eq!(tables[0].hit_policy, HitPolicy::Any);
    }

    #[test]
    fn parse_drd_required_decisions() {
        let tables = parse(&drd_dmn()).unwrap();
        let final_table = tables.iter().find(|t| t.decision_key == "finalDecision").unwrap();
        assert_eq!(final_table.required_decisions, vec!["subDecision"]);
        let sub_table = tables.iter().find(|t| t.decision_key == "subDecision").unwrap();
        assert!(sub_table.required_decisions.is_empty());
    }

    // ── evaluate: UNIQUE ──────────────────────────────────────────────────────

    #[test]
    fn unique_match_adult() {
        let tables = parse(&age_category_dmn()).unwrap();
        let mut ctx = HashMap::new();
        ctx.insert("age".to_string(), json!(25));
        let result = evaluate(&tables[0], &ctx).unwrap();
        assert_eq!(result["category"], json!("adult"));
    }

    #[test]
    fn unique_match_minor() {
        let tables = parse(&age_category_dmn()).unwrap();
        let mut ctx = HashMap::new();
        ctx.insert("age".to_string(), json!(10));
        let result = evaluate(&tables[0], &ctx).unwrap();
        assert_eq!(result["category"], json!("minor"));
    }

    #[test]
    fn unique_no_match_is_err() {
        let tables = parse(&age_category_dmn()).unwrap();
        let ctx = HashMap::new(); // missing age → no rule fires
        assert!(evaluate(&tables[0], &ctx).is_err());
    }

    // ── evaluate: FIRST ───────────────────────────────────────────────────────

    #[test]
    fn first_returns_first_matching_rule() {
        let tables = parse(&first_dmn()).unwrap();
        let mut ctx = HashMap::new();
        ctx.insert("x".to_string(), json!(5));
        let result = evaluate(&tables[0], &ctx).unwrap();
        assert_eq!(result["out"], json!("positive"));
    }

    #[test]
    fn first_falls_through_to_default() {
        let tables = parse(&first_dmn()).unwrap();
        let mut ctx = HashMap::new();
        ctx.insert("x".to_string(), json!(-1));
        let result = evaluate(&tables[0], &ctx).unwrap();
        assert_eq!(result["out"], json!("other"));
    }

    // ── evaluate: COLLECT (no aggregator) ─────────────────────────────────────

    #[test]
    fn collect_returns_all_matches_as_array() {
        let tables = parse(&collect_dmn()).unwrap();
        let mut ctx = HashMap::new();
        ctx.insert("flag".to_string(), json!(true));
        let result = evaluate(&tables[0], &ctx).unwrap();
        assert_eq!(result["score"], json!([10, 20]));
    }

    // ── evaluate: COLLECT+SUM ─────────────────────────────────────────────────

    #[test]
    fn collect_sum_aggregates_numeric_outputs() {
        let tables = parse(&collect_sum_dmn()).unwrap();
        let mut ctx = HashMap::new();
        ctx.insert("flag".to_string(), json!(true));
        let result = evaluate(&tables[0], &ctx).unwrap();
        assert_eq!(result["amount"], json!(150));
    }

    #[test]
    fn collect_min_returns_minimum() {
        let xml = format!(
            r#"<?xml version="1.0"?>
<definitions xmlns="{ns}" id="d">
  <decision xmlns="{ns}" id="d1">
    <decisionTable hitPolicy="COLLECT" aggregation="MIN">
      <input><inputExpression><text>x</text></inputExpression></input>
      <output name="val"/>
      <rule><inputEntry><text>-</text></inputEntry><outputEntry><text>10</text></outputEntry></rule>
      <rule><inputEntry><text>-</text></inputEntry><outputEntry><text>3</text></outputEntry></rule>
      <rule><inputEntry><text>-</text></inputEntry><outputEntry><text>7</text></outputEntry></rule>
    </decisionTable>
  </decision>
</definitions>"#,
            ns = DMN_NS
        );
        let tables = parse(&xml).unwrap();
        let ctx = HashMap::new();
        let result = evaluate(&tables[0], &ctx).unwrap();
        assert_eq!(result["val"], json!(3));
    }

    #[test]
    fn collect_max_returns_maximum() {
        let xml = format!(
            r#"<?xml version="1.0"?>
<definitions xmlns="{ns}" id="d">
  <decision xmlns="{ns}" id="d1">
    <decisionTable hitPolicy="COLLECT" aggregation="MAX">
      <input><inputExpression><text>x</text></inputExpression></input>
      <output name="val"/>
      <rule><inputEntry><text>-</text></inputEntry><outputEntry><text>10</text></outputEntry></rule>
      <rule><inputEntry><text>-</text></inputEntry><outputEntry><text>3</text></outputEntry></rule>
      <rule><inputEntry><text>-</text></inputEntry><outputEntry><text>7</text></outputEntry></rule>
    </decisionTable>
  </decision>
</definitions>"#,
            ns = DMN_NS
        );
        let tables = parse(&xml).unwrap();
        let ctx = HashMap::new();
        let result = evaluate(&tables[0], &ctx).unwrap();
        assert_eq!(result["val"], json!(10));
    }

    #[test]
    fn collect_count_returns_matched_rule_count() {
        let tables = parse(&collect_count_dmn()).unwrap();
        let mut ctx = HashMap::new();
        ctx.insert("flag".to_string(), json!(true));
        let result = evaluate(&tables[0], &ctx).unwrap();
        assert_eq!(result["count"], json!(3));
    }

    // ── evaluate: ANY ─────────────────────────────────────────────────────────

    #[test]
    fn any_all_same_output_returns_it() {
        let tables = parse(&any_dmn()).unwrap();
        let mut ctx = HashMap::new();
        ctx.insert("score".to_string(), json!(90)); // matches both rules
        let result = evaluate(&tables[0], &ctx).unwrap();
        assert_eq!(result["level"], json!("high"));
    }

    #[test]
    fn any_conflicting_outputs_is_err() {
        let xml = format!(
            r#"<?xml version="1.0"?>
<definitions xmlns="{ns}" id="d">
  <decision xmlns="{ns}" id="d1">
    <decisionTable hitPolicy="ANY">
      <input><inputExpression><text>x</text></inputExpression></input>
      <output name="out"/>
      <rule><inputEntry><text>-</text></inputEntry><outputEntry><text>"a"</text></outputEntry></rule>
      <rule><inputEntry><text>-</text></inputEntry><outputEntry><text>"b"</text></outputEntry></rule>
    </decisionTable>
  </decision>
</definitions>"#,
            ns = DMN_NS
        );
        let tables = parse(&xml).unwrap();
        let ctx = HashMap::new();
        assert!(evaluate(&tables[0], &ctx).is_err());
    }

    #[test]
    fn any_no_match_is_err() {
        let tables = parse(&any_dmn()).unwrap();
        let mut ctx = HashMap::new();
        ctx.insert("score".to_string(), json!(10)); // below all thresholds
        assert!(evaluate(&tables[0], &ctx).is_err());
    }

    // ── evaluate: PRIORITY ───────────────────────────────────────────────────

    #[test]
    fn priority_returns_highest_priority_match() {
        let tables = parse(&priority_dmn()).unwrap();
        let mut ctx = HashMap::new();
        ctx.insert("score".to_string(), json!(90)); // both rules match; "high" > "medium"
        let result = evaluate(&tables[0], &ctx).unwrap();
        assert_eq!(result["level"], json!("high"));
    }

    #[test]
    fn priority_only_one_match() {
        let tables = parse(&priority_dmn()).unwrap();
        let mut ctx = HashMap::new();
        ctx.insert("score".to_string(), json!(60)); // only rule 1 matches (>= 50 but < 80)
        let result = evaluate(&tables[0], &ctx).unwrap();
        assert_eq!(result["level"], json!("medium"));
    }

    #[test]
    fn priority_no_match_is_err() {
        let tables = parse(&priority_dmn()).unwrap();
        let mut ctx = HashMap::new();
        ctx.insert("score".to_string(), json!(10)); // below all thresholds
        assert!(evaluate(&tables[0], &ctx).is_err());
    }

    // ── evaluate: OUTPUT ORDER ───────────────────────────────────────────────

    #[test]
    fn output_order_returns_sorted_list() {
        let tables = parse(&output_order_dmn()).unwrap();
        let mut ctx = HashMap::new();
        ctx.insert("score".to_string(), json!(90)); // both rules match
        let result = evaluate(&tables[0], &ctx).unwrap();
        // "high" has priority 0, "medium" has priority 1 → order: high, medium
        assert_eq!(result["level"], json!(["high", "medium"]));
    }

    #[test]
    fn output_order_single_match() {
        let tables = parse(&output_order_dmn()).unwrap();
        let mut ctx = HashMap::new();
        ctx.insert("score".to_string(), json!(60)); // only rule 1 matches
        let result = evaluate(&tables[0], &ctx).unwrap();
        assert_eq!(result["level"], json!(["medium"]));
    }

    // ── output literals (tested via evaluate) ─────────────────────────────────

    #[test]
    fn output_literal_types() {
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="{ns}" id="d">
  <decision xmlns="{ns}" id="literals">
    <decisionTable id="dt1" hitPolicy="FIRST">
      <input id="i1"><inputExpression><text>key</text></inputExpression></input>
      <output id="o1" name="str_out"/>
      <output id="o2" name="num_out"/>
      <output id="o3" name="bool_out"/>
      <output id="o4" name="null_out"/>
      <rule id="r1">
        <inputEntry><text>-</text></inputEntry>
        <outputEntry><text>"hello"</text></outputEntry>
        <outputEntry><text>99</text></outputEntry>
        <outputEntry><text>true</text></outputEntry>
        <outputEntry><text>null</text></outputEntry>
      </rule>
    </decisionTable>
  </decision>
</definitions>"#,
            ns = DMN_NS
        );
        let tables = parse(&xml).unwrap();
        let ctx = HashMap::new();
        let result = evaluate(&tables[0], &ctx).unwrap();
        assert_eq!(result["str_out"], json!("hello"));
        assert_eq!(result["num_out"], json!(99));
        assert_eq!(result["bool_out"], json!(true));
        assert_eq!(result["null_out"], json!(null));
    }

    // ── output cell FEEL expressions ──────────────────────────────────────────

    #[test]
    fn output_cell_feel_expression() {
        let xml = format!(
            r#"<?xml version="1.0"?>
<definitions xmlns="{ns}" id="d">
  <decision xmlns="{ns}" id="d1">
    <decisionTable hitPolicy="FIRST">
      <input><inputExpression><text>amount</text></inputExpression></input>
      <output name="fee"/>
      <rule><inputEntry><text>-</text></inputEntry><outputEntry><text>amount * 0.1</text></outputEntry></rule>
    </decisionTable>
  </decision>
</definitions>"#,
            ns = DMN_NS
        );
        let tables = parse(&xml).unwrap();
        let mut ctx = HashMap::new();
        ctx.insert("amount".to_string(), json!(1000));
        let result = evaluate(&tables[0], &ctx).unwrap();
        assert_eq!(result["fee"], json!(100));
    }

    // ── serde round-trip ──────────────────────────────────────────────────────

    #[test]
    fn hit_policy_serializes_correctly() {
        assert_eq!(
            serde_json::to_string(&HitPolicy::Unique).unwrap(),
            r#""UNIQUE""#
        );
        assert_eq!(
            serde_json::to_string(&HitPolicy::RuleOrder).unwrap(),
            r#""RULE_ORDER""#
        );
        assert_eq!(
            serde_json::to_string(&HitPolicy::OutputOrder).unwrap(),
            r#""OUTPUT_ORDER""#
        );
        assert_eq!(
            serde_json::to_string(&HitPolicy::Any).unwrap(),
            r#""ANY""#
        );
        assert_eq!(
            serde_json::to_string(&HitPolicy::Priority).unwrap(),
            r#""PRIORITY""#
        );
    }

    #[test]
    fn collect_aggregator_serializes_correctly() {
        assert_eq!(
            serde_json::to_string(&CollectAggregator::Sum).unwrap(),
            r#""SUM""#
        );
        assert_eq!(
            serde_json::to_string(&CollectAggregator::Count).unwrap(),
            r#""COUNT""#
        );
    }

    #[test]
    fn decision_table_serializes_to_json() {
        let tables = parse(&priority_dmn()).unwrap();
        let j = serde_json::to_value(&tables[0]).unwrap();
        assert_eq!(j["hit_policy"], json!("PRIORITY"));
        assert_eq!(j["outputs"][0]["output_values"], json!(["high", "medium", "low"]));
    }
}
