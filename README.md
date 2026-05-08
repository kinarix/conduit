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
| 12 | Sub-processes | ✅ |
| 13 | Inclusive gateway | ✅ |
| 14 | DMN decision tables | ✅ |
| 15 | Clustering + observability | ✅ |
| 16 | Decision Table UI + Full FEEL (DRD, all hit policies) | 🚧 In progress |

### Beyond core phases (shipped)

In parallel with Phase 16 work, the engine and UI have grown several operational features:

- **HTTP push connector** for `serviceTask` via `<conduit:http>` — **deprecated** as of Phase 20; runtime still works, deployments emit a `U010` warning. Migrate to a worker-based `serviceTask` per [`docs/MIGRATION.md`](docs/MIGRATION.md). Rationale: [ADR-008](docs/adr/ADR-008-engine-stays-pure-bpmn.md)
- **Encrypted secrets** — referenced as `{{secret:name}}` inside connector configs and templated values
- **Per-version enable/disable** of process definitions for safe rollback (`PATCH /deployments/{id}/disabled`)
- **Human-friendly instance counter** — sequential per `(org, process_key)` ID alongside the UUID
- **Pagination** on the instances list endpoint (`limit`, `offset`, `X-Total-Count`)
- **Rename across all versions** of a process or decision in one call (`PATCH .../by-key`)
- **Decision version pinning** on `businessRuleTask` (`<conduit:decisionRef version="3">`)
- **Sidebar UI** — orgs → process groups → processes & decisions tree, inline rename, draft/promote workflow
- **Visual BPMN editor** — ReactFlow-based modeller with elbow connectors, fit-on-open, schema builder

## Supported BPMN Elements

| Element | Since |
|---|---|
| `startEvent` | Phase 3 |
| `endEvent` | Phase 3 |
| `userTask` | Phase 3 |
| `serviceTask` (external worker via `conduit:taskTopic`, **or** HTTP push via `<conduit:http>` — *deprecated, see [MIGRATION.md](docs/MIGRATION.md)*) | Phase 3 / 16 |
| `scriptTask` (FEEL expression body, optional `result_variable`) | Phase 16 |
| `sendTask` (publishes a message by name) | Phase 16 |
| `sequenceFlow` | Phase 3 |
| `exclusiveGateway` (FEEL conditions, default flow) | Phase 6 |
| `intermediateCatchEvent` — timer (`timeDuration` ISO 8601) | Phase 8 |
| `boundaryEvent` — interrupting timer on tasks | Phase 8 |
| `parallelGateway` (fork + join) | Phase 9 |
| `intermediateCatchEvent` — message (correlation key) | Phase 10 |
| `receiveTask` | Phase 10 |
| `intermediateCatchEvent` — signal (broadcast) | Phase 11 |
| `boundaryEvent` — interrupting / non-interrupting signal | Phase 11 |
| `boundaryEvent` — error (interrupting; `errorCode` optional) | Phase 14 |
| `startEvent` — message start | Phase 10 |
| `startEvent` — signal start | Phase 11 |
| `startEvent` — timer start (re-armed on deploy/enable) | Phase 8 |
| `subProcess` (embedded, nested) | Phase 12 |
| `inclusiveGateway` (OR routing with selective join) | Phase 13 |
| `businessRuleTask` (with `conduit:decisionRef`, optional `version` pin) | Phase 14 |

Standard BPMN 2.0 is supported, with `bpmn:` as the prefix and Conduit's own `conduit:` namespace (`http://conduit.io/bpmn`) for extension attributes such as `conduit:topic`, `conduit:assignee`, and `conduit:decisionRef`.

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

### Disable / re-enable a deployed version

```bash
curl -X PATCH http://localhost:8080/api/v1/deployments/{id}/disabled \
  -H 'Content-Type: application/json' \
  -d '{"disabled": true}'
```

A disabled version cannot start **new** instances (manual, message, signal, or timer). Existing instances keep running on whichever version they were started on. The engine cancels timer-start jobs when a version is disabled and re-arms them when it is re-enabled. Drafts cannot be disabled.

### Rename across all versions

A process key or decision key has many versions. Renaming the *display name* once updates every version in the org / group:

```bash
curl -X PATCH http://localhost:8080/api/v1/deployments/by-key \
  -H 'Content-Type: application/json' \
  -d '{"org_id":"...","process_group_id":"...","process_key":"order-fulfillment","name":"Order Fulfillment v2"}'

curl -X PATCH http://localhost:8080/api/v1/decisions/by-key \
  -H 'Content-Type: application/json' \
  -H 'x-org-id: <org-uuid>' \
  -d '{"decision_key":"order-classification","name":"Order Classification"}'
```

### Pin a `businessRuleTask` to a specific decision version

By default, a `businessRuleTask` evaluates the latest deployed version of its decision. Add `version` to pin:

```xml
<bpmn:businessRuleTask id="classify_order">
  <bpmn:extensionElements>
    <conduit:decisionRef version="3">order-classification</conduit:decisionRef>
  </bpmn:extensionElements>
</bpmn:businessRuleTask>
```

### List instances with pagination

```bash
curl 'http://localhost:8080/api/v1/process-instances?org_id=<uuid>&process_key=order-fulfillment&limit=50&offset=0'
```

| Query param | Description |
|---|---|
| `org_id` | required |
| `definition_id` | restrict to a specific deployed version |
| `process_key` | restrict to a process key across all versions |
| `limit` | 1–500, default 100 |
| `offset` | default 0 |

The total row count is returned in the `X-Total-Count` response header.

Each `ProcessInstance` carries a `counter` field — a sequential per-(org, process_key) integer (1, 2, 3, …) assigned by the database. It's the human-friendly identifier shown in the UI in place of the UUID.

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
| Expressions | FEEL via `dsntk-feel-evaluator` (DMN 1.5) — Phase 6, migrated from Rhai in 6.1 |
| Migrations | SQLx migrate |

## License

Apache 2.0 — see [LICENSE](LICENSE).
