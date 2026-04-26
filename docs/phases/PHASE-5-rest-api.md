# Phase 5 — REST API

## Status
**Complete** — 2026-04-26

## Prerequisites
Phase 4 complete and all tests passing.

## Summary
See docs/PLAN.md Phase 5 section for detailed task list, tests, and acceptance criteria.

## Checklist
- [x] Failing tests written
- [x] Implementation complete
- [x] All tests passing (this phase + all previous)
- [x] cargo clippy clean
- [x] cargo fmt clean
- [ ] CI green
- [x] Phase marked complete in PLAN.md

## What was built

Endpoints:
```
POST   /api/v1/deployments                  deploy BPMN → ProcessDefinition
POST   /api/v1/process-instances            start instance
GET    /api/v1/process-instances/:id        get instance
GET    /api/v1/tasks                        list pending tasks
GET    /api/v1/tasks/:id                    get task
POST   /api/v1/tasks/:id/complete           complete user task (→ 204)
```

- `src/api/deployments.rs`, `instances.rs`, `tasks.rs`
- `tests/deployment_test.rs` — 13 tests covering valid deploys, version increments, validation rejections (400/422)
- `tests/engine_test.rs` — 11 tests covering instance lifecycle, user task completion, history audit, cold-cache restart

## Key decisions

- Deploy returns `201` with `{ id, key, version, deployed_at }` — minimal response, no BPMN XML echoed back
- `POST /tasks/:id/complete` returns `204 No Content` (not the updated task) — completion is a side-effect, not a resource mutation from the caller's perspective
- Validation errors return `400 { "error": "..." }`; missing required JSON fields return `422` (Axum extractor default)
- `GET /tasks` lists all pending tasks globally — scoping by org/assignee deferred to Phase 5.5+
