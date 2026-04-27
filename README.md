# Conduit

A lightweight, high-performance BPMN process orchestration engine written in Rust.

Built as a modern alternative to JVM-based engines (Camunda, Flowable, Activiti) — no middleware, no JVM, no app server. Just PostgreSQL and a single binary.

## Why

| | JVM engines | Conduit |
|---|---|---|
| Startup | 30–90 seconds | < 100ms |
| Memory footprint | 2 GB+ | ~10 MB |
| Deployment | JVM + WAR + config | Single binary |
| Infrastructure | App server + MQ + DB | PostgreSQL only |

A cloud-native process engine needs two things: **an HTTP server and a database**.

## Architecture

```
Your App ──REST──▶ ┌─────────────────────────┐
                   │        Conduit          │
Workers ──REST──▶  │   API Layer (Axum)      │
                   │   Execution Engine      │
                   │   Job Executor (Tokio)  │
                   │      PostgreSQL         │
                   └─────────────────────────┘
```

- Workers are external — the engine orchestrates, workers execute
- Workers can be written in any language and poll for work over REST
- All state is persisted atomically in PostgreSQL
- Multiple engine instances share one database safely via `FOR UPDATE SKIP LOCKED`

## Current Status

The engine is being built incrementally. Each phase is working and deployable before the next begins.

| Phase | Description | Status |
|---|---|---|
| 0 | Technology evaluation | ✅ |
| 1 | Foundation (config, DB pool, health endpoint) | ✅ |
| 2 | Database schema | ✅ |
| 3 | BPMN parser + deploy endpoint | ✅ |
| 4 | Token execution engine | ✅ |
| 5 | REST API (instances, tasks) | ✅ |
| 5.5 | Ownership + labels (orgs, users) | ✅ |
| 6 | Exclusive gateway | ✅ |
| 7 | External tasks (fetch-and-lock workers) | ✅ |
| 8 | Job executor + timers | ✅ |
| 9 | Parallel gateway | ✅ |
| 10 | Messages | ✅ |
| 11 | Signals | ✅ |
| 12 | Sub-processes | 🔜 |
| 13 | Inclusive gateway | 🔜 |
| 14 | DMN decision tables | 🔜 |
| 15 | Clustering + observability | 🔜 |

## Supported BPMN Elements

| Element | Since |
|---|---|
| `startEvent` | Phase 3 |
| `endEvent` | Phase 3 |
| `userTask` | Phase 3 |
| `serviceTask` (with `topic` / `camunda:topic` for external workers) | Phase 3 |
| `sequenceFlow` | Phase 3 |
| `exclusiveGateway` (with Rhai condition expressions, default flow) | Phase 6 |
| `intermediateCatchEvent` — timer (`timeDuration` ISO 8601) | Phase 8 |
| `boundaryEvent` — interrupting timer on tasks | Phase 8 |
| `parallelGateway` (fork + join) | Phase 9 |
| `intermediateCatchEvent` — message (correlation key) | Phase 10 |
| `receiveTask` | Phase 10 |
| `intermediateCatchEvent` — signal (broadcast) | Phase 11 |
| `boundaryEvent` — interrupting / non-interrupting signal | Phase 11 |
| `startEvent` — signal start | Phase 11 |

Both standard BPMN 2.0 and Camunda dialect (`bpmn:` namespace prefix, `camunda:` extension attributes) are supported.

## Getting Started

**Prerequisites:** Rust, Docker

```bash
# Start PostgreSQL
docker-compose up -d

# Run migrations
cargo sqlx migrate run

# Start the engine
DATABASE_URL=postgres://conduit:conduit_secret@localhost/conduit cargo run
```

The server listens on `http://0.0.0.0:8080` by default.

## API

### Deploy a process definition

```bash
curl -X POST http://localhost:8080/api/v1/deployments \
  -H 'Content-Type: application/json' \
  -d '{
    "key": "order-fulfillment",
    "name": "Order Fulfillment",
    "bpmn_xml": "<definitions>...</definitions>"
  }'
```

```json
{
  "id": "018e1b2c-3d4e-7f8a-9b0c-1d2e3f4a5b6c",
  "key": "order-fulfillment",
  "version": 1,
  "deployed_at": "2026-04-26T10:00:00Z"
}
```

Deploying the same `key` again creates version 2, 3, etc.

### Health check

```bash
curl http://localhost:8080/health
```

```json
{ "status": "ok", "database": "connected", "version": "0.1.0" }
```

## Configuration

| Variable | Default | Description |
|---|---|---|
| `DATABASE_URL` | — | PostgreSQL connection string (required) |
| `SERVER_HOST` | `0.0.0.0` | Bind address |
| `SERVER_PORT` | `8080` | Listen port |
| `LOG_LEVEL` | `info` | Tracing filter (e.g. `debug`, `conduit=trace`) |

## Running Tests

Integration tests require a running PostgreSQL instance.

```bash
# Run everything
DATABASE_URL=postgres://conduit:conduit_secret@localhost/conduit cargo test

# Parser unit tests only (no DB needed)
cargo test --test parser_test
```

## Technology Stack

| Concern | Library |
|---|---|
| Async runtime | Tokio |
| Web framework | Axum |
| Database | SQLx (compile-time checked queries) |
| XML parsing | roxmltree |
| Expressions | Rhai (Phase 6) |
| Migrations | SQLx migrate |

## License

Apache 2.0 — see [LICENSE](LICENSE).
