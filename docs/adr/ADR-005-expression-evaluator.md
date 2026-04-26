# ADR-005: Expression Evaluator — Rhai

## Status
Proposed (pending Phase 0 spike)

## Context
Gateway conditions, sequence flow conditions, and script tasks need an expression
evaluator. Expressions must access process variables and return boolean or value results.

Security is critical — expressions come from user-provided BPMN files and must be
fully sandboxed (no file system access, no network, no system calls).

## Decision
Use **Rhai 1.x** as the embedded expression/scripting engine.

## Candidates Evaluated

| Library | Pros | Cons |
|---|---|---|
| Rhai | Sandboxed by design, fast, Rust-native, variable map access, good error messages | Not FEEL compliant |
| Boa | Full JavaScript engine, FEEL-like syntax | Heavy (V8-level), slower startup |
| evalexpr | Lightweight expression evaluator | Limited features, no function calls |
| Custom FEEL | Full spec compliance | Months of implementation work |

## Rationale
- Rhai is sandboxed by default — cannot access filesystem, network, or system
- Variables can be injected as a scope — maps directly to process variables
- Fast: sub-microsecond for simple expressions
- Rust-native: no FFI, no unsafe, compiles to our binary
- Good error messages help debug broken gateway conditions
- FEEL compliance is not required for v1 — Rhai syntax is close enough
- Can build a FEEL-to-Rhai transpiler later if needed

## Spike Validation
Evaluate these expressions against a variable map:
- [ ] `amount > 100`
- [ ] `plan == "premium" && creditScore >= 700`
- [ ] `!isBlacklisted`
- [ ] `status.starts_with("APP")`  (string functions)
- [ ] `amount > 500` where amount is integer variable
- [ ] Sandbox test: expression trying to read a file → must fail/error

Performance:
- [ ] 1,000,000 evaluations of `amount > 100` — should be < 1 second

## Consequences
- Gateway conditions use Rhai syntax (not FEEL)
- Script tasks use Rhai syntax (not Groovy/JavaScript)
- Future: could add FEEL transpiler layer on top
