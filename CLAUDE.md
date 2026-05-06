# Conduit — Claude Code Guide

## Project Overview

A lightweight, high-performance process orchestration engine built in Rust.
Designed to be a modern alternative to JVM-based engines (Camunda, Flowable, Activiti)
without the middleware baggage of the enterprise era.

## Knowledge Graph

A navigable knowledge graph of this codebase lives in `graphify-out/`:
- `graphify-out/graph.html` — interactive visualization (open in browser)
- `graphify-out/graph.json` — raw graph data (457 nodes, 715 edges, 51 communities)
- `graphify-out/GRAPH_REPORT.md` — audit report with god nodes and surprising connections

Query it with `/graphify query "<question>"`. Rebuild after major changes with `/graphify .`.

## Core Design Principles

1. **No middleware** — PostgreSQL + Tokio is the entire infrastructure
2. **Single binary** — compiles to one executable, no JVM, no app server
3. **DB is the source of truth** — all state persisted atomically
4. **Workers are external** — engine orchestrates, workers execute
5. **Incremental phases** — every phase is working and deployable
6. **Test first** — integration tests with real DB via testcontainers
7. **Structured error codes** — every error has a U/S code, a client message, an optional user-action hint, and an optional server-side debug hint. U-prefix = user/client errors (4xx, actionable). S-prefix = system errors (5xx, never leaks internals). Codes are defined in `src/error_codes.toml` and asserted complete at startup. Wire format: `{"code": "U001", "message": "...", "action": "..."}`. See `src/error.rs`.

## Repository Structure

```
conduit/
├── CLAUDE.md                    ← You are here
├── Cargo.toml                   ← Workspace manifest
├── Cargo.lock
├── .env.example                 ← Environment variable template
├── docker-compose.yml           ← Local PostgreSQL
│
├── docs/
│   ├── PLAN.md                  ← Full incremental build plan
│   ├── BPM_CONCEPTS.md          ← BPM concepts reference
│   ├── ARCHITECTURE.md          ← Architecture decisions and diagrams
│   ├── adr/                     ← Architecture Decision Records
│   │   ├── ADR-001-async-runtime.md
│   │   ├── ADR-002-web-framework.md
│   │   ├── ADR-003-database-driver.md
│   │   ├── ADR-004-xml-parser.md
│   │   ├── ADR-005-expression-evaluator.md
│   │   └── ADR-006-migrations.md
│   └── phases/                  ← Detailed spec per phase
│       ├── PHASE-0-evaluation.md
│       ├── PHASE-1-foundation.md
│       ├── PHASE-2-schema.md
│       ├── PHASE-3-parser.md
│       ├── PHASE-4-token-engine.md
│       ├── PHASE-5-rest-api.md
│       ├── PHASE-5.5-ownership-labels.md
│       ├── PHASE-6-exclusive-gateway.md
│       ├── PHASE-7-external-tasks.md
│       ├── PHASE-8-timers.md
│       ├── PHASE-9-parallel-gateway.md
│       ├── PHASE-10-messages.md
│       ├── PHASE-11-signals.md
│       ├── PHASE-12-subprocess.md
│       ├── PHASE-13-inclusive-gateway.md
│       ├── PHASE-14-dmn.md
│       ├── PHASE-15-clustering.md
│       ├── PHASE-17-external-task-long-polling.md
│       ├── PHASE-18-element-documentation.md
│       └── PHASE-19-instance-notes.md
│
├── migrations/                  ← SQL migrations (SQLx)
│   ├── 001_initial.sql          ← uuid-ossp, schema_info, orgs
│   ├── 002_core_schema.sql      ← users, process_definitions, process_instances, executions, variables, tasks, jobs, event_subscriptions
│   └── 003_execution_history.sql ← execution_history audit table
│
├── src/
│   ├── main.rs                  ← Entry point
│   ├── config.rs                ← Environment config
│   ├── error.rs                 ← Unified error type
│   ├── db.rs                    ← DB pool setup
│   ├── api/                     ← HTTP handlers (Axum)
│   │   ├── mod.rs
│   │   ├── health.rs
│   │   ├── orgs.rs
│   │   ├── users.rs
│   │   ├── deployments.rs
│   │   ├── instances.rs
│   │   ├── tasks.rs
│   │   └── external_tasks.rs
│   ├── engine/                  ← Core execution engine
│   │   └── mod.rs
│   ├── parser/                  ← BPMN XML parser
│   │   └── mod.rs
│   └── db/                      ← DB query modules
│       ├── mod.rs
│       ├── models.rs
│       ├── orgs.rs
│       ├── users.rs
│       ├── process_definitions.rs
│       ├── process_instances.rs
│       ├── executions.rs
│       ├── execution_history.rs
│       ├── variables.rs
│       ├── tasks.rs
│       ├── jobs.rs
│       └── event_subscriptions.rs
│
└── tests/
    ├── common/
    │   └── mod.rs               ← Shared test helpers, spawn_test_app, create_test_org
    ├── health_test.rs
    ├── schema_test.rs
    ├── deployment_test.rs
    ├── engine_test.rs
    ├── parser_test.rs
    ├── api_test.rs
    └── external_task_test.rs
```

