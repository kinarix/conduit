# ADR-005: Expression Evaluator — FEEL via dsntk

## Status
Accepted (revised 2026-05-01).

This ADR supersedes the previous "Rhai" decision. Rhai shipped in Phase 6 as the
gateway-condition evaluator; in Phase 6.1 we migrated to FEEL. The Rhai-era
content is preserved at the bottom under "History" for context.

## Context

Sequence-flow conditions on Exclusive and Inclusive gateways need an embedded
expression evaluator. Expressions reference process variables and must return a
boolean. The evaluator must be sandboxed — expressions come from user-supplied
BPMN files.

Beyond Phase 6 we have additional pressures:

- **DMN integration (Phase 14)** already uses a mini-FEEL for input cells in
  `src/dmn/feel.rs`. Having two expression languages in the same engine
  (Rhai for gateways, FEEL-subset for DMN) is a usability and maintenance tax.
- **BPMN/DMN spec alignment.** FEEL is the language the OMG specifies for
  conditions. Camunda 8 / Zeebe — the modern incumbent we're competing with —
  uses FEEL natively.
- **User authoring.** Users moving from Camunda expect to paste FEEL into the
  condition editor and have it work.

## Decision

Use **FEEL (Friendly Enough Expression Language)** as the gateway-condition
language, via the **dsntk** suite of crates (`dsntk-feel`,
`dsntk-feel-parser`, `dsntk-feel-evaluator`).

## Candidates Evaluated (revised)

| Option | Pros | Cons |
|---|---|---|
| **dsntk-feel-evaluator** | Active Rust DMN/FEEL implementation, dual MIT/Apache-2.0, full FEEL grammar incl. `count()`, ranges, path access, 1-based list indexing, decimal numbers | Newish (0.x), ~16 transitive crates from same project |
| Rhai (Phase 6) | Sandboxed, fast, Rust-native | Not FEEL — `==` vs `=`, `&&` vs `and`, `.len()` vs `count()`. Drift from BPMN/DMN spec; second expression language alongside DMN's mini-FEEL |
| Boa (JS) | FEEL-like syntax | Heavy, full JS surface area is overkill |
| Hand-rolled FEEL | Full control | Months of work; the DMN spec FEEL grammar is non-trivial |
| Extend `src/dmn/feel.rs` to full FEEL | Reuse | Same problem — months of work |

## Rationale

- **Spec alignment.** FEEL is what BPMN/DMN specify; Camunda 8 / Zeebe use it.
- **Single expression language.** Conditions, DMN cells, and (future) script
  tasks can all sit on one evaluator. The mini-FEEL in `src/dmn/feel.rs` is
  earmarked for migration onto dsntk in a later pass.
- **Sandbox.** dsntk is a pure expression evaluator — no I/O, no system calls.
  Same safety posture as Rhai.
- **Strict semantics.** Undefined variables produce `Value::Null`, which our
  wrapper surfaces as `Err`. This preserves the gateway "fail loudly" behaviour
  hardened in Phase 6 (no silent fallback to default on a typo'd condition).
- **Effort.** The dsntk migration was small: ~250 LOC of evaluator code +
  test rewrites. A hand-rolled FEEL would be ~10× that.

## Spike (executed 2026-05-01)

12 unit cases written against `dsntk-feel-evaluator` covered:
numeric / float comparison, string equality, boolean direct reference,
compound `and`/`or`, nested object path access (`customer.tier = "gold"`),
array `count()`, 1-based indexing (`scores[1] = 85`), malformed-expression
error, undefined-variable error, `not(...)` negation. All passed first try.

## Consequences

- Gateway conditions use FEEL: `=` (not `==`), `and`/`or`/`not(...)`
  (not `&&` / `||` / `!`), `count(list)` (not `list.len()`), 1-based list
  indices.
- The `rhai` dependency is removed from `Cargo.toml`.
- Persisted process definitions written under Phase 6 (using Rhai syntax) need
  one-time syntax migration. Conduit is pre-prod; this is acceptable.
- DMN's mini-FEEL evaluator (`src/dmn/feel.rs`) remains for now — its scope
  is single input-cell tests, which it handles cleanly. Future work: replace
  it with dsntk for full unification.
- UI condition editor (`ui/src/components/bpmn/BpmnProperties.tsx`) shows FEEL
  examples and a small cheat sheet.

## History

Phase 6 originally chose Rhai for the reasons above (small, sandboxed, fast).
That choice shipped and was hardened (strict eval-error semantics, full JSON
type coverage in scope) before being revisited. Migration to FEEL was driven
by spec alignment and the decision to unify with DMN's expression surface.
