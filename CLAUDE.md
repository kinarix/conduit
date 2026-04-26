# Conduit — Claude Code Guide

## Project Overview

A lightweight, high-performance process orchestration engine built in Rust.
Designed to be a modern alternative to JVM-based engines (Camunda, Flowable, Activiti)
without the middleware baggage of the enterprise era.

## Core Design Principles

1. **No middleware** — PostgreSQL + Tokio is the entire infrastructure
2. **Single binary** — compiles to one executable, no JVM, no app server
3. **DB is the source of truth** — all state persisted atomically
4. **Workers are external** — engine orchestrates, workers execute
5. **Incremental phases** — every phase is working and deployable
6. **Test first** — integration tests with real DB via testcontainers

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
│       ├── PHASE-6-exclusive-gateway.md
│       ├── PHASE-7-external-tasks.md
│       ├── PHASE-8-timers.md
│       ├── PHASE-9-parallel-gateway.md
│       ├── PHASE-10-messages.md
│       ├── PHASE-11-signals.md
│       ├── PHASE-12-subprocess.md
│       ├── PHASE-13-inclusive-gateway.md
│       ├── PHASE-14-dmn.md
│       └── PHASE-15-clustering.md
│
├── migrations/                  ← SQL migrations (SQLx)
│   └── 001_initial.sql
│
├── src/
│   ├── main.rs                  ← Entry point
│   ├── config.rs                ← Environment config
│   ├── error.rs                 ← Unified error type
│   ├── db.rs                    ← DB pool setup
│   ├── api/                     ← HTTP handlers (Axum)
│   │   ├── mod.rs
│   │   └── health.rs
│   ├── engine/                  ← Core execution engine
│   │   └── mod.rs
│   ├── parser/                  ← BPMN XML parser
│   │   └── mod.rs
│   └── db/                      ← DB query modules
│       └── mod.rs
│
└── tests/
    ├── common/
    │   └── mod.rs               ← Shared test helpers, DB containers
    └── health_test.rs
```

## Current Phase

**Phase 0 — Technology Evaluation** (start here)

See `docs/phases/PHASE-0-evaluation.md` for what needs to be done.

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

# Run all tests
cargo test

# Run with logging
RUST_LOG=debug cargo run

# Check before commit
cargo fmt && cargo clippy -- -D warnings && cargo test
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
| Expressions | Rhai | 1.x | Sandboxed, fast |
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