## Current Phase

**Phase 15 — Clustering + Observability** (next up)

Phases 0–14 are complete. See `docs/phases/PHASE-15-clustering.md` and `docs/PLAN.md` for the next phase spec.

### Completed phases
| Phase | What was built |
|---|---|
| 0 | Technology evaluation — ADRs for runtime, web framework, DB driver, XML parser, expression evaluator, migrations |
| 1 | Foundation — service entrypoint, config, error types, DB pool, health endpoint, migrations |
| 2 | Core DB schema — 7 tables: process_definitions, process_instances, executions, variables, tasks, jobs, event_subscriptions |
| 3 | BPMN parser — ProcessGraph from XML; startEvent, endEvent, userTask, serviceTask; `POST /api/v1/deployments` |
| 4 | Token engine — start_instance, complete_user_task, execution_history audit log, single-transaction advancement |
| 5 | REST API — deployments, instances, tasks endpoints; 77 integration tests |
| 5.5 | Ownership + labels — orgs table, users table, org_id/owner_id/labels on definitions and instances; `POST /api/v1/orgs`, `/users` |
| 6 | Exclusive gateway — condition evaluation (FEEL via dsntk; originally Rhai, migrated in Phase 6.1), default flow fallback, strict expression-error handling |
| 7 | External task API — fetch-and-lock, complete, failure, extend-lock; Camunda-style worker pattern |
| 8 | Job executor + timers — ISO 8601 duration parsing, IntermediateCatchEvent (timer), boundary timer events (interrupting), FOR UPDATE SKIP LOCKED concurrent safety |
| 9 | Parallel gateway — fork/join with atomic join counting, parallel_join_state table, work-stack execution, variable merging |
| 10 | Message events — IntermediateCatchEvent (message), ReceiveTask, correlation key matching, MessageStartEvent, `POST /api/v1/messages` |
| 11 | Signal events — IntermediateSignalCatchEvent, BoundarySignalEvent (interrupting + non-interrupting), SignalStartEvent, `POST /api/v1/signals/broadcast` |
| 12 | Embedded subprocess — SubProcess variant, `find_element_graph` recursive lookup, subprocess scoping, nested subprocess support |
| 13 | Inclusive gateway — OR routing with selective join, condition evaluation on all outgoing flows, `parallel_join_state` reuse with `expected_count = matched.len()` |
| 14 | DMN integration — decision_definitions table, DMN XML parser, mini FEEL evaluator, BusinessRuleTask engine arm, `POST /api/v1/decisions` |

## How to Work Through the Phases

Each phase has a spec in `docs/phases/`. Work through them in order:

```
1. Read the phase spec
2. Write failing tests first
3. Implement minimum code to pass tests
4. Run full test suite — nothing from previous phases should break
5. Commit with phase tag: "phase-1: foundation complete"
6. Move to next phase
```

## Commands

```bash
# Start local DB
docker-compose up -d

# Run migrations
cargo sqlx migrate run

# Run all tests (always use make test, never cargo test directly)
make test

# Run with logging
RUST_LOG=debug cargo run

# Check before commit
make check
```

## Environment Variables

```bash
DATABASE_URL=postgres://conduit:conduit_secret@localhost/conduit
SERVER_HOST=0.0.0.0
SERVER_PORT=8080
LOG_LEVEL=info
```

## Key Concepts (Quick Reference)

```
Process Definition  → the BPMN blueprint (XML)
Process Instance    → a running execution of the definition
Execution / Token   → tracks current position in the flow
Task                → work item waiting for human or worker
Job                 → scheduled work (timers, async tasks)
Event Subscription  → waiting for a message or signal
Variable            → process working memory (key-value)
```

## Technology Stack

| Concern | Library | Version | Why |
|---|---|---|---|
| Async runtime | Tokio | 1.x | Industry standard |
| Web framework | Axum | 0.7.x | Native Tokio, ergonomic |
| Database | SQLx | 0.7.x | Compile-time checked queries |
| XML parsing | roxmltree | 0.19.x | Simple DOM API |
| Expressions | dsntk-feel-evaluator (FEEL, DMN 1.5) | 0.3.x | Sandboxed; spec-aligned with BPMN/DMN and Camunda 8 / Zeebe |
| Migrations | SQLx migrate | built-in | Zero overhead |
| Testing | testcontainers | 0.15.x | Real DB in tests |

## Architecture Overview

```
                    ┌──────────────────────────────┐
                    │           Conduit               │
                    │                              │
  Your App ──REST──▶│  API Layer (Axum)            │
                    │        │                    │
  Workers ──REST──▶│  Execution Engine            │
                    │        │                    │
                    │  Job Executor (Tokio)        │
                    │        │                    │
                    │     PostgreSQL               │
                    └──────────────────────────────┘
```

The engine has no knowledge of workers. Workers poll for work.
Workers can be written in any language.

## Non-Goals (explicitly out of scope)

- ESB / middleware integration
- SOAP / JMS support
- Built-in UI (bring your own)
- Multi-tenancy (phase 15+)
- Full BPMN 2.0 conformance (build incrementally)
- DMN (phase 14)
