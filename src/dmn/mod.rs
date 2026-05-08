//! DMN 1.5 decision-table support: types, parser, evaluator.
//!
//! Layered as `parser` (XML → `DecisionTable`) and `evaluator` (table + ctx →
//! outputs); both reuse the FEEL engines in `feel` and `feel_stdlib`. Types and
//! re-exports live here so callers can keep using `crate::dmn::{parse, evaluate, ...}`.

pub mod feel;
pub mod feel_stdlib;

mod evaluator;
mod parser;

pub use evaluator::evaluate;
pub use parser::parse;

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
        assert_eq!(tables[0].collect_aggregator, Some(CollectAggregator::Sum));
    }

    #[test]
    fn parse_collect_count_aggregator() {
        let tables = parse(&collect_count_dmn()).unwrap();
        assert_eq!(tables[0].collect_aggregator, Some(CollectAggregator::Count));
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
        let final_table = tables
            .iter()
            .find(|t| t.decision_key == "finalDecision")
            .unwrap();
        assert_eq!(final_table.required_decisions, vec!["subDecision"]);
        let sub_table = tables
            .iter()
            .find(|t| t.decision_key == "subDecision")
            .unwrap();
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
        assert_eq!(serde_json::to_string(&HitPolicy::Any).unwrap(), r#""ANY""#);
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
        assert_eq!(
            j["outputs"][0]["output_values"],
            json!(["high", "medium", "low"])
        );
    }
}
