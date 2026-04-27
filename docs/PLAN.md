# Takt — Incremental Build Plan

## Philosophy

> Working software at every phase. No big bang. Each phase is deployable.

Every phase follows this pattern:
1. Write failing tests first
2. Implement minimum to pass
3. Full test suite stays green
4. Commit and tag

---

## Phase Summary

| Phase | Name | Status | Deliverable |
|---|---|---|---|
| 0 | Technology Evaluation | ✅ Complete | ADRs + spikes |
| 1 | Foundation | ✅ Complete | Running service + health endpoint |
| 2 | Core DB Schema | ✅ Complete | All tables, migrations |
| 3 | BPMN Parser (subset) | ✅ Complete | Parse start/end/task/sequence |
| 4 | Token Engine | ✅ Complete | Token creation + basic advancement |
| 5 | REST API | ✅ Complete | Start instance, complete task |
| 5.5 | Ownership + Labels | ✅ Complete | Orgs, users, org_id, labels on all resources |
| 6 | Exclusive Gateway | ✅ Complete | Conditions + routing |
| 7 | External Task API | ✅ Complete | Worker fetch-and-lock |
| 8 | Job Executor + Timers | ✅ Complete | Timer events, async jobs |
| 9 | Parallel Gateway | ✅ Complete | Fork + join |
| 10 | Message Events | ✅ Complete | Correlation + receive task |
| 11 | Signal Events | ✅ Complete | Broadcast |
| 12 | Subprocess + Boundary | — | Embedded subprocess, boundary events |
| 13 | Inclusive Gateway | — | OR routing |
| 14 | DMN Integration | — | Decision tables |
| 15 | Clustering + Observability | — | Multi-node, metrics |
| 16 | Table Partitioning + Archival | — | Partitioned schema, retention policy |

---

## Phase 0 — Technology Evaluation

**Goal:** Prove every library works before committing to it.

### Evaluations Required

#### 0.1 Async Runtime
- Candidates: Tokio, async-std, smol
- Spike: spawn 10k tasks, measure throughput + timer accuracy
- Decision criteria: ecosystem compatibility, performance, maturity
- Expected: Tokio

#### 0.2 Web Framework
- Candidates: Axum, Actix-web, Warp
- Spike: minimal REST API with shared state, JSON in/out
- Decision criteria: ergonomics, Tokio native, middleware support
- Expected: Axum

#### 0.3 Database Driver
- Candidates: SQLx, Diesel, SeaORM
- Spike: concurrent transactions + FOR UPDATE SKIP LOCKED
- Decision criteria: compile-time checks, async native, PG support
- Expected: SQLx

#### 0.4 XML Parser
- Candidates: roxmltree, quick-xml, minidom
- Spike: parse real BPMN file, extract elements + namespaced attributes
- Decision criteria: API ergonomics, namespace handling, performance
- Expected: roxmltree

#### 0.5 Expression Evaluator
- Candidates: Rhai, Boa, evalexpr, custom FEEL
- Spike: evaluate gateway conditions against variable maps
- Decision criteria: sandboxed, variable map access, performance
- Expected: Rhai

#### 0.6 Migration Tooling
- Candidates: SQLx migrate, Refinery, Flyway
- Spike: fresh DB migration + existing DB migration
- Decision criteria: zero overhead, CI friendly
- Expected: SQLx migrate

#### 0.7 Test Infrastructure
- testcontainers-rs — real PostgreSQL in tests
- cargo-nextest — faster test runner
- wiremock-rs — mock HTTP for worker tests

### Deliverables
- `docs/adr/ADR-001` through `ADR-006` — one per decision
- Spike code in `spikes/` directory (not production code)
- Benchmark numbers backing each decision

---

## Phase 1 — Foundation

**Goal:** A running Rust service with DB connection, migrations, health endpoint.

### Tasks
- [ ] Project structure (Cargo.toml, src layout)
- [ ] Environment config (dotenvy)
- [ ] Unified error type (thiserror + axum IntoResponse)
- [ ] DB connection pool (SQLx PgPool)
- [ ] Auto-run migrations on startup
- [ ] GET /health endpoint (checks DB connectivity)
- [ ] Structured logging (tracing)
- [ ] docker-compose for local PostgreSQL
- [ ] Integration test infrastructure (testcontainers)
- [ ] CI pipeline (GitHub Actions)

### Tests
- `GET /health` returns 200 with `{"status": "ok"}`
- `GET /health` returns degraded when DB unreachable
- Migrations run idempotently on startup

### Deliverable
A running service. `cargo run` → server starts → `curl /health` → ok.

---

## Phase 2 — Core DB Schema

**Goal:** All core tables created via migrations. No engine logic yet.

