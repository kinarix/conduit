# Phase 16 — Decision Table UI + Full FEEL

## Status
Not started — implementation plan in plan file.

## Prerequisites
Phase 15 complete and all tests passing.

## Goal
Decision tables can be created and edited visually in the Conduit UI without touching raw XML.
The mini-FEEL evaluator (Phase 14) is extended to cover the two most common patterns it
currently rejects: `not(...)` negation and `null` matching.

---

## Scope

### What is included

| Concern | Details |
|---|---|
| FEEL extension | `not(...)` negation, `null` literal, full standard library (`sum`, `count`, `min`, `max`, `date(...)`, `string length`, `substring`, `list contains`, etc.) |
| Hit policies | All DMN 1.5 hit policies: UNIQUE, FIRST, COLLECT (with SUM/MIN/MAX/COUNT aggregators), RULE_ORDER, OUTPUT ORDER, ANY, PRIORITY |
| Expression-based outputs | Output cells may contain FEEL expressions, not just literals |
| DRD | Decision Requirement Diagram — visual graph showing decision-to-decision dependencies; editor supports wiring inputs from other decisions |
| New API route | `GET /api/v1/decisions/:key` — returns full table structure as JSON |
| Frontend list page | `/decisions` — list all deployed decisions with DRD graph overview |
| Frontend editor | `/decisions/new` and `/decisions/:key/edit` — spreadsheet grid editor |
| DMN XML serializer | Pure TypeScript function; serializes editor state back to valid DMN XML |
| Round-trip | Edit → Save POSTs to existing `POST /api/v1/decisions`; no new deploy endpoint needed |

### What is explicitly excluded

- Real-time collaborative editing

---

## FEEL Extension

### Input entry extensions (`eval_single_entry` in `src/dmn/feel.rs`)

Add before the number-literal branch:

```rust
// not(...) negation — DMN 1.5 §10.3.2.7
if let Some(inner) = cell.strip_prefix("not(").and_then(|s| s.strip_suffix(')')) {
    return Ok(!eval_input_entry(inner.trim(), value)?);
}

// null literal — matches JSON null exactly
if cell == "null" {
    return Ok(value.is_null());
}
```

`not(...)` delegates recursively to `eval_input_entry`, so it composes with OR lists, ranges,
string literals, and comparisons.

### Full standard library (new module `src/dmn/feel_stdlib.rs`)

The mini-evaluator only covers unary input entries. A separate function `eval_feel_expression(expr: &str, ctx: &HashMap<String, JsonValue>) -> Result<JsonValue>` handles full FEEL for output cells and condition expressions:

| Category | Functions |
|---|---|
| Numeric | `decimal`, `floor`, `ceiling`, `abs`, `modulo`, `sqrt`, `log`, `exp`, `odd`, `even` |
| String | `string length`, `upper case`, `lower case`, `substring`, `string join`, `contains`, `starts with`, `ends with`, `matches`, `replace` |
| List | `list contains`, `count`, `min`, `max`, `sum`, `mean`, `all`, `any`, `sublist`, `append`, `concatenate`, `insert before`, `remove`, `reverse`, `index of`, `union`, `distinct values`, `flatten`, `product`, `median`, `mode` |
| Date/time | `date`, `time`, `date and time`, `duration`, `years and months duration`, `now`, `today` |
| Context | `get value`, `get entries`, `put`, `put all` |
| Conversion | `string`, `number`, `boolean` |

Implementation: delegate to `dsntk-feel-evaluator` (already a dependency) using `evaluate_expression` from `src/engine/evaluator.rs` — no new crate needed.

### Hit policy engine extension (`src/dmn/mod.rs`)

| Hit policy | Current | Change |
|---|---|---|
| UNIQUE | ✅ | — |
| FIRST | ✅ | — |
| COLLECT (no aggregator) | ✅ | — |
| COLLECT SUM/MIN/MAX/COUNT | ❌ | Add aggregator enum + evaluation |
| RULE_ORDER | ✅ | — |
| OUTPUT ORDER | ❌ | Sort output rows by output value priority list |
| ANY | ❌ | All matching rules must produce same output; return it |
| PRIORITY | ❌ | Return first match by output value priority list |

---

## New API Endpoint

`GET /api/v1/decisions/:key` — header `X-Org-Id: <uuid>`

Returns the latest version of the named decision table as structured JSON so the editor
can pre-populate from existing deployed tables:

```json
{
  "id": "uuid",
  "decision_key": "ageCategory",
  "version": 2,
  "name": "Age Category",
  "deployed_at": "2025-01-01T12:00:00Z",
  "table": {
    "hit_policy": "UNIQUE",
    "inputs": [{ "expression": "age" }],
    "outputs": [{ "name": "category" }],
    "rules": [
      { "input_entries": [">= 18"], "output_entries": ["\"adult\""] },
      { "input_entries": ["< 18"],  "output_entries": ["\"minor\""] }
    ]
  }
}
```

Requires `#[derive(serde::Serialize)]` on `DecisionTable`, `InputClause`, `OutputClause`,
`Rule`, and `HitPolicy` in `src/dmn/mod.rs`.

---

