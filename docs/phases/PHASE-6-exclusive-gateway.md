# Phase 6 — Exclusive Gateway

## Status
Complete.

## Prerequisites
Phase 5.5 complete and all tests passing (77 tests).

## Summary

Processes can branch based on variable conditions. A token reaching an ExclusiveGateway evaluates conditions on each outgoing sequence flow and follows the first one that matches. This requires variables to be writable — currently `POST /tasks/:id/complete` takes no body, so variable passing is implemented here as the prerequisite.

---

## Tasks

### 6.0 — Variable passing on task completion

- Add `variables` field to `POST /api/v1/tasks/:id/complete` request body
- Write each variable to the `variables` table (upsert by `execution_id` + `name`) before advancing the token
- Variables are typed: `{ "name": "approved", "type": "boolean", "value": true }`
- Variables written at completion are scoped to the execution (root execution for flat processes)

### 6.1 — Parser

- Add `ExclusiveGateway` to `FlowNodeKind` in the parser
- Read `conditionExpression` from each outgoing `sequenceFlow`
- Mark one outgoing flow as the default (via `default` attribute on the gateway element)
- Validate: gateway must have at least one outgoing flow; default flow (if present) must exist

### 6.2 — Expression evaluator

- Integrate Rhai engine (already in `Cargo.toml` — confirm or add)
- `evaluate_condition(expression: &str, variables: &HashMap<String, Value>) -> Result<bool>`
- Variables are injected into the Rhai scope by name before evaluation
- Engine is sandboxed (no file I/O, no network, no `eval`)
- Expressions are short FEEL-like strings: `amount > 1000`, `approved == true`, `status == "gold"`

### 6.3 — Engine routing

- On entering an ExclusiveGateway: load current execution's variables from DB
- Evaluate each non-default outgoing flow's condition in order
- Follow the first flow whose condition evaluates to `true`
- If no condition matches and a default flow exists: follow the default
- If no condition matches and no default: mark instance as `error`, log the gateway ID

### 6.4 — Tests

- Variable round-trip: complete task with variables → variables readable from DB
- Route left: `amount > 1000` → true → token follows left flow
- Route right: `amount > 1000` → false → token follows right flow
- Default flow taken when no condition matches
- Error raised (instance state = `error`) when no condition matches and no default
- Nested exclusive gateways work correctly

---

## Acceptance Criteria

- `POST /tasks/:id/complete` with `{ "variables": [{ "name": "approved", "type": "boolean", "value": true }] }` writes variables and advances token
- A BPMN process with ExclusiveGateway routes correctly based on variables set at task completion
- All 77 prior tests still pass; new tests cover the cases above

---

## Checklist
- [x] Failing tests written
- [x] Implementation complete
- [x] All tests passing (this phase + all previous)
- [x] cargo clippy clean (no new warnings)
- [x] cargo fmt clean
- [x] CI green
- [x] Phase marked complete in PLAN.md

## Hardening (post-phase)

Three corrections to the original implementation:

1. **Strict expression-error semantics.** The original engine used
   `evaluate_condition(...).unwrap_or(false)` — a typo or undeclared variable
   silently routed to the default flow. Now eval failures mark the instance
   `state = 'error'` and log `flow_id`, `condition`, and the parser/runtime error.
   Same fix applied to InclusiveGateway. **Behaviour change:** any process that
   relied on a broken expression silently falling to default will now error.
2. **Validator: gateways must have ≥1 outgoing flow.** Both ExclusiveGateway
   and InclusiveGateway now reject zero-outgoing definitions at deploy time.
   Default flow IDs must also originate from the gateway they belong to.
3. **Full JSON variable type coverage.** Conditions can reference array
   length, object fields, and nested objects. Previously arrays/objects/null
   were silently dropped from the evaluator scope.

## Phase 6.1 — Migration from Rhai to FEEL (2026-05-01)

The original implementation used Rhai. In a follow-on we migrated the
gateway-condition language to **FEEL** (DMN 1.5) via `dsntk-feel-evaluator`.
Rationale and full evaluation: see `docs/adr/ADR-005-expression-evaluator.md`.

### What changed

- `src/engine/evaluator.rs` rewritten to use FEEL. Rhai dependency removed
  from `Cargo.toml`.
- Gateway BPMN test fixtures updated to FEEL syntax across
  `tests/engine_test.rs`, `tests/inclusive_gateway_test.rs`,
  `tests/subprocess_test.rs`.
- UI hint text and DocsDrawer (`ui/src/components/bpmn/BpmnProperties.tsx`)
  updated to show FEEL examples and a small cheat sheet.
- Strict eval-error semantics carry through cleanly — undefined variables
  produce FEEL `Null`, surfaced as `Err` (no silent default fallback).

### Syntax delta (for users authoring conditions)

| Before (Rhai)             | After (FEEL)               |
|---------------------------|----------------------------|
| `amount > 1000`           | `amount > 1000`            |
| `approved == true`        | `approved`                 |
| `tier == "gold"`          | `tier = "gold"`            |
| `x && y`                  | `x and y`                  |
| `x \|\| y`                | `x or y`                   |
| `!x`                      | `not(x)`                   |
| `items.len() >= 3`        | `count(items) >= 3`        |
| `customer.tier == "gold"` | `customer.tier = "gold"`   |

**Behaviour change:** any process definition persisted with Rhai-style
conditions (`==`, `&&`, `.len()`) will fail to parse after this migration.
Conduit is pre-prod; the change is acceptable but loud.
