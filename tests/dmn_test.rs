use conduit::dmn::feel;
use conduit::dmn::{self, HitPolicy};
use serde_json::json;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!("tests/fixtures/dmn/{name}.dmn"))
        .unwrap_or_else(|_| panic!("DMN fixture not found: {name}.dmn"))
}

// ── Parser ────────────────────────────────────────────────────────────────────

#[test]
fn parse_single_decision() {
    let tables = dmn::parse(&fixture("risk_check")).unwrap();
    assert_eq!(tables.len(), 1);

    let t = &tables[0];
    assert_eq!(t.decision_key, "risk-check");
    assert_eq!(t.name.as_deref(), Some("Risk Check"));
    assert!(matches!(t.hit_policy, HitPolicy::Unique));
    assert_eq!(t.inputs.len(), 2);
    assert_eq!(t.outputs.len(), 1);
    assert_eq!(t.rules.len(), 4);

    assert_eq!(t.inputs[0].expression, "age");
    assert_eq!(t.inputs[1].expression, "credit_score");
    assert_eq!(t.outputs[0].name, "risk_level");
}

#[test]
fn parse_multi_decision() {
    let tables = dmn::parse(&fixture("multi_decision")).unwrap();
    assert_eq!(tables.len(), 2);

    let keys: Vec<&str> = tables.iter().map(|t| t.decision_key.as_str()).collect();
    assert!(keys.contains(&"decision-a"));
    assert!(keys.contains(&"decision-b"));
}

#[test]
fn parse_hit_policy_first() {
    let tables = dmn::parse(&fixture("fee_tiers")).unwrap();
    assert!(matches!(tables[0].hit_policy, HitPolicy::First));
}

#[test]
fn parse_hit_policy_collect() {
    let tables = dmn::parse(&fixture("collect_flags")).unwrap();
    assert!(matches!(tables[0].hit_policy, HitPolicy::Collect));
}

#[test]
fn parse_missing_decision_table() {
    let xml = r#"<?xml version="1.0"?>
<definitions xmlns="https://www.omg.org/spec/DMN/20191111/MODEL/" id="d">
</definitions>"#;
    let result = dmn::parse(xml);
    assert!(result.is_err(), "expected error for empty definitions");
}

#[test]
fn parse_invalid_xml() {
    let result = dmn::parse("<not valid <<xml");
    assert!(result.is_err());
}

// ── FEEL evaluator ────────────────────────────────────────────────────────────

#[test]
fn feel_wildcard_matches_number() {
    assert!(feel::eval_input_entry("-", &json!(42)).unwrap());
}

#[test]
fn feel_wildcard_matches_string() {
    assert!(feel::eval_input_entry("-", &json!("hello")).unwrap());
}

#[test]
fn feel_wildcard_matches_null() {
    assert!(feel::eval_input_entry("-", &json!(null)).unwrap());
}