### Tables
- `process_definitions` — deployed BPMN, versioned
- `process_instances` — running executions
- `executions` — tokens (current position)
- `variables` — process working memory
- `tasks` — user task records
- `jobs` — timers + async work queue
- `event_subscriptions` — waiting for messages/signals

### Tasks
- [ ] Migration for each table
- [ ] Indexes for common query patterns
- [ ] Repository structs (Rust models matching DB rows)
- [ ] Basic CRUD for each table (no engine logic)

### Tests
- Insert + read each entity
- FK constraints enforced
- Index usage verified via EXPLAIN

### Deliverable
Full schema in place. All tables readable/writable.

---

## Phase 3 — BPMN Parser (Subset)

**Goal:** Parse a minimal BPMN XML into an in-memory graph.

### Supported Elements (subset only)
- StartEvent (None type)
- EndEvent (None type)
- UserTask
- ServiceTask (external topic)
- SequenceFlow (with optional condition)
- Process (root element)

### Tasks
- [ ] ProcessGraph struct (elements + adjacency)
- [ ] BPMN XML → ProcessGraph
- [ ] Validation (dangling flows, missing start/end)
- [ ] Store raw XML in process_definitions
- [ ] Cache parsed graph in memory (Arc<RwLock<HashMap>>)
- [ ] Deploy endpoint: POST /api/v1/deployments

### Tests
- Parse valid BPMN → correct graph structure
- Detect invalid BPMN (no start event, disconnected elements)
- Deploy via API → stored in DB
- Same key deployed twice → new version created

### Deliverable
POST a BPMN file → engine stores and parses it.

---

## Phase 4 — Token Engine

**Goal:** Tokens can be created and advanced through simple flows.

### Supported Flow
```
StartEvent → UserTask → EndEvent
StartEvent → ServiceTask → EndEvent
```

### Tasks
- [ ] Create process instance (insert to DB)
- [ ] Place token at StartEvent
- [ ] Advance past StartEvent immediately
- [ ] Enter UserTask → create task record, stop
- [ ] Enter ServiceTask → create external task job, stop
- [ ] Enter EndEvent → complete instance
- [ ] All advances in single DB transaction

### Tests
- Start instance → token at UserTask
- Complete UserTask → token at EndEvent → instance complete
- DB state consistent at each step
- Instance survives simulated restart (load from DB)

### Deliverable
A simple process runs end to end in the DB.

---

## Phase 5 — REST API

**Goal:** External callers can start instances and complete tasks.

### Endpoints
```
POST   /api/v1/process-instances          start instance
GET    /api/v1/process-instances/:id      get instance + status
GET    /api/v1/tasks                      list open tasks
GET    /api/v1/tasks/:id                  get task detail
POST   /api/v1/tasks/:id/complete         complete task + variables
```

### Tasks
- [ ] Request/response types (serde)
- [ ] Input validation
- [ ] Error responses (404, 400, 500)
- [ ] Pagination for list endpoints
- [ ] Variable serialisation (string, integer, boolean, json)

### Tests
- Start instance via API → 201 with instanceId
- Get instance → correct status and variables
- Complete task → process advances
- Invalid input → 400 with clear error
- Unknown ID → 404

### Deliverable
A process can be driven entirely via REST API calls.

---

## Phase 5.5 — Ownership + Labels

**Goal:** Every resource belongs to an org. Definitions and instances carry JSONB labels for filtered queries and future access-scoping. Auth enforcement is out of scope — structural plumbing only.

### Tables added / modified
- `orgs` — new table in migration 001
- `users` — new table in migration 002 (`auth_provider IN ('internal', 'external')`, email unique per org)
- `process_definitions` — gains `org_id`, `owner_id`, `labels JSONB`; UNIQUE constraint becomes `(org_id, process_key, version)`
- `process_instances` — gains `org_id`, `labels JSONB`

### Tasks
- [x] Add `orgs` table (migration 001)
- [x] Add `users` table (migration 002)
- [x] Add `org_id`, `owner_id`, `labels` to `process_definitions`
- [x] Add `org_id`, `labels` to `process_instances`
- [x] GIN indexes on both `labels` columns
- [x] `Org`, `User` Rust models; update `ProcessDefinition`, `ProcessInstance`
- [x] `src/db/orgs.rs`, `src/db/users.rs` query modules
- [x] Update `process_definitions::insert`, `next_version` signatures
- [x] Update `process_instances::insert` signature
- [x] Update `engine::start_instance` signature
- [x] `POST /api/v1/orgs`, `POST /api/v1/users` endpoints
- [x] `DeployRequest` and `StartInstanceRequest` gain `org_id` + `labels?`
- [x] `AUTH_PROVIDER` env var config
- [x] All tests updated; 77 tests passing

### Deliverable
All resources are org-scoped. Labels round-trip through the API. Schema is ready for auth middleware in a later phase.

---

