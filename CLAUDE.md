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
│   │   ├── ADR-006-migrations.md
│   │   ├── ADR-007-connector-architecture.md  ← rejected
│   │   └── ADR-008-engine-stays-pure-bpmn.md  ← supersedes ADR-007
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
│       ├── PHASE-16-decision-table-ui.md
│       ├── PHASE-17-external-task-long-polling.md
│       ├── PHASE-18-element-documentation.md
│       ├── PHASE-19-instance-notes.md
│       ├── PHASE-20-deprecate-http-connector.md
│       └── PHASE-21-reference-workers.md
│
├── migrations/                  ← SQL migrations (SQLx) — 001..023
│   ├── 001_initial.sql          ← uuid-ossp, schema_info, orgs
│   ├── 002_users.sql            ← users + auth
│   ├── 003_orgs_users.sql
│   ├── 004_process_groups.sql   ← orgs → groups → processes/decisions hierarchy
│   ├── 005_process_definitions.sql
│   ├── 006_process_instances.sql
│   ├── 007–013                  ← executions, variables, tasks, jobs, event_subscriptions, history, parallel_join_state
│   ├── 014_decision_definitions.sql
│   ├── 015_timer_start_triggers.sql
│   ├── 016_process_events.sql
│   ├── 017_secrets.sql          ← encrypted secret storage
│   ├── 018_jobs_http_config.sql ← HTTP connector job config
│   ├── 019_event_subscriptions_error_type.sql
│   ├── 020_process_layouts.sql  ← persisted modeller positions
│   ├── 021_decision_group_scoping.sql
│   ├── 022_process_definition_disabled.sql ← per-version disable
│   └── 023_process_instance_counter.sql    ← human-friendly sequential ID
│
├── src/
│   ├── main.rs                  ← Entry point
│   ├── config.rs                ← Environment config
│   ├── error.rs                 ← Unified error type
│   ├── error_codes.toml         ← U/S structured error codes
│   ├── db.rs                    ← DB pool setup
│   ├── api/                     ← HTTP handlers (Axum)
│   │   ├── mod.rs, extractors.rs, health.rs
│   │   ├── orgs.rs, users.rs, process_groups.rs
│   │   ├── deployments.rs       ← deploy + disable + rename-by-key
│   │   ├── instances.rs         ← paginated list, start, cancel
│   │   ├── tasks.rs, external_tasks.rs
│   │   ├── decisions.rs         ← deploy DMN + rename-by-key + test
│   │   ├── messages.rs, signals.rs
│   │   ├── process_layouts.rs   ← modeller layout persistence
│   │   └── secrets.rs           ← secret CRUD
│   ├── engine/                  ← Core execution engine
│   │   ├── mod.rs, instance.rs, token.rs, helpers.rs
│   │   ├── timer.rs, message.rs, signal.rs
│   │   ├── send_message.rs, user_task.rs, external_task.rs
│   │   ├── http.rs              ← HTTP push connector runtime
│   │   ├── evaluator.rs         ← FEEL evaluator wrapper
│   │   └── jq.rs                ← jq-style transforms
│   ├── parser/                  ← BPMN XML parser
│   │   ├── mod.rs
│   │   └── types.rs             ← FlowNodeKind, ProcessGraph
│   └── db/                      ← DB query modules
│       ├── mod.rs, models.rs
│       ├── orgs.rs, users.rs, process_groups.rs
│       ├── process_definitions.rs, process_instances.rs
│       ├── executions.rs, execution_history.rs
│       ├── variables.rs, tasks.rs, jobs.rs
│       ├── event_subscriptions.rs, process_events.rs
│       ├── decision_definitions.rs
│       ├── process_layouts.rs
│       └── secrets.rs
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

**Phase 20 — Deprecate `<conduit:http>` connector** (in progress); **Phase 16 — Decision Table UI + Full FEEL** continuing in parallel.

Phases 0–15 are complete. Phase 20 lands the deprecation warning (`U010`) on the deployment response and the migration guide; runtime behaviour of `<conduit:http>` is unchanged. Removal happens in a follow-up phase gated on at least one external user successfully migrating to the reference HTTP worker (Phase 21, sibling repo).

Phase 16 (Decision Table UI) work continues in parallel along with several operational improvements that have already shipped past the core phase line:

### Active workstreams (shipped or in flight beyond Phase 15)
- **HTTP push connector** for `serviceTask` (`<conduit:http>`) — **deprecated** per [ADR-008](docs/adr/ADR-008-engine-stays-pure-bpmn.md); migration via reference HTTP worker (Phases 20–21)
- **Encrypted secrets** referenced as `{{secret:name}}` in connector configs
- **Per-version enable/disable** for process definitions (`PATCH /deployments/{id}/disabled`)
- **Process instance counter** — sequential per-(org, process_key) human-friendly ID
- **List pagination** on instances (`limit`, `offset`, `X-Total-Count`)
- **Rename across versions** for processes and decisions (`PATCH .../by-key`)
- **Decision version pinning** on `businessRuleTask` (`<conduit:decisionRef version="N">`)
- **ScriptTask** (FEEL body, optional result variable) and **SendTask** (publish a named message)
- **BoundaryErrorEvent** (interrupting; pairs with worker-thrown BPMN errors)
- **Sidebar UI** — orgs → process groups → processes & decisions tree, inline rename, draft/promote
- **Visual BPMN editor** — ReactFlow modeller with elbow connectors, fit-on-open, schema builder
- **Decision Table editor** scaffolding for the Phase 16 UI

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
| 15 | Clustering + Observability — leader election (pg_try_advisory_lock), Prometheus /metrics, graceful shutdown (CancellationToken), JSON logging, enhanced health endpoint |

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
# DATABASE_URL is unprefixed by sqlx convention; everything else uses CONDUIT_*.
DATABASE_URL=postgres://conduit:conduit_secret@localhost/conduit
CONDUIT_SERVER_HOST=0.0.0.0
CONDUIT_SERVER_PORT=8080
CONDUIT_LOG_LEVEL=info
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
