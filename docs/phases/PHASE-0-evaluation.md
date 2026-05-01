# Phase 0 — Technology Evaluation

## Goal
Prove every library choice before writing production code.
All spikes are throwaway — they live in `spikes/` and are never imported.

## Duration
2-3 weeks

## Output
- One ADR per technology decision (docs/adr/)
- Benchmark numbers for each decision
- Go/no-go on each candidate

## Spikes to Build

### Spike 0.1 — Tokio
```
spikes/tokio_eval/
```
- Spawn 10,000 concurrent tasks
- Measure: total time, memory usage
- Timer accuracy: sleep for 1ms, 10ms, 100ms, 1s — measure actual elapsed
- Expected: Tokio wins, proceed

### Spike 0.2 — Axum
```
spikes/axum_eval/
```
- Build 3 endpoints: POST /items, GET /items/:id, DELETE /items/:id
- Add shared state: Arc<Mutex<Vec<Item>>>
- Add middleware: request tracing
- Load test with `wrk` or `hey`: 1000 concurrent, 30 seconds
- Measure: p50, p95, p99 latency

### Spike 0.3 — SQLx
```
spikes/sqlx_eval/
```
- Create a jobs table with: id, topic, locked_by, locked_until, due_date
- Insert 1000 rows
- Spawn 10 Tokio tasks each doing:
  `SELECT ... FOR UPDATE SKIP LOCKED LIMIT 1`
- Verify: each row processed exactly once
- Measure: total processing time

### Spike 0.4 — XML Parser
```
spikes/xml_eval/
```
- Use a representative BPMN 2.0 fixture (e.g. one from `tests/fixtures/bpmn/`)
- Parse with roxmltree
- Extract and print: all element IDs, types, incoming/outgoing flows
- Extract Conduit-namespaced extension attributes (`conduit:topic`, `conduit:assignee`, etc.)
- Measure parse time

### Spike 0.5 — Rhai
```
spikes/rhai_eval/
```
- Create a variable scope with: amount=250, plan="premium", creditScore=720
- Evaluate:
  - `amount > 100` → true
  - `plan == "premium" && creditScore >= 700` → true
  - `amount > 1000` → false
  - `!isBlacklisted` → (isBlacklisted not in scope → error or false?)
- Sandbox test: try to read a file, must fail
- Benchmark: 1M evaluations of `amount > 100`

### Spike 0.6 — SQLx Migrate
```
spikes/migrate_eval/
```
- Create 3 migrations
- Run on fresh DB — verify all apply
- Run again — verify idempotent (no re-run)
- Modify a migration checksum — verify it refuses

## Status: Skipped — Fast-tracked to Phase 1

Spikes were not built. ADRs were written directly based on prior knowledge of the
libraries. Phase 1 has validated Tokio, Axum, SQLx, and SQLx migrate in practice.
Roxmltree and Rhai remain unvalidated until Phase 3 and Phase 6 respectively.

## Checklist

- [x] ADR-001 Tokio — accepted (validated in Phase 1 integration tests)
- [x] ADR-002 Axum — accepted (validated in Phase 1 health endpoint)
- [x] ADR-003 SQLx — accepted (validated in Phase 1 DB pool + migrations)
- [ ] ADR-004 XML (roxmltree) — deferred to Phase 3
- [ ] ADR-005 Rhai — deferred to Phase 6
- [x] ADR-006 SQLx migrate — accepted (validated in Phase 1 startup migrations)
- [x] All accepted ADRs have status "Accepted"
- [x] Ready to start Phase 1
