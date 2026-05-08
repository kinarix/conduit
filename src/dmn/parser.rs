//! DMN XML → `DecisionTable` parsing.

use super::{
    feel, CollectAggregator, DecisionTable, HitPolicy, InputClause, OutputClause, Rule, DMN_NS,
};
use crate::error::EngineError;

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
