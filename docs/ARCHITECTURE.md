# Architecture

## Why Rust

| Concern | Rust Advantage |
|---|---|
| Performance | C-level speed, zero-cost abstractions |
| Safety | Memory safety at compile time, no data races |
| Concurrency | Fearless concurrency, caught at compile time |
| Deployment | Single binary, no runtime dependencies |
| Footprint | ~10MB vs 2GB+ for JVM engines |
| Startup | <100ms vs 30-90s for JVM engines |

## Why Not Middleware

Traditional BPM engines (Camunda, Flowable, jBPM) were built on JVM middleware because:
- They emerged from ESB / enterprise integration world (1990s-2000s)
- Messaging, transactions, connectors came from middleware for free
- Enterprise buyers demanded WebSphere / JBoss compatibility
- JVM was the dominant enterprise platform

In 2026 this is legacy weight. The cloud native stack replaces everything:

| Middleware Provided | Modern Replacement |
|---|---|
| Message broker (MQ/JMS) | Kafka, NATS, RabbitMQ |
| Service connectivity | REST, gRPC |
| Protocol mediation | API Gateway |
| Transaction management | Saga pattern, outbox pattern |
| Service discovery | Kubernetes DNS |
| Load balancing | Kubernetes, Envoy |
| Connector ecosystem | SaaS APIs, webhooks |
| Deployment | Docker + Kubernetes |

A modern process engine needs only: **PostgreSQL + an HTTP server**.

---

## System Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                          Conduit                               │
│                                                              │
│   ┌──────────────────────────────────────────────────────┐  │
│   │                    API Layer (Axum)                   │  │
│   │                                                       │  │
│   │  /orgs  /users  /deployments  /instances  /tasks       │  │
│   │  /external-tasks  /messages  /signals  /decisions      │  │
│   │  /health                                               │  │
│   └────────────────────────┬──────────────────────────────┘  │
│                            │                                  │
│   ┌────────────────────────▼──────────────────────────────┐  │
│   │                 Execution Engine                       │  │
│   │                                                       │  │
│   │   enter_element()  leave_element()  advance()         │  │
│   │   evaluate_conditions()  correlate_message()          │  │
│   └────────────────────────┬──────────────────────────────┘  │
│                            │                                  │
│   ┌────────────────────────▼──────────────────────────────┐  │
│   │                  Job Executor                          │  │
│   │           (Tokio background task)                      │  │
│   │                                                       │  │
│   │   poll due jobs → FOR UPDATE SKIP LOCKED → fire       │  │
│   └────────────────────────┬──────────────────────────────┘  │
│                            │                                  │
│   ┌────────────────────────▼──────────────────────────────┐  │
│   │                   PostgreSQL                           │  │
│   │                                                       │  │
│   │  orgs  users                                          │  │
│   │  process_definitions  process_instances  executions   │  │
│   │  variables  tasks  jobs  event_subscriptions          │  │
│   │  execution_history  parallel_join_state               │  │
│   │  decision_definitions                                 │  │
│   └───────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────┘
         ▲                                   ▲
         │                                   │
  Your Application                       Workers
  (starts instances,                 (any language,
   completes tasks,                   poll /external-tasks,
   sends messages)                    do business logic)
```

### Worker boundary

The engine speaks BPMN; **workers speak protocols.** A `serviceTask` carries a `<conduit:taskTopic>` that the engine uses purely as a routing label — it knows nothing about HTTP, gRPC, Kafka, file I/O, or any other transport. A worker subscribes to that topic, fetches the task via `/external-tasks/fetch-and-lock`, runs whatever side effect it owns, and reports back with `complete` or `failure`.

This is the single rule recorded in [ADR-008](adr/ADR-008-engine-stays-pure-bpmn.md). It's why the engine has no `reqwest`, no Kafka client, no GCS SDK in its dependency graph — and why durability of side effects is a worker concern (Phase 21's idempotency-key store), not an engine concern.

The one historical exception, `<conduit:http>`, is deprecated as of Phase 20 and will be removed once at least one external user confirms a clean migration to the [reference HTTP worker](../workers/crates/http-worker/) shipped alongside the engine.

---

## Data Flow

### Starting a Process
```
POST /api/v1/process-instances
  { "org_id": "...", "definition_id": "...", "labels": { ... } }
        ↓
API handler validates request
        ↓