## Frontend Editor

The editor manages a local state object matching `DecisionDetail`. On save it serializes to
DMN XML and POSTs to `POST /api/v1/decisions`.

**Hit policy selector:** `<select>` with UNIQUE / FIRST / COLLECT / RULE_ORDER.

**Column management:** "+ Input" and "+ Output" header buttons. Column headers are editable
text inputs (FEEL expression for inputs, variable name for outputs). Columns can be removed.

**Rules grid:** Each row maps to one `<rule>`. Input cells accept raw FEEL entry text
(`-`, `>= 18`, `"gold","silver"`, `not("inactive")`, `[500..999]`, etc.). Output cells
accept literal values (`"adult"`, `42`, `true`, `null`). "+ Rule" appends a blank row.
Rules can be reordered via drag handle or deleted.

**Save:** Validates that every column has a non-empty header. Serializes state to DMN XML.
POSTs to `/api/v1/decisions` with the active org's `X-Org-Id` header. On success, navigates
to `/decisions`.

---

## Tasks

### Backend

- [ ] `src/dmn/feel.rs` — add `not(...)` and `null` branches; add 5 unit tests
- [ ] `src/dmn/feel_stdlib.rs` — full FEEL standard library (`sum`, `count`, `min`, `max`, `date(...)`, string/list/context functions) via `dsntk-feel-evaluator`; used for output cell and DRD expression evaluation
- [ ] `src/dmn/mod.rs` — `#[derive(serde::Serialize)]` on DMN types; add COLLECT aggregator (SUM/MIN/MAX/COUNT), OUTPUT ORDER, ANY, PRIORITY hit policies; add DRD dependency tracking (`required_decisions: Vec<String>` on `DecisionTable`)
- [ ] `src/db/decision_definitions.rs` — add `get_latest(pool, org_id, key)` query; add `list_all(pool, org_id)` for DRD graph
- [ ] `src/api/decisions.rs` — add `GET /api/v1/decisions/:key` route + handler

### Frontend

- [ ] `ui/src/api/decisions.ts` — TypeScript API client (fetchDecisions, fetchDecision, deployDecision)
- [ ] `ui/src/pages/Decisions.tsx` — list page + DRD overview graph (ReactFlow)
- [ ] `ui/src/pages/DecisionTableEditor.tsx` — grid editor + DMN XML serializer; all hit policies, COLLECT aggregators, expression-based output cells, DRD dependency wiring
- [ ] `ui/src/App.tsx` — add routes `/decisions`, `/decisions/new`, `/decisions/:key/edit`
- [ ] `ui/src/components/Sidebar/FooterNav.tsx` — add Decisions nav link

---

## Tests

### FEEL unit tests (add to `src/dmn/feel.rs`)

```
not("inactive") vs "active"        → true
not("inactive") vs "inactive"      → false
not(>= 18)      vs 15             → true
not(>= 18)      vs 20             → false
null            vs Value::Null    → true
null            vs json!(0)       → false
```

### API tests (backend)

- `GET /api/v1/decisions/:key` returns 200 with `table` object after a deploy
- `GET /api/v1/decisions/missing` returns 404
- Org isolation: key from org A not visible to org B

---

## Critical Files

| File | Change |
|---|---|
| `src/dmn/feel.rs` | Add `not(...)` + `null` + tests |
| `src/dmn/feel_stdlib.rs` | New — full FEEL standard library via dsntk-feel-evaluator |
| `src/dmn/mod.rs` | `#[derive(serde::Serialize)]`, all hit policies, DRD dependency tracking |
| `src/db/decision_definitions.rs` | Add `get_latest` + `list_all` functions |
| `src/api/decisions.rs` | Add `GET /api/v1/decisions/:key` route |
| `ui/src/api/decisions.ts` | New — TypeScript API client |
| `ui/src/pages/Decisions.tsx` | New — list page + DRD graph |
| `ui/src/pages/DecisionTableEditor.tsx` | New — grid editor + all hit policies + DRD wiring |
| `ui/src/App.tsx` | Add 3 new routes |
| `ui/src/components/Sidebar/FooterNav.tsx` | Add Decisions nav link |

---

## Verification

```bash
# 1. FEEL extension
make test   # new feel tests must pass; all existing tests stay green
make check

# 2. API smoke test
curl -X POST http://localhost:8080/api/v1/decisions \
  -H "X-Org-Id: <uuid>" -H "Content-Type: text/xml" \
  --data-binary @tests/fixtures/age_category.dmn

curl http://localhost:8080/api/v1/decisions/ageCategory \
  -H "X-Org-Id: <uuid>"
# → JSON with hit_policy, inputs, outputs, rules

# 3. UI
# Open /decisions — list shows deployed decisions
# Click "New Decision" → blank editor loads
# Add inputs/outputs/rules → Save → appears in list
# Click Edit on existing → grid pre-populated from API
```

## Checklist
- [ ] Failing tests written
- [ ] Implementation complete
- [ ] All tests passing (this phase + all previous)
- [ ] cargo clippy clean
- [ ] cargo fmt clean
- [ ] CI green
- [ ] Phase marked complete in PLAN.md
