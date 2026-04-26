# Phase 2 — Core DB Schema

## Status
**Complete** — 2026-04-26

## Prerequisites
Phase 1 complete and all tests passing.

## Summary
See docs/PLAN.md Phase 2 section for detailed task list, tests, and acceptance criteria.

## Checklist
- [x] Failing tests written
- [x] Implementation complete
- [x] All tests passing (26 tests: 7 schema + 19 prior)
- [x] cargo clippy clean
- [x] cargo fmt clean
- [ ] CI green
- [ ] Phase marked complete in PLAN.md

## What was built

- `migrations/002_core_schema.sql` — 7 tables: `process_definitions`, `process_instances`, `executions`, `variables`, `tasks`, `jobs`, `event_subscriptions`
- `src/db/` module directory with `models.rs` + one query module per table
- `tests/schema_test.rs` — 19 integration tests covering insert/read, FK constraints, cascade deletes, upsert idempotency, job locking exclusivity, retry counting, event routing, and partial index existence

## Key decisions

- TEXT + CHECK constraints for state fields (not ENUM) to allow adding values without migrations
- `executions.parent_id` self-referential FK included now (needed for Phase 9 parallel gateway, Phase 12 subprocess)
- `variables.execution_id` NOT NULL — process-level vars scope to root execution
- Partial index `idx_jobs_due_date_unlocked` on `jobs(due_date) WHERE locked_until IS NULL` for job executor hot path
- `FOR UPDATE SKIP LOCKED` in `jobs::fetch_and_lock` for safe concurrent worker access