## Phase 6 — Exclusive Gateway

**Goal:** Processes can branch based on variable conditions.

### Tasks
- [ ] Variable passing on task completion — `POST /tasks/:id/complete` accepts `{ "variables": [...] }` body; writes to `variables` table before advancing
- [ ] ExclusiveGateway in parser; read `conditionExpression` from sequence flows; mark default flow
- [ ] Rhai expression evaluator — sandboxed, variables injected into scope
- [ ] Condition evaluation on sequence flows — first-true-wins routing
- [ ] Default flow support
- [ ] Error on no matching condition (no default) — instance marked `error`

### Tests
- Variable round-trip: complete task with variables → variables in DB
- Route to correct path based on variable value
- Default flow taken when no condition matches
- Error raised when no condition matches and no default
- Nested exclusive gateways work correctly

### Deliverable
Processes with if/else branching work correctly. Variables can be written at task completion.

---

## Phase 7 — External Task API

**Goal:** Workers can poll for work and complete tasks.

### Endpoints
```
POST   /api/v1/external-tasks/fetch-and-lock    worker polls
POST   /api/v1/external-tasks/:id/complete      worker done
POST   /api/v1/external-tasks/:id/failure       worker failed
POST   /api/v1/external-tasks/:id/extend-lock   worker needs more time
```

### Tasks
- [ ] External task job creation on ServiceTask enter
- [ ] Fetch-and-lock (FOR UPDATE SKIP LOCKED)
- [ ] Lock expiry + automatic unlock
- [ ] Retry on failure (decrement retries)
- [ ] Dead letter after max retries
- [ ] Variable passing in + out

### Tests
- Worker fetches job → locked for that worker
- Second worker cannot fetch same job while locked
- Lock expires → job available again
- Worker completes → process advances
- Worker fails → retried up to max
- Max retries exceeded → instance marked error

### Deliverable
External workers can integrate with the engine.

---

## Phase 8 — Job Executor + Timer Events

**Goal:** Processes can wait for time durations and resume automatically.

### Tasks
- [ ] Job executor loop (Tokio background task)
- [ ] FOR UPDATE SKIP LOCKED for distributed safety
- [ ] Timer event in parser (IntermediateCatchEvent + TimerEventDefinition)
- [ ] Duration parsing (ISO 8601: PT24H, P7D)
- [ ] Job inserted when timer event entered
- [ ] Job fired when due → token advanced
- [ ] Boundary timer events (interrupting)

### Tests
- Process waits at timer → advances after duration
- Multiple timers fire in correct order
- Engine restart → timers still fire correctly
- Boundary timer interrupts task after duration
- Concurrent job executors don't double-fire

### Deliverable
Time-based waiting and escalation works.

---

## Phase 9 — Parallel Gateway ✅

**Goal:** Multiple paths execute simultaneously and synchronise.

### Tasks
- [x] ParallelGateway in parser
- [x] Fork: create concurrent execution per outgoing flow
- [x] Join: wait for all concurrent executions
- [x] Track concurrent execution count in DB (parallel_join_state table)
- [x] Handle nested parallel gateways (via scope propagation through parent_id)

### Tests
- [x] Fork creates correct number of tokens
- [x] Join waits until all paths complete
- [x] Parallel tasks execute independently (order-independent completion)
- [x] Variables from parallel paths merged correctly
- [x] Post-join continuation works

### Deliverable
Parallel execution with synchronisation works.

---

## Phase 10 — Message Events

**Goal:** Processes can send and receive messages for cross-instance communication.

### Tasks
- [ ] Message definitions in parser
- [ ] IntermediateCatchEvent (Message) → create subscription
- [ ] ReceiveTask → create subscription
- [ ] POST /api/v1/messages → correlate + advance
- [ ] Correlation key matching
- [ ] Message Start Event → start new instance on message

### Tests
- Process waits at receive → message arrives → advances
- Wrong correlation key → no match (not advanced)
- Message before instance ready → error (not silently dropped)
- Message Start Event → new instance created on message

### Deliverable
Message-based coordination between processes works.

---

## Phase 11 — Signal Events ✅

**Goal:** Broadcast signals can activate multiple waiting instances simultaneously.

### Tasks
- [x] Signal definitions in parser (`SignalStartEvent`, `IntermediateSignalCatchEvent`, `BoundarySignalEvent`)
- [x] IntermediateCatchEvent (Signal) → subscription in `event_subscriptions`
- [x] POST /api/v1/signals/broadcast → broadcast to all waiting (no-error if no listeners)
- [x] Signal Start Event → creates new instance per matching definition
- [x] Boundary signal events (interrupting + non-interrupting)

### Tests
- [x] Signal wakes all waiting instances for that signal name
- [x] Signal with no waiting instances → no error
- [x] Interrupting boundary signal cancels task
- [x] Non-interrupting boundary signal spawns parallel path
- [x] Normal task completion cleans up signal subscriptions