#[test]
fn feel_string_literal_match() {
    assert!(feel::eval_input_entry(r#""low""#, &json!("low")).unwrap());
}

#[test]
fn feel_string_literal_no_match() {
    assert!(!feel::eval_input_entry(r#""low""#, &json!("high")).unwrap());
}

#[test]
fn feel_number_literal_match() {
    assert!(feel::eval_input_entry("42", &json!(42)).unwrap());
}

#[test]
fn feel_number_literal_no_match() {
    assert!(!feel::eval_input_entry("42", &json!(43)).unwrap());
}

#[test]
fn feel_bool_literal_true() {
    assert!(feel::eval_input_entry("true", &json!(true)).unwrap());
    assert!(!feel::eval_input_entry("true", &json!(false)).unwrap());
}

#[test]
fn feel_unary_gte_matches() {
    assert!(feel::eval_input_entry(">= 18", &json!(18)).unwrap());
    assert!(feel::eval_input_entry(">= 18", &json!(100)).unwrap());
    assert!(!feel::eval_input_entry(">= 18", &json!(17)).unwrap());
}

#[test]
fn feel_unary_gt_matches() {
    assert!(feel::eval_input_entry("> 18", &json!(19)).unwrap());
    assert!(!feel::eval_input_entry("> 18", &json!(18)).unwrap());
}

#[test]
fn feel_unary_lte_matches() {
    assert!(feel::eval_input_entry("<= 100", &json!(100)).unwrap());
    assert!(feel::eval_input_entry("<= 100", &json!(0)).unwrap());
    assert!(!feel::eval_input_entry("<= 100", &json!(101)).unwrap());
}

#[test]
fn feel_unary_lt_matches() {
    assert!(feel::eval_input_entry("< 500", &json!(499)).unwrap());
    assert!(!feel::eval_input_entry("< 500", &json!(500)).unwrap());
}

#[test]
fn feel_unary_eq_string() {
    assert!(feel::eval_input_entry(r#"= "A""#, &json!("A")).unwrap());
    assert!(!feel::eval_input_entry(r#"= "A""#, &json!("B")).unwrap());
}

#[test]
fn feel_unary_neq() {
    assert!(feel::eval_input_entry("!= 0", &json!(1)).unwrap());
    assert!(!feel::eval_input_entry("!= 0", &json!(0)).unwrap());
}

#[test]
fn feel_range_inclusive_both() {
    assert!(feel::eval_input_entry("[1..10]", &json!(1)).unwrap());
    assert!(feel::eval_input_entry("[1..10]", &json!(5)).unwrap());
    assert!(feel::eval_input_entry("[1..10]", &json!(10)).unwrap());
    assert!(!feel::eval_input_entry("[1..10]", &json!(0)).unwrap());
    assert!(!feel::eval_input_entry("[1..10]", &json!(11)).unwrap());
}

#[test]
fn feel_range_exclusive_both() {
    assert!(feel::eval_input_entry("(1..10)", &json!(2)).unwrap());
    assert!(feel::eval_input_entry("(1..10)", &json!(9)).unwrap());
    assert!(!feel::eval_input_entry("(1..10)", &json!(1)).unwrap());
    assert!(!feel::eval_input_entry("(1..10)", &json!(10)).unwrap());
}

#[test]
fn feel_range_inclusive_exclusive() {
    assert!(feel::eval_input_entry("[1..10)", &json!(1)).unwrap());
    assert!(!feel::eval_input_entry("[1..10)", &json!(10)).unwrap());
    assert!(feel::eval_input_entry("[1..10)", &json!(9)).unwrap());
}

#[test]
fn feel_range_exclusive_inclusive() {
    assert!(!feel::eval_input_entry("(1..10]", &json!(1)).unwrap());
    assert!(feel::eval_input_entry("(1..10]", &json!(10)).unwrap());
    assert!(feel::eval_input_entry("(1..10]", &json!(2)).unwrap());
}

#[test]
fn feel_or_list_matches_any() {
    assert!(feel::eval_input_entry(r#""A","B""#, &json!("A")).unwrap());
    assert!(feel::eval_input_entry(r#""A","B""#, &json!("B")).unwrap());
    assert!(!feel::eval_input_entry(r#""A","B""#, &json!("C")).unwrap());
}

#[test]
fn feel_or_list_with_numbers() {
    assert!(feel::eval_input_entry("1,2,3", &json!(2)).unwrap());
    assert!(!feel::eval_input_entry("1,2,3", &json!(4)).unwrap());
}

#[test]
fn feel_invalid_cell_returns_error() {
    let result = feel::eval_input_entry("??garbage??", &json!(42));
    assert!(result.is_err(), "expected DmnFeel error for malformed cell");
}

// ── Decision table evaluation ─────────────────────────────────────────────────

fn make_context(
    pairs: &[(&str, serde_json::Value)],
) -> std::collections::HashMap<String, serde_json::Value> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect()
}

#[test]
fn evaluate_unique_low_risk() {
    let tables = dmn::parse(&fixture("risk_check")).unwrap();
    let ctx = make_context(&[("age", json!(25)), ("credit_score", json!(750))]);
    let result = dmn::evaluate(&tables[0], &ctx).unwrap();
    assert_eq!(result.get("risk_level"), Some(&json!("low")));
}

#[test]
fn evaluate_unique_medium_risk() {
    let tables = dmn::parse(&fixture("risk_check")).unwrap();
    let ctx = make_context(&[("age", json!(30)), ("credit_score", json!(600))]);
    let result = dmn::evaluate(&tables[0], &ctx).unwrap();
    assert_eq!(result.get("risk_level"), Some(&json!("medium")));
}

#[test]
fn evaluate_unique_rejected() {
    let tables = dmn::parse(&fixture("risk_check")).unwrap();
    let ctx = make_context(&[("age", json!(16)), ("credit_score", json!(800))]);
    let result = dmn::evaluate(&tables[0], &ctx).unwrap();
    assert_eq!(result.get("risk_level"), Some(&json!("rejected")));
}

#[test]
fn evaluate_unique_multiple_matches_error() {
    // Inline table with overlapping rules so x=7 fires both
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="https://www.omg.org/spec/DMN/20191111/MODEL/" id="test" name="Test">
  <decision id="overlapping" name="Overlapping">
    <decisionTable hitPolicy="UNIQUE">
      <input id="i1" label="X">
        <inputExpression><text>x</text></inputExpression>
      </input>
      <output id="o1" name="result" label="Result"/>
      <rule id="r1">
        <inputEntry><text>&gt;= 5</text></inputEntry>
        <outputEntry><text>"high"</text></outputEntry>
      </rule>
      <rule id="r2">
        <inputEntry><text>&lt;= 10</text></inputEntry>
        <outputEntry><text>"low"</text></outputEntry>
      </rule>
    </decisionTable>
  </decision>
</definitions>"#;
    let tables = dmn::parse(xml).unwrap();
    let ctx = make_context(&[("x", json!(7))]);
    let result = dmn::evaluate(&tables[0], &ctx);
    assert!(
        result.is_err(),
        "expected DmnMultipleMatches error for UNIQUE policy with two matching rules"
    );
}

#[test]
fn evaluate_unique_no_match_error() {
    let tables = dmn::parse(&fixture("risk_check")).unwrap();
    // age = 25, score = 800 → matches rule r1 only (>= 18, >= 700); should be low
    // Construct a scenario that matches no rule by using a missing variable → value is null
    let ctx = make_context(&[("age", json!(25))]);
    // credit_score is missing → null; no rule covers null credit_score explicitly
    let result = dmn::evaluate(&tables[0], &ctx);
    // null doesn't match >= 700, [500..699], or < 500, so no rule fires
    assert!(result.is_err(), "expected DmnNoMatch");
}

#[test]
fn evaluate_first_large_amount() {
    let tables = dmn::parse(&fixture("fee_tiers")).unwrap();
    let ctx = make_context(&[("amount", json!(50000))]);
    let result = dmn::evaluate(&tables[0], &ctx).unwrap();
    assert_eq!(result.get("fee_percent"), Some(&json!(1.0)));
}

#[test]
fn evaluate_first_medium_amount() {
    let tables = dmn::parse(&fixture("fee_tiers")).unwrap();
    let ctx = make_context(&[("amount", json!(5000))]);
    let result = dmn::evaluate(&tables[0], &ctx).unwrap();
    assert_eq!(result.get("fee_percent"), Some(&json!(1.5)));
}

#[test]
fn evaluate_first_fallback() {
    let tables = dmn::parse(&fixture("fee_tiers")).unwrap();
    let ctx = make_context(&[("amount", json!(100))]);
    let result = dmn::evaluate(&tables[0], &ctx).unwrap();
    assert_eq!(result.get("fee_percent"), Some(&json!(2.0)));
}

#[test]
fn evaluate_collect_enterprise() {
    let tables = dmn::parse(&fixture("collect_flags")).unwrap();
    let ctx = make_context(&[("category", json!("enterprise"))]);
    let result = dmn::evaluate(&tables[0], &ctx).unwrap();
    let flags = result.get("flag").unwrap().as_array().unwrap();
    // enterprise matches r1 ("premium","enterprise"), r2 ("enterprise"), and r3 (-) = 3 flags
    assert_eq!(flags.len(), 3);
    assert!(flags.contains(&json!("priority-support")));
    assert!(flags.contains(&json!("dedicated-account-manager")));
    assert!(flags.contains(&json!("standard-support")));
}

#[test]
fn evaluate_collect_premium() {
    let tables = dmn::parse(&fixture("collect_flags")).unwrap();
    let ctx = make_context(&[("category", json!("premium"))]);
    let result = dmn::evaluate(&tables[0], &ctx).unwrap();
    let flags = result.get("flag").unwrap().as_array().unwrap();
    // premium matches r1 and r3 = 2 flags
    assert_eq!(flags.len(), 2);
    assert!(flags.contains(&json!("priority-support")));
    assert!(flags.contains(&json!("standard-support")));
}
