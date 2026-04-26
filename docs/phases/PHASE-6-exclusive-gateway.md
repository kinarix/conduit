# Phase 6 — Exclusive Gateway

## Status
Not started

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
- [ ] Failing tests written
- [ ] Implementation complete
- [ ] All tests passing (this phase + all previous)
- [ ] cargo clippy clean
- [ ] cargo fmt clean
- [ ] CI green
- [ ] Phase marked complete in PLAN.md