### Deliverable
Broadcast coordination between processes works.

---

## Phase 12 — Subprocess + Boundary Events

**Goal:** Processes can encapsulate sub-flows and react to events during task execution.

### Sub-phases

#### 12a — Embedded Subprocess
- Token enters subprocess → child execution created
- Child completes → parent resumes

#### 12b — Boundary Events (Timer + Message)
- Interrupting: cancel task, follow boundary flow
- Non-interrupting: parallel path, task continues

#### 12c — Event Subprocess
- Triggered by event within parent scope
- Interrupting or non-interrupting

### Tests
- Subprocess executes fully before parent continues
- Variables shared between parent and subprocess
- Boundary timer interrupts task correctly
- Non-interrupting boundary creates parallel path
- Nested subprocesses work

### Deliverable
Complex process structures with subprocesses work.

---

## Phase 13 — Inclusive Gateway

**Goal:** Multiple paths can be active simultaneously based on conditions.

### Tasks
- [ ] InclusiveGateway in parser
- [ ] Evaluate all conditions (not first-true)
- [ ] Create token per true condition
- [ ] Track which paths were activated at split
- [ ] Join waits for exactly activated paths

### Tests
- All conditions true → all paths run
- One condition true → one path runs
- Two of three conditions → two paths, merge waits for two
- No conditions true + no default → error

### Deliverable
Conditional parallel paths work correctly.

---

## Phase 14 — DMN Integration

**Goal:** Decision tables can be evaluated from Business Rule Tasks.

### Tasks
- [ ] DMN XML parser (decision tables only)
- [ ] Hit policy implementation (UNIQUE, FIRST, COLLECT)
- [ ] FEEL expression evaluator for cell conditions
- [ ] BusinessRuleTask → evaluate DMN → variables out
- [ ] DMN deployment (separate from BPMN)
- [ ] DMN versioning

### Tests
- Decision table evaluates correctly for all hit policies
- FEEL expressions in cells work
- Variables from DMN output become process variables
- DMN errors propagate as process errors

### Deliverable
Business rules modelled in DMN drive process routing.

---

## Phase 15 — Clustering + Observability

**Goal:** Multiple engine instances can run safely. System is observable.

### Tasks
- [ ] Verify FOR UPDATE SKIP LOCKED under load (already built in)
- [ ] Leader election for administrative tasks
- [ ] Prometheus metrics endpoint
- [ ] OpenTelemetry tracing
- [ ] Structured log format (JSON)
- [ ] Health endpoint enhanced (version, uptime, instance count)
- [ ] Graceful shutdown (drain in-flight work)

### Deliverable
Engine can run as multiple replicas safely.

---

## Phase 16 — Table Partitioning + Archival

**Goal:** Schema scales to tens of millions of instances without degrading query performance.

**Prerequisite:** Real production workload data to validate partition boundaries and retention windows.

### Background

At scale, `process_instances` and its five child tables (`executions`, `variables`, `tasks`, `jobs`, `event_subscriptions`) accumulate unbounded rows. Even with partial indexes, full-table vacuums and index bloat become bottlenecks. Range partitioning by time lets PostgreSQL prune irrelevant partitions and allows old partitions to be detached and archived without locking.

### Constraints

PostgreSQL declarative partitioning requires the partition key to be part of every primary key. This is a **breaking schema change** — all PKs and FKs across the six tables must be redesigned. Do not attempt to retrofit this onto a live populated schema without a migration window.

### Tasks

- [ ] Benchmark current schema at realistic data volumes (10M instances, 50M executions) to establish baseline
- [ ] Decide partition key and granularity (e.g. `process_instances.started_at` by month)
- [ ] Redesign PKs: `(id, started_at)` as composite PK on `process_instances`; propagate partition key to all child tables
- [ ] Evaluate FK trade-off: declarative FKs across partition boundaries are not supported — decide between application-enforced integrity vs. trigger-based checks
- [ ] Write migration: create partitioned tables, copy data, swap names
- [ ] Implement partition management: auto-create future partitions, detach+archive partitions beyond retention window
- [ ] Verify partial indexes survive partitioning (they must be recreated per partition or via `CREATE INDEX ... ON ONLY`)
- [ ] Load test fetch-and-lock and event correlation queries against partitioned schema

### Deliverable
Schema handles 10M+ instances with sub-10ms p99 on all hot-path queries.

---

## Definition of Done (per phase)

A phase is complete when:
- [ ] All tests in that phase pass
- [ ] All tests from previous phases still pass
- [ ] `cargo clippy -- -D warnings` clean
- [ ] `cargo fmt --check` clean
- [ ] CI pipeline green
- [ ] Phase spec marked complete in this document
