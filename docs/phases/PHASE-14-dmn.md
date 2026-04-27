# Phase 14 — DMN Integration

## Status
Not started

## Prerequisites
Phase 13 complete and all tests passing.

## Goal
Decision tables can be deployed separately from BPMN and evaluated synchronously from
`BusinessRuleTask` elements. Output columns become process variables.

---

## Scope

### What is included
| Concern | Details |
|---|---|
| DMN elements | `<definitions>` → `<decision>` → `<decisionTable>` only (no DRD) |
| Hit policies | UNIQUE (default, error if >1 match), FIRST (first match wins), COLLECT (all matching rows as list), RULE_ORDER (matching rows in declaration order) |
| Input conditions (FEEL subset) | `-`, literals, unary comparisons, ranges, comma-separated OR |
| Output entries | Scalar literals only (string, number, boolean, null) |
| BPMN integration | `BusinessRuleTask` with `camunda:decisionRef` attribute |
| Deployment | `POST /api/v1/decisions` — raw DMN XML body |
| Versioning | Auto-increment per `(org_id, decision_key)`; engine always uses latest version |
| Multi-decision files | One DMN file may contain multiple `<decision>` elements; each stored as a separate row |

### What is explicitly excluded
- DRD — decision-to-decision dependencies
- FEEL standard library functions (`sum(list)`, `count(list)`, `date("2025-01-01")`, etc.)
- COLLECT aggregation (SUM, MIN, MAX, COUNT)
- OUTPUT ORDER, ANY, PRIORITY hit policies
- Stateless DMN evaluation API (no `/api/v1/decisions/{id}/evaluate`)
- Expression-based outputs (output cells are literals only)

---

## FEEL Subset (input entry cells)

The engine evaluates each input cell against the corresponding input variable using this grammar:

```
cell      = "-"                 # always matches (wildcard)
          | or_list             # "A","B" matches if any entry matches
          | entry

or_list   = entry ("," entry)+

entry     = string_lit          # "foo" — exact string equality
          | number_lit          # 42 — exact number equality
          | bool_lit            # true / false — exact boolean equality
          | unary_cmp           # >= 18  /  < 100  /  = "A"  /  != false
          | range               # [1..10]  (1..10)  [1..10)  (1..10]

unary_cmp = op (string_lit | number_lit | bool_lit)
op        = ">=" | ">" | "<=" | "<" | "=" | "!="

range     = ("[" | "(") number_lit ".." number_lit ("]" | ")")
            # "[" = inclusive, "(" = exclusive on that end
```

**Type coercion**: the input variable value (from process variables JSONB) is compared after coercion — numbers compared numerically, strings case-sensitively, booleans exactly.

**Null / missing variables**: if an input variable is absent from the context its value is `null`. A null value compared against any numeric or string comparator (unary, range) evaluates to `false` — it does not error. Only wildcard (`-`) matches null.

Output entry cells are plain literals: `"string"`, `42`, `3.14`, `true`, `false`, or empty (null).

---

## Data Model

### New table: `decision_definitions`

```sql
CREATE TABLE decision_definitions (
    id            UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    org_id        UUID NOT NULL REFERENCES orgs(id),
    decision_key  TEXT NOT NULL,
    version       INT  NOT NULL DEFAULT 1,
    name          TEXT,
    dmn_xml       TEXT NOT NULL,
    deployed_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (org_id, decision_key, version)
);
CREATE INDEX idx_decision_definitions_key ON decision_definitions (org_id, decision_key, version DESC);
```

---

## Source Layout

### New modules
| Path | Purpose |
|---|---|
| `src/dmn/mod.rs` | DMN XML parser — `parse(xml: &str) -> Result<Vec<DecisionTable>>` |
| `src/dmn/feel.rs` | Mini FEEL evaluator — `eval_input_entry(cell: &str, value: &serde_json::Value) -> Result<bool>` |
| `src/db/decision_definitions.rs` | DB CRUD: deploy, get_latest, list |
| `src/api/decisions.rs` | HTTP handlers: `POST /api/v1/decisions`, `GET /api/v1/decisions` |

### Modified files
| Path | Change |
|---|---|
| `src/parser/mod.rs` | Add `BusinessRuleTask { decision_ref: String }` variant; remove from unsupported list |
| `src/engine/mod.rs` | New match arm: load decision → evaluate → write output variables → advance |
| `src/lib.rs` | `pub mod dmn;` |
| `src/db/mod.rs` | `pub mod decision_definitions;` |
| `src/db/models.rs` | Add `DecisionDefinition` struct |
| `src/api/mod.rs` | Register `/api/v1/decisions` routes |
| `src/api/instances.rs` | Add `variables: Option<Vec<VariableInput>>` to `StartInstanceRequest`; write them at start |
| `migrations/` | `006_decision_definitions.sql` |