Engine.start_instance(definition_id, org_id, labels)
  → INSERT process_instances (org_id, labels)
  → INSERT executions (at StartEvent)
  → advance() → passes through StartEvent immediately
  → enters first real element (UserTask, ServiceTask etc.)
  → INSERT tasks or jobs depending on element type
  → INSERT execution_history entries
  → COMMIT transaction
        ↓
Return 201 ProcessInstance { id, org_id, state, labels, ... }
```

### Advancing a Token
```
All state changes happen in ONE transaction:

BEGIN
  UPDATE executions SET activity_id = $new, updated_at = NOW()
  -- element-specific actions:
  INSERT tasks (if UserTask)
  INSERT jobs  (if ServiceTask / Timer)
  INSERT event_subscriptions (if CatchEvent)
  DELETE tasks/jobs/subscriptions (cleanup previous)
COMMIT
```

### Job Executor Loop
```
Loop every 200ms:
  SELECT * FROM jobs
  WHERE due_date <= NOW()
  AND (locked_until IS NULL OR locked_until < NOW())
  LIMIT 10
  FOR UPDATE SKIP LOCKED    ← safe for multiple engine instances

  For each job:
    tokio::spawn(async { engine.fire_job(job).await })

  Sleep 200ms
```

`FOR UPDATE SKIP LOCKED` means multiple engine replicas can all poll
simultaneously without blocking each other or double-firing.

---

## Concurrency Model

```
Single engine instance:
  - One Tokio runtime
  - Multiple async tasks running concurrently
  - Each token advancement is one transaction
  - DB handles concurrent access safely

Multiple engine instances (clustering):
  - All instances share the same PostgreSQL
  - FOR UPDATE SKIP LOCKED prevents double job firing
  - Optimistic locking prevents concurrent token advancement
  - No engine-level coordination needed
```

### Optimistic Locking for Token Advancement
```sql
UPDATE executions
SET activity_id = $1, version = version + 1, updated_at = NOW()
WHERE id = $2
AND version = $3   -- if someone else advanced it, version won't match
```
If 0 rows updated → conflict → retry or error.

---

## Process Graph (In-Memory)

The BPMN XML is parsed once at deployment into an in-memory graph.
Cached per process definition version. Never mutated.

```
ProcessGraph {
  elements: HashMap<ActivityId, FlowElement>
  outgoing:  HashMap<ActivityId, Vec<SequenceFlow>>
  incoming:  HashMap<ActivityId, Vec<SequenceFlow>>
}
```

Token advancement = graph traversal:
- Read current element from graph
- Evaluate leave conditions
- Follow sequence flows to next element(s)
- Execute enter logic for next element

---

## Variable Scoping

```
Process Instance scope (default):
  All tasks in the process can read/write

Execution scope (parallel branches):
  Variables created in a parallel branch
  Merged back to instance scope at join gateway

Subprocess scope:
  Subprocess can read parent variables
  Subprocess variables pushed back to parent on completion
```

---

## Error Handling Strategy

```
Transient errors (DB timeout, network blip):
  → Retry with exponential backoff
  → Job retries field decremented

Business errors (worker reports failure):
  → Boundary error event if configured
  → Instance marked ERROR if no handler

Engine errors (bug, assertion failure):
  → Instance marked ERROR
  → Error logged with full context
  → Human intervention required

Unhandled exceptions in workers:
  → Worker calls /external-tasks/:id/failure
  → Engine decrements retry count
  → After max retries → incident created
```

---

## Deployment

### Minimal (single node)
```yaml
services:
  engine:
    image: conduit:latest
    environment:
      DATABASE_URL: postgres://...
    ports:
      - "8080:8080"

  postgres:
    image: postgres:16-alpine
```

### Production (multi-node)
```yaml
services:
  engine:
    image: conduit:latest
    replicas: 3                    # all share same DB, safe via SKIP LOCKED
    environment:
      DATABASE_URL: postgres://...

  postgres:
    image: postgres:16-alpine      # or managed RDS/CloudSQL
```

No additional coordination between engine instances.
PostgreSQL is the single source of truth.

---

## API Design Principles

- REST with JSON
- UUID identifiers everywhere
- ISO 8601 timestamps
- Pagination via cursor (not offset)
- Errors as `{ "error": "message" }` with appropriate HTTP status
- Variables as typed objects `{ "name": "amount", "type": "integer", "value": 250 }`
