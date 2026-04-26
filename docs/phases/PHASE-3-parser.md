# Phase 3 — BPMN Parser (Subset)

## Status
**Complete** — 2026-04-26

## Prerequisites
Phase 2 complete and all tests passing.

## Summary
See docs/PLAN.md Phase 3 section for detailed task list, tests, and acceptance criteria.

## Checklist
- [x] Failing tests written
- [x] Implementation complete
- [x] All tests passing (this phase + all previous)
- [x] cargo clippy clean
- [x] cargo fmt clean
- [ ] CI green
- [x] Phase marked complete in PLAN.md

## What was built

- `src/parser/mod.rs` — parses BPMN XML into `ProcessGraph` (elements + adjacency maps)
- Supported elements: `startEvent`, `endEvent`, `userTask`, `serviceTask`, `sequenceFlow`
- Validation: rejects unsupported element types (e.g. `exclusiveGateway`), missing start event, malformed XML
- `ProcessGraph` cached in-memory per definition ID via `Arc<RwLock<HashMap>>`
- `POST /api/v1/deployments` — validates + parses BPMN before persisting; fail-fast, no DB write on parse error

## Key decisions

- Parse-first, persist-second: the graph is validated in memory before any DB write, so a bad BPMN never touches the DB
- Graph is stored as two `HashMap`s (outgoing, incoming flows) keyed by element ID — O(1) traversal at runtime
- Unsupported BPMN elements return a `400` with the element name in the error message