---

## DMN Structs (in `src/dmn/mod.rs`)

```rust
pub struct DecisionTable {
    pub decision_key: String,          // <decision id="...">
    pub name: Option<String>,          // <decision name="...">
    pub hit_policy: HitPolicy,
    pub inputs: Vec<InputClause>,
    pub outputs: Vec<OutputClause>,
    pub rules: Vec<Rule>,
}

pub enum HitPolicy {
    Unique,       // default — error if > 1 rule matches
    First,        // first matching rule wins
    Collect,      // all matching rules, outputs as Vec<serde_json::Value>
    RuleOrder,    // matching rules in declaration order (same as Collect, no dedup)
}

pub struct InputClause {
    pub label: Option<String>,         // <input label="...">
    pub expression: String,            // <inputExpression><text>age</text></inputExpression>
}

pub struct OutputClause {
    pub name: String,                  // <output name="risk">
    pub label: Option<String>,
}

pub struct Rule {
    pub id: Option<String>,
    pub entries: Vec<String>,          // one per input column
    pub outputs: Vec<String>,          // one per output column (literal text)
}
```

---

## Engine Evaluation Flow

When the token engine encounters a `BusinessRuleTask { decision_ref }`:

1. Load latest `DecisionDefinition` for `(instance.org_id, decision_ref)` — error if not found.
2. Parse `dmn_xml` with `dmn::parse()`.
3. Collect input values from process variables (one per `InputClause.expression`).
4. Evaluate each `Rule`: for each input clause, call `feel::eval_input_entry(cell, value)`. A rule matches only if **all** input cells match.
5. Apply hit policy:
   - `UNIQUE` — exactly one match required; zero → `DmnNoMatch` error; two or more → `DmnMultipleMatches` error.
   - `FIRST` — return first matching rule; zero matches → `DmnNoMatch` error.
   - `COLLECT` / `RULE_ORDER` — return all matching rules (empty Vec is not an error).
6. Write outputs as process variables:
   - Single-row result (UNIQUE / FIRST): each output column → one variable, value parsed from literal.
   - Multi-row result (COLLECT / RULE_ORDER): each output column → one variable whose value is a JSON array.
7. Advance the token to the next node.

### New error variants (in `src/error.rs`)

```rust
DmnNoMatch(String),           // "decision {key}: no rule matched"
DmnMultipleMatches(String),   // "decision {key}: hit policy UNIQUE but N rules matched"
DmnParse(String),             // DMN XML parse failure
DmnFeel(String),              // FEEL cell evaluation error
DmnNotFound(String),          // "decision {key}: no definition deployed"
```

---

## API

### POST /api/v1/decisions
Deploy a DMN file (may contain multiple `<decision>` elements).

**Request headers**: `Content-Type: application/xml` or `text/xml`
**Request body**: raw DMN XML

**Response 201**:
```json
{
  "deployed": [
    { "id": "<uuid>", "decision_key": "risk-check", "version": 1 },
    { "id": "<uuid>", "decision_key": "fee-calc",   "version": 1 }
  ]
}
```

**Behaviour**: for each `<decision>` element found, determine the next version number for `(org_id, decision_key)` and insert a row. All inserts in one transaction.

**Auth/org**: `org_id` taken from `X-Org-Id` header (same pattern as existing endpoints).

### GET /api/v1/decisions
List latest version of every deployed decision for the org.

**Response 200**:
```json
[
  { "id": "<uuid>", "decision_key": "risk-check", "version": 2, "name": "Risk Check", "deployed_at": "..." }
]
```

---

## BPMN — BusinessRuleTask

```xml
<businessRuleTask id="task_risk"
                  name="Assess Risk"
                  camunda:decisionRef="risk-check" />
```

Parser: add `BusinessRuleTask { decision_ref: String }` to `FlowNodeKind`. Extract
`decision_ref` from `camunda:decisionRef` attribute (same Camunda namespace already in parser).
Remove `"businessRuleTask"` from the unsupported-elements rejection list.

---

## Fixture Files

### `tests/fixtures/dmn/risk_check.dmn`
Single decision, two input columns, one output, UNIQUE hit policy.

