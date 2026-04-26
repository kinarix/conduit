# Phase 4 — Token Engine

## Status
**Complete** — 2026-04-26

## Prerequisites
Phase 3 complete and all tests passing.

## Summary
See docs/PLAN.md Phase 4 section for detailed task list, tests, and acceptance criteria.

## Checklist
- [x] Failing tests written
- [x] Implementation complete
- [x] All tests passing (this phase + all previous)
- [x] cargo clippy clean
- [x] cargo fmt clean
- [ ] CI green
- [x] Phase marked complete in PLAN.md

## What was built

- `src/engine/mod.rs` — `Engine` struct with two public methods:
  - `start_instance(definition_id, org_id, labels)` → creates instance, places token at StartEvent, advances through the graph until it hits a waiting element or EndEvent
  - `complete_user_task(task_id)` → marks task complete, resumes token advancement from that point
- Token advancement is fully transactional: all DB writes (executions, tasks, history) happen in one `BEGIN…COMMIT`
- `execution_history` table written on every element enter/leave (migration 003)
- `src/db/execution_history.rs` — `insert`, `set_left_at`, `list_by_instance`
- Graph loaded from in-memory cache; cache miss falls back to DB + re-parse (handles restart)

## Key decisions

- `execution_history` added in Phase 4 rather than a later phase — audit log is fundamental, not an add-on (see project memory)
- Engine holds `Arc<RwLock<HashMap<Uuid, Arc<ProcessGraph>>>>` — read-heavy cache with write on first miss
- `complete_user_task` returns `Conflict` (409) if the task is already completed — idempotency guard
- All state transitions use single atomic DB transaction; no in-memory state to lose on crash