```xml
<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="https://www.omg.org/spec/DMN/20191111/MODEL/"
             id="risk_check_defs" name="Risk Check">
  <decision id="risk-check" name="Risk Check">
    <decisionTable hitPolicy="UNIQUE">
      <input id="i_age"    label="Age">
        <inputExpression><text>age</text></inputExpression>
      </input>
      <input id="i_score"  label="Credit Score">
        <inputExpression><text>credit_score</text></inputExpression>
      </input>
      <output id="o_risk"  name="risk_level" label="Risk Level"/>
      <rule><inputEntry><text>>= 18</text></inputEntry><inputEntry><text>>= 700</text></inputEntry><outputEntry><text>"low"</text></outputEntry></rule>
      <rule><inputEntry><text>>= 18</text></inputEntry><inputEntry><text>[500..699]</text></inputEntry><outputEntry><text>"medium"</text></outputEntry></rule>
      <rule><inputEntry><text>>= 18</text></inputEntry><inputEntry><text>< 500</text></inputEntry><outputEntry><text>"high"</text></outputEntry></rule>
      <rule><inputEntry><text>< 18</text></inputEntry><inputEntry><text>-</text></inputEntry><outputEntry><text>"rejected"</text></outputEntry></rule>
    </decisionTable>
  </decision>
</definitions>
```

### `tests/fixtures/dmn/fee_tiers.dmn`
FIRST hit policy with a wildcard fallback rule.

### `tests/fixtures/dmn/collect_flags.dmn`
COLLECT hit policy, single input, single output.

### `tests/fixtures/dmn/multi_decision.dmn`
Two `<decision>` elements in one file.

### `tests/fixtures/bpmn/business_rule_task.bpmn`
Process: start → BusinessRuleTask(risk-check) → end.

---

## Tests

### `tests/dmn_test.rs` — unit tests (no DB)

| Test | Description |
|---|---|
| `parse_single_decision` | Parse `risk_check.dmn` — correct hit policy, 2 inputs, 1 output, 4 rules |
| `parse_multi_decision` | Parse `multi_decision.dmn` — returns Vec of length 2 |
| `parse_missing_decision` | Empty `<definitions>` → `DmnParse` error |
| `feel_wildcard_matches_any` | `-` matches number, string, null |
| `feel_string_literal` | `"low"` matches `"low"`, does not match `"high"` |
| `feel_number_literal` | `42` matches `42`, fails `43` |
| `feel_unary_gte` | `>= 18` matches 18, 100; fails 17 |
| `feel_unary_lt` | `< 500` matches 499; fails 500, 501 |
| `feel_range_inclusive` | `[1..10]` matches 1, 5, 10; fails 0, 11 |
| `feel_range_exclusive` | `(1..10)` matches 2, 9; fails 1, 10 |
| `feel_mixed_range` | `[1..10)` matches 1, 9; fails 0, 10 |
| `feel_or_list` | `"A","B"` matches "A", matches "B", fails "C" |
| `feel_invalid_cell` | Malformed cell text → `DmnFeel` error |
| `evaluate_unique_low_risk` | age=25, score=750 → risk_level="low" |
| `evaluate_unique_rejected` | age=16, score=800 → risk_level="rejected" |
| `evaluate_unique_no_match` | age=25, score=750 but no rule covers → `DmnNoMatch` |
| `evaluate_unique_multiple_matches` | overlapping rules → `DmnMultipleMatches` |
| `evaluate_first_hit_policy` | first matching rule wins |
| `evaluate_collect_hit_policy` | all matching rows returned as list |

### `tests/decision_test.rs` — integration tests (real DB)

| Test | Description |
|---|---|
| `deploy_single_decision` | POST DMN → 201, version=1 |
| `deploy_increments_version` | POST same decision_key twice → versions 1 and 2 |
| `deploy_multi_decision_file` | POST file with 2 decisions → 201 with 2 entries |
| `list_decisions` | GET /api/v1/decisions returns latest versions |
| `engine_runs_business_rule_task` | Full integration: deploy DMN + BPMN, start instance, engine evaluates BusinessRuleTask, output variable written |
| `engine_decision_not_found` | BusinessRuleTask with unknown ref → instance in error state |
| `engine_dmn_no_match_error` | Inputs match no rule with UNIQUE policy → instance in error state |
| `parser_accepts_business_rule_task` | BPMN parser extracts `decision_ref` from `camunda:decisionRef` |

---

## Checklist
- [ ] Failing tests written
- [ ] Implementation complete
- [ ] All tests passing (this phase + all previous)
- [ ] cargo clippy clean
- [ ] cargo fmt clean
- [ ] CI green
- [ ] Phase marked complete in PLAN.md
