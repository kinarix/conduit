# Conduit — Incremental Build Plan

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
| 12 | Subprocess + Boundary | ✅ Complete | Embedded subprocess (12a), boundary events deferred |
| 13 | Inclusive Gateway | ✅ Complete | OR routing with selective join |
| 14 | DMN Integration | ✅ Complete | Decision tables |
| 15 | Clustering + Observability | ✅ Complete | Multi-node, metrics |
| 16 | Decision Table UI + Full FEEL | — | Visual DMN editor, full FEEL, DRD, all hit policies |
| 17 | External Task Long-Polling | — | LISTEN/NOTIFY-driven fetch-and-lock |
| 18 | Element Documentation + Attachments | — | Per-element rich-text docs and file attachments (PDF/DOC/XLS/PPT) |
| 19 | Instance Notes + Attachments | — | User-authored notes and attachments on running instances and steps |

> **Beyond the phase line:** several operational features have shipped alongside Phase 16 prep — HTTP push connector for `serviceTask`, encrypted secrets, per-version enable/disable, sequential instance counter, instances-list pagination, rename-across-versions, `businessRuleTask` version pinning, `scriptTask`, `sendTask`, and `boundaryEvent` of type error. See `README.md` for usage.

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

## Phase 6 — Exclusive Gateway ✓

**Goal:** Processes can branch based on variable conditions.

### Tasks
- [x] Variable passing on task completion — `POST /tasks/:id/complete` accepts `{ "variables": [...] }` body; writes to `variables` table before advancing
- [x] ExclusiveGateway in parser; read `conditionExpression` from sequence flows; mark default flow
- [x] FEEL expression evaluator (Phase 6.1, replaced Rhai) — sandboxed, variables injected into scope (full JSON type coverage: arrays, objects, nested)
- [x] Condition evaluation on sequence flows — first-true-wins routing
- [x] Default flow support
- [x] Error on no matching condition (no default) — instance marked `error`
- [x] Strict eval-error semantics — typo'd condition marks instance `error` (not silent default fallback)
- [x] Validator: gateways must have ≥1 outgoing flow; default flow ID must originate from the gateway

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

## Phase 12 — Embedded Subprocess

**Goal:** Processes can encapsulate sub-flows. Token enters, inner flow executes fully, token exits.

### Tasks

#### 12a — Embedded Subprocess (core) ✅
- [x] Parser: `SubProcess { sub_graph: ProcessGraph }` variant; recursively parse `subProcess` children
- [x] Engine: `find_element_graph` helper searches nested sub_graphs for element lookup
- [x] Engine: `SubProcess` arm — create subprocess execution, push inner start with subprocess scope
- [x] Engine: `EndEvent` arm — check outer_chain; if non-empty, count active siblings; if 0, complete subprocess + push outgoing in outer graph
- [x] Engine: resumption paths (`complete_user_task`, `complete_external_task`, `fire_timer_job`, `correlate_message`, `broadcast_signal`) use `find_element_graph` for correct graph resolution

#### 12b — Boundary Message Event
- [ ] Parser: `BoundaryMessageEvent { message_name, correlation_key_expr, attached_to, cancelling }` variant
- [ ] Engine: UserTask setup creates message subscription for each BoundaryMessageEvent attached to the task
- [ ] Engine: `deliver_message` handles BoundaryMessageEvent subscriptions (interrupting cancels task; non-interrupting spawns parallel path)
- [ ] Engine: normal task completion cleans up boundary message subscriptions

#### 12c — Event Subprocess (deferred)
- See spec in `docs/phases/PHASE-12-subprocess.md`

### Tests
- [x] Subprocess executes all inner steps before parent continues
- [x] Subprocess with UserTask pauses; inner task completion advances inner flow then exits subprocess
- [x] Variable written inside subprocess is visible to parent after exit
- [x] Variable written before subprocess is readable by inner ExclusiveGateway condition
- [x] Instance reaches `completed` state after subprocess exits
- [x] Nested subprocess works
- [ ] Boundary message event (interrupting) cancels task and follows boundary path (12b — deferred)
- [ ] Boundary message event (non-interrupting) spawns parallel path, task stays active (12b — deferred)

### Deliverable
Processes with embedded subprocesses and boundary message events work.

---

## Phase 13 — Inclusive Gateway

**Goal:** Multiple paths can be active simultaneously based on conditions.

### Tasks
- [x] InclusiveGateway in parser
- [x] Evaluate all conditions (not first-true)
- [x] Create token per true condition
- [x] Track which paths were activated at split
- [x] Join waits for exactly activated paths

### Tests
- All conditions true → all paths run
- One condition true → one path runs
- Two of three conditions → two paths, merge waits for two
- No conditions true + no default → error

### Deliverable
Conditional parallel paths work correctly.

---

## Phase 14 — DMN Integration

**Goal:** Decision tables can be deployed separately from BPMN and evaluated synchronously
from `BusinessRuleTask` elements. Output columns become process variables.

### Tasks
- [x] `migrations/006_decision_definitions.sql` — new table with versioning index
- [x] `src/dmn/mod.rs` — DMN XML parser → `Vec<DecisionTable>`
- [x] `src/dmn/feel.rs` — mini FEEL evaluator for input-entry cells (`-`, literals, unary comparisons, ranges, OR lists)
- [x] Hit policies: UNIQUE (default), FIRST, COLLECT, RULE_ORDER
- [x] `src/db/decision_definitions.rs` — deploy, get_latest, list
- [x] `src/db/models.rs` — `DecisionDefinition` struct
- [x] `src/api/decisions.rs` — `POST /api/v1/decisions`, `GET /api/v1/decisions`
- [x] `src/parser/mod.rs` — add `BusinessRuleTask { decision_ref }`, remove from unsupported list
- [x] `src/engine/mod.rs` — new arm: load decision → evaluate → write variables → advance
- [x] `src/api/instances.rs` — add `variables: Option<Vec<VariableInput>>` to `StartInstanceRequest`
- [x] New `EngineError` variants: `DmnParse`, `DmnFeel`, `DmnNotFound`, `DmnNoMatch`, `DmnMultipleMatches`
- [x] DMN fixtures: `risk_check.dmn`, `fee_tiers.dmn`, `collect_flags.dmn`, `multi_decision.dmn`
- [x] BPMN fixture: `business_rule_task.bpmn`
- [x] `tests/dmn_test.rs` — parser + FEEL unit tests (no DB)
- [x] `tests/decision_test.rs` — deployment + engine integration tests

### Tests
- Decision table parses correctly (inputs, outputs, rules, hit policy)
- Mini FEEL evaluator handles: `-`, string/number/bool literals, `>=`, `>`, `<=`, `<`, `=`, `!=`, inclusive/exclusive ranges, OR lists
- UNIQUE hit policy: errors on zero or multiple matches
- FIRST/COLLECT/RULE_ORDER hit policies work correctly
- Variables from DMN output become process variables
- Multiple `<decision>` elements in one DMN file → multiple versioned rows
- DMN errors propagate (no-match, multiple-match, not-found) → instance error state
- `BusinessRuleTask` BPMN element parsed with `decision_ref`

### Deliverable
Business rules modelled in DMN deployed via REST and evaluated transparently during process execution.

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

## Phase 16 — Decision Table UI + Full FEEL

**Goal:** Decision tables can be created and edited visually in the Conduit UI. Full FEEL standard library and all DMN hit policies are supported.

**Prerequisite:** Phase 15 complete and all tests passing.

### Tasks

- [ ] `src/dmn/feel.rs` — `not(...)` negation, `null` literal, full standard library (`sum`, `count`, `min`, `max`, `date(...)`, `string length`, etc.)
- [ ] `src/dmn/mod.rs` — `#[derive(serde::Serialize)]` on DMN types; all hit policies: UNIQUE, FIRST, COLLECT (with SUM/MIN/MAX/COUNT aggregators), RULE_ORDER, OUTPUT ORDER, ANY, PRIORITY
- [ ] `src/db/decision_definitions.rs` — add `get_latest(pool, org_id, key)` query
- [ ] `src/api/decisions.rs` — `GET /api/v1/decisions/:key` route returning full table JSON
- [ ] `ui/src/api/decisions.ts` — TypeScript API client (fetchDecisions, fetchDecision, deployDecision)
- [ ] `ui/src/pages/Decisions.tsx` — list page with DRD graph view
- [ ] `ui/src/pages/DecisionTableEditor.tsx` — spreadsheet grid editor + DMN XML serializer; supports all hit policies, expression-based output cells, DRD dependency wiring
- [ ] `ui/src/App.tsx` — routes `/decisions`, `/decisions/new`, `/decisions/:key/edit`
- [ ] `ui/src/components/Sidebar/FooterNav.tsx` — Decisions nav link

### Deliverable
Full visual DMN authoring: decision tables with all hit policies, full FEEL expressions, DRD dependency graphs, and round-trip XML serialization.

---

## Phase 17 — External Task Long-Polling

**Goal:** Replace short-poll worker traffic with long-polling on `fetch-and-lock`. Workers wait inside a single open HTTP request until a job for their topic appears (or the timeout elapses), eliminating the 500ms poll-per-worker tax without changing the worker contract.

**Prerequisite:** Phase 7. Independent of Phases 15 and 16.

### Background

Today `POST /api/v1/external-tasks/fetch-and-lock` is a short poll: workers loop on the endpoint, the SQL `FOR UPDATE SKIP LOCKED` returns whatever is ready, and the request returns immediately even when the queue is empty (`src/api/external_tasks.rs:57`, `src/db/jobs.rs::fetch_and_lock_many`). With N workers polling every K ms across M topics, idle traffic scales as `N · M / K`.

The Camunda 7 model — long-polling with a server-side `async_response_timeout` — keeps the worker contract intact (same endpoint, same JSON, same locking semantics) but parks the request on the server. The server is woken by Postgres `LISTEN/NOTIFY` whenever a row that could match becomes available: a new external_task job is inserted, an existing job's lock expires, or a job is unlocked after a `failure`.

### Tasks

- [ ] `migrations/0NN_external_task_notify.sql` — `LISTEN/NOTIFY` setup. Add a trigger on `jobs` for `INSERT` and `UPDATE OF locked_until, retry_count, type` that emits `pg_notify('external_task_ready', topic)` when `type = 'external_task'` and the row is fetchable (`locked_until IS NULL OR locked_until <= now()`). Trigger function must be `SECURITY INVOKER` and cheap (no heavy joins).
- [ ] `src/db/jobs.rs` — extend `fetch_and_lock_many` callers, or add a sibling `fetch_and_lock_many_long_poll(pool, worker_id, lock_secs, topic, max_jobs, wait_secs)` that:
  1. Tries `fetch_and_lock_many` once.
  2. If empty and `wait_secs > 0`, acquires a dedicated `PgConnection` from the pool, issues `LISTEN external_task_ready`, then `tokio::select!`s between `connection.notifications().recv()` filtered by topic and a `tokio::time::sleep(wait_secs)`.
  3. On wake, retries the fetch once (any payload — there may be contention, an empty result is fine).
  4. Always `UNLISTEN` and return the connection to the pool.
- [ ] `src/api/external_tasks.rs::FetchAndLockRequest` — add `async_response_timeout_secs: Option<i64>` (clamped to `[0, 60]`, default `0` to preserve current short-poll behaviour). Route to long-poll path when `> 0`.
- [ ] Engine wake-ups for re-fetchable rows. Audit every site that makes a job fetchable and ensure the trigger fires:
  - Insert (already covered by trigger on INSERT)
  - `fail_external_task` decrements retries and clears lock — covered by UPDATE trigger
  - Lock-expiry sweep (background unlock job, if any) — covered by UPDATE trigger
- [ ] `src/state.rs` — confirm `PgPool` size is sufficient. Long-pollers hold a connection for up to `wait_secs` each; document the relationship `max_long_pollers + ambient_query_load ≤ pool_size`. Consider exposing `EXTERNAL_TASK_LONG_POLL_MAX_WAIT_SECS` as a config bound (default 30, hard cap 60).
- [ ] Graceful shutdown — on `SIGTERM`, signal in-flight long-pollers to return early (200 with empty array) so workers reconnect against the next replica.
- [ ] Metrics (defer to Phase 15 if not yet built): counters for `long_poll_woken_by_notify`, `long_poll_timed_out`, `long_poll_woken_but_lost_race` (notified but no row claimed on retry).

### Tests

- [ ] Long-poll returns immediately when a job already exists (no waiting).
- [ ] Long-poll with empty queue + `wait_secs=2` returns empty array after ~2s.
- [ ] Long-poll waiting on topic `A` is woken when a job for topic `A` is inserted; returns within ~100ms.
- [ ] Long-poll waiting on topic `A` is **not** woken (and times out) when a job for topic `B` is inserted.
- [ ] Two long-pollers on the same topic — one job inserted — exactly one returns the job, the other waits or times out (no double-delivery, no panic).
- [ ] Long-poll wakes when an existing job's lock expires past `now()` (simulate by updating `locked_until`).
- [ ] Long-poll wakes when a `failure` call decrements retries and clears the lock.
- [ ] `async_response_timeout_secs = 0` preserves Phase 7 behaviour byte-for-byte.
- [ ] `async_response_timeout_secs > 60` clamped to 60 (no silent multi-minute holds).
- [ ] Pool exhaustion test: long-pollers ≥ pool size — handler returns 503 or short-polls instead of deadlocking other queries.
- [ ] Graceful shutdown drains in-flight long-pollers within shutdown grace period.

### Non-goals

- Replacing REST with WebSockets, SSE, or gRPC streaming. Workers stay HTTP/JSON.
- Server-side push of variable updates. Only job-availability notifications.
- Cross-engine notification routing — `LISTEN/NOTIFY` is per-Postgres-cluster, which is exactly the boundary we want.

### Deliverable

Workers configured with `async_response_timeout_secs > 0` see job latency drop from `≤ poll_interval` to `≤ NOTIFY round-trip` (~tens of ms) while idle DB load drops to near zero. Existing workers continue to short-poll without code changes.

---

## Phase 18 — Element Documentation + Attachments

**Goal:** Every BPMN element that supports `<bpmn:documentation>` (and every DMN decision) can carry a rich-text description and file attachments, edited via a modal in the UI modeller. Documentation round-trips through BPMN XML so processes stay portable; attachments are a Conduit-side store keyed by `(definition_id, element_id)`.

**Prerequisite:** Phase 5 + existing UI modeller. Independent of Phases 15–17.

### Background

Today an element's only metadata in the modeller is its `id`, `name`, and a few extension attributes. There's nowhere to capture *why* a step exists, link to a runbook, or attach the spec PDF a business analyst handed over. BPMN 2.0 reserves `<documentation>` as an optional child on virtually every flow element — supporting it is table-stakes for a modeller. Attachments are not part of BPMN; they live in Conduit's own store and are linked by element ID.

### Scope

#### Documented elements (anywhere `<bpmn:documentation>` is permitted in BPMN 2.0)
- `Process`, `SubProcess`
- All events: `StartEvent`, `EndEvent`, `IntermediateCatchEvent`, `IntermediateThrowEvent`, `BoundaryEvent`
- All tasks: `UserTask`, `ServiceTask`, `ReceiveTask`, `BusinessRuleTask`, `SendTask`, `ScriptTask`, `ManualTask`
- All gateways: `Exclusive`, `Parallel`, `Inclusive`, `Event`, `Complex`
- `SequenceFlow`, `MessageFlow`, `DataObject`, `Lane`, `Participant`
- DMN: `Decision` (separate API surface; same UX)

#### Out of scope
- Per-revision documentation history (latest wins per definition version)
- Inline images embedded as base64 in the rich text body — must be uploaded as attachments
- Comments/threading on documentation
- Cross-element linking (Phase 19+ if needed)
- External-store backends (S3, MinIO) — deferred; bytea only for now

### Data Model

#### `element_documentation` (new table)

```sql
CREATE TABLE element_documentation (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    definition_id   UUID NOT NULL,            -- references process_definitions or decision_definitions
    definition_kind TEXT NOT NULL CHECK (definition_kind IN ('process', 'decision')),
    element_id      TEXT NOT NULL,            -- BPMN/DMN element ID (string, not UUID)
    body_html       TEXT NOT NULL DEFAULT '', -- sanitized HTML
    body_text       TEXT NOT NULL DEFAULT '', -- plain-text projection for search
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_by      UUID REFERENCES users(id),
    UNIQUE (definition_id, definition_kind, element_id)
);
CREATE INDEX idx_element_documentation_def
    ON element_documentation (definition_id, definition_kind);
```

#### `element_attachments` (new table)

```sql
CREATE TABLE element_attachments (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    definition_id   UUID NOT NULL,
    definition_kind TEXT NOT NULL CHECK (definition_kind IN ('process', 'decision')),
    element_id      TEXT NOT NULL,
    filename        TEXT NOT NULL,
    mime_type       TEXT NOT NULL,
    size_bytes      BIGINT NOT NULL,
    sha256          BYTEA NOT NULL,           -- dedup key
    content         BYTEA NOT NULL,           -- raw bytes; bytea storage for now
    uploaded_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    uploaded_by     UUID REFERENCES users(id)
);
CREATE INDEX idx_element_attachments_element
    ON element_attachments (definition_id, definition_kind, element_id);
CREATE INDEX idx_element_attachments_sha
    ON element_attachments (sha256);
```

Notes:
- `definition_id` is intentionally **not** a foreign key — `process_definitions` and `decision_definitions` are separate tables; the `definition_kind` discriminator is checked by the API layer.
- Cascade behaviour on definition delete: on hard-delete of a definition, attachments and documentation are deleted in the same transaction (handled in `db::process_definitions::delete`).
- Element-rename behaviour: when a deployment overwrites a definition (new version), documentation/attachments are **not** auto-migrated. They're keyed to the version. Optional migration tool can copy from previous version on deploy if `inherit_docs=true` is passed.

### Configuration (env)

```
ATTACHMENT_MAX_SIZE_BYTES=26214400        # 25 MiB default
ATTACHMENT_MAX_TOTAL_PER_ELEMENT_BYTES=104857600  # 100 MiB total per element
ATTACHMENT_ALLOWED_MIME=application/pdf,application/msword,application/vnd.openxmlformats-officedocument.wordprocessingml.document,application/vnd.ms-excel,application/vnd.openxmlformats-officedocument.spreadsheetml.sheet,application/vnd.ms-powerpoint,application/vnd.openxmlformats-officedocument.presentationml.presentation,text/plain,text/markdown,image/png,image/jpeg,image/gif
```

MIME type is validated server-side from sniffed content (via `infer` crate or magic bytes), not just the request header — uploaded files are not trusted.

### API

```
GET    /api/v1/{kind}/{definition_id}/elements/{element_id}/documentation
PUT    /api/v1/{kind}/{definition_id}/elements/{element_id}/documentation
DELETE /api/v1/{kind}/{definition_id}/elements/{element_id}/documentation

GET    /api/v1/{kind}/{definition_id}/elements/{element_id}/attachments
POST   /api/v1/{kind}/{definition_id}/elements/{element_id}/attachments     (multipart)
GET    /api/v1/attachments/{attachment_id}                                   (download stream)
DELETE /api/v1/attachments/{attachment_id}
```

Where `{kind}` ∈ `processes` | `decisions`.

`PUT` documentation accepts:
```json
{ "body_html": "<p>…</p>", "body_text": "…" }
```
Server sanitizes `body_html` (`ammonia` crate, allowlist: standard formatting + safe inline elements; no `<script>`, no `on*` handlers, no `style` attributes, no inline data: URIs except for the small image case). `body_text` is recomputed server-side from sanitized HTML — the client value is advisory.

`POST` attachment accepts `multipart/form-data` with a single `file` part. Server response includes `id`, `filename`, `mime_type`, `size_bytes`, `download_url`. Deduplication: if `sha256` already exists for this element, return the existing row (idempotent re-upload).

`GET /api/v1/attachments/{id}` streams bytes with `Content-Disposition: attachment; filename="…"` and the original MIME type. Range requests supported for large files (PDFs in browser viewer).

### BPMN XML round-trip

On deployment (`POST /api/v1/deployments`):
1. Parser walks the XML and harvests every `<bpmn:documentation>` child it finds (one per element).
2. For each, upsert into `element_documentation` with `body_text = documentation text content` and `body_html = wrapped <p> of the same`.
3. On `GET /api/v1/deployments/{id}/xml`, the engine re-emits XML with `<bpmn:documentation>` populated from `body_text` (HTML stripped to plain text — round-trip is plain text, rich content is Conduit-only).

This means: BPMN XML carries plain text only (portable). Rich HTML and attachments are Conduit additions, lost on export but preserved on Conduit-internal redeploy if `inherit_docs=true`.

### UI

#### Component layout
- `ui/src/components/bpmn/DocumentationModal.tsx` — new modal component
- `ui/src/components/bpmn/BpmnProperties.tsx` — add a "Documentation" button next to id/name fields that opens the modal for the currently selected element
- `ui/src/api/documentation.ts` — fetch wrappers for the four documentation/attachment endpoints

#### Editor library
TipTap (`@tiptap/react` + `@tiptap/starter-kit` + `@tiptap/extension-link` + `@tiptap/extension-image`). Headless, MIT, ProseMirror under the hood, ~80kb gzip with the kit. Output: HTML.

Toolbar: bold, italic, underline, strike, h1/h2/h3, bullet list, ordered list, blockquote, code block, inline code, link, undo/redo. Image insertion goes through the attachment upload flow (uploaded image → embed via `download_url`).

#### Modal UX
- Two-pane layout: editor on top, attachment list below (drag-and-drop drop zone or "+ Add file" button).
- Attachment row: filename, size, type icon (PDF/DOC/XLS/PPT/IMG/TXT), upload date, download button, delete button (confirm).
- Save button persists both the editor body and any pending attachment uploads in one shot.
- Cancel discards in-memory edits — no autosave.
- Loading and error states for upload (progress bar per file).
- Max-size feedback before upload (client-side check against `ATTACHMENT_MAX_SIZE_BYTES` exposed via `/api/v1/config`).

### Tests

#### Backend
- [ ] `documentation_round_trip` — PUT body_html, GET returns sanitized body_html and recomputed body_text.
- [ ] `documentation_html_sanitization` — `<script>alert(1)</script>` stripped; `<p onclick="…">` attributes stripped; `<a href="javascript:…">` neutralised; safe formatting (`<b>`, `<a href="https://…">`, `<ul>`, `<code>`) preserved.
- [ ] `documentation_extracted_on_deploy` — BPMN with `<bpmn:documentation>foo</bpmn:documentation>` on a userTask → row in `element_documentation` after deploy.
- [ ] `documentation_export_round_trips` — Deploy BPMN, edit doc via API, re-export XML → `<bpmn:documentation>` contains plain text from `body_text`.
- [ ] `attachment_upload_pdf` — POST 1 MiB PDF, GET attachment list returns the row, GET attachment stream returns bytes with correct Content-Type.
- [ ] `attachment_rejects_oversized` — Upload > `ATTACHMENT_MAX_SIZE_BYTES` → 413.
- [ ] `attachment_rejects_disallowed_mime` — Upload `.exe` (or content-sniff says `application/x-msdownload`) → 415.
- [ ] `attachment_dedup_by_sha256` — Same file uploaded twice for same element → second call returns first row's ID, only one row in DB.
- [ ] `attachment_cascade_on_definition_delete` — Hard-delete definition → attachment rows removed.
- [ ] `attachment_per_element_total_cap` — Uploads exceeding `ATTACHMENT_MAX_TOTAL_PER_ELEMENT_BYTES` rejected with 413.
- [ ] `unknown_element_id_404` — Documentation/attachment ops against an element ID not present in any deployed version of that definition → 404.

#### UI (component tests via Vitest + React Testing Library)
- [ ] Modal opens with current documentation pre-filled.
- [ ] Editing body and saving fires PUT with sanitized HTML.
- [ ] Drag-and-drop a PDF into the drop zone → upload progress → row appears.
- [ ] Attempting to drop an `.exe` shows "file type not allowed" inline error.
- [ ] Delete attachment requires confirm; on confirm, row removed and DELETE called.
- [ ] Cancel discards unsaved body and pending uploads.

### Deliverable

Every BPMN element (and DMN decision) has a "Documentation" button in the modeller. Clicking opens a modal with a TipTap rich-text editor and an attachment panel. Process designers can write specs, embed images, and link spec PDFs / Word docs / Excel sheets / PowerPoint decks directly to the elements they describe. Documentation round-trips through BPMN XML as plain text; rich content and attachments persist in Conduit's database.

---

## Phase 19 — Instance Notes + Attachments (User-Driven)

**Goal:** End users (task assignees, supervisors, observers) can attach rich-text notes and files to a running process instance — either to the instance as a whole or to a specific step (element or task). Unlike Phase 18 (design-time, one body per definition), Phase 19 is a runtime, append-only timeline scoped to one execution.

**Prerequisite:** Phase 5 + Phase 5.5. Reuses the editor, sanitizer, MIME allowlist, and size caps from Phase 18.

### Why this is separate from Phase 18

| | Phase 18 — Element Documentation | Phase 19 — Instance Notes |
|---|---|---|
| Author | Process designer | End user (task worker, supervisor) |
| Scope | `(definition, element_id)` | `(instance, optional element_id, optional task_id)` |
| Cardinality | One body per element, updated in place | Many notes per instance, append-only timeline |
| Lifetime | Survives across instances; lost on definition delete | Tied to the instance; lost on instance archival |
| Purpose | "What this step *should* do" | "What happened in *this* run" |
| Visibility | Modeller | Task UI + instance detail UI |

The two should be visually distinct in the UI: design-time docs are the "spec" panel (read-only for end users), instance notes are the "activity" panel (writeable by users with task access).

### Scope

#### Captured note targets
- Whole instance: `(instance_id, element_id=NULL, task_id=NULL)` — e.g. "escalated to manager"
- Specific element across the instance lifetime: `(instance_id, element_id='task_review', task_id=NULL)` — survives even after the task closes
- Specific task occurrence: `(instance_id, element_id='task_review', task_id=<task_uuid>)` — bound to one specific execution of that user task (matters for loops and re-entry)

The UI presents a single timeline; the discriminators are filters/badges, not separate surfaces.

#### Out of scope
- Threaded replies / mentions (flat timeline only)
- Edit history (notes are append-only; an author can soft-delete their own note within a configurable grace window, otherwise it stays)
- Real-time collaborative editing
- Cross-instance search (deferred to a search-indexing phase)
- @-notifications / email digests (deferred — needs a notifier phase)

### Data Model

#### `instance_notes` (new table)

```sql
CREATE TABLE instance_notes (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    instance_id     UUID NOT NULL REFERENCES process_instances(id) ON DELETE CASCADE,
    element_id      TEXT,
    task_id         UUID REFERENCES tasks(id) ON DELETE SET NULL,
    body_html       TEXT NOT NULL,
    body_text       TEXT NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by      UUID NOT NULL REFERENCES users(id),
    deleted_at      TIMESTAMPTZ,                          -- soft-delete by author within grace window
    pinned          BOOLEAN NOT NULL DEFAULT FALSE        -- pinned notes float to top of timeline
);
CREATE INDEX idx_instance_notes_instance       ON instance_notes (instance_id, created_at DESC);
CREATE INDEX idx_instance_notes_instance_elem  ON instance_notes (instance_id, element_id) WHERE element_id IS NOT NULL;
CREATE INDEX idx_instance_notes_task           ON instance_notes (task_id) WHERE task_id IS NOT NULL;
```

#### `instance_attachments` (new table)

```sql
CREATE TABLE instance_attachments (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    instance_id     UUID NOT NULL REFERENCES process_instances(id) ON DELETE CASCADE,
    element_id      TEXT,
    task_id         UUID REFERENCES tasks(id) ON DELETE SET NULL,
    note_id         UUID REFERENCES instance_notes(id) ON DELETE SET NULL,  -- optional: attachment authored alongside a note
    filename        TEXT NOT NULL,
    mime_type       TEXT NOT NULL,
    size_bytes      BIGINT NOT NULL,
    sha256          BYTEA NOT NULL,
    content         BYTEA NOT NULL,
    uploaded_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    uploaded_by     UUID NOT NULL REFERENCES users(id)
);
CREATE INDEX idx_instance_attachments_instance ON instance_attachments (instance_id, uploaded_at DESC);
CREATE INDEX idx_instance_attachments_task     ON instance_attachments (task_id) WHERE task_id IS NOT NULL;
CREATE INDEX idx_instance_attachments_note     ON instance_attachments (note_id) WHERE note_id IS NOT NULL;
```

Notes:
- `ON DELETE CASCADE` from `process_instances` — when an instance is hard-deleted (archival/GDPR), notes and attachments go with it. History service should snapshot before delete.
- `ON DELETE SET NULL` from `tasks` — task rows are deleted when the task closes, but the note about that task should remain attached to the instance (with `task_id` becoming NULL). The UI shows "task <element_id> (closed)" once `task_id` is NULL.
- Soft-delete grace window: configurable env `NOTE_EDIT_GRACE_SECONDS=300` — author can soft-delete within 5 minutes; after that, notes are immutable.

### Configuration (env)

Reuses Phase 18's MIME allowlist and per-file size cap. Adds:

```
NOTE_EDIT_GRACE_SECONDS=300
INSTANCE_ATTACHMENT_MAX_TOTAL_BYTES=524288000      # 500 MiB total per instance
```

### API

```
GET    /api/v1/process-instances/{instance_id}/notes
       ?element_id=<elem>&task_id=<uuid>           # optional filters
POST   /api/v1/process-instances/{instance_id}/notes
DELETE /api/v1/notes/{note_id}                     # author-only, within grace window

GET    /api/v1/process-instances/{instance_id}/attachments
       ?element_id=<elem>&task_id=<uuid>
POST   /api/v1/process-instances/{instance_id}/attachments     (multipart)
GET    /api/v1/instance-attachments/{attachment_id}            (download stream)
DELETE /api/v1/instance-attachments/{attachment_id}            (uploader-only)

POST   /api/v1/tasks/{task_id}/notes                # convenience: scopes element_id + task_id automatically
POST   /api/v1/tasks/{task_id}/attachments
```

`POST .../notes` body:
```json
{
  "body_html": "<p>…</p>",
  "body_text": "…",
  "element_id": "task_review",       // optional
  "task_id": "<uuid>",               // optional
  "pinned": false,                   // optional
  "attachment_ids": ["<uuid>", "..."] // optional: link existing instance attachments to this note
}
```

`POST /api/v1/tasks/{task_id}/complete` is **extended** to optionally include:
```json
{
  "variables": [...],
  "completion_note": { "body_html": "...", "body_text": "..." },
  "attachment_ids": ["<uuid>", ...]
}
```
The completion_note becomes a pinned note for `(instance_id, element_id, task_id)` so the task's outcome rationale is captured at the moment of completion. Attachments uploaded earlier in the session can be linked here.

Sanitization, MIME sniffing, dedup-by-sha256 — same rules as Phase 18.

### Authorization

- Read: any user in the instance's org with read access to the instance.
- Write: any user in the instance's org. Where task RBAC is enforced (later phase), users restricted to their assigned tasks see only notes/attachments tied to their tasks.
- Delete: only the original author, only within `NOTE_EDIT_GRACE_SECONDS`.
- Pin/unpin: any user with write access (audit-logged).

(Auth enforcement scaffolding is whatever exists at the time Phase 19 lands — Phase 19 doesn't introduce new auth machinery.)

### Audit Trail Integration

Every note creation, deletion, pin/unpin, and attachment upload writes a row into `execution_history` with `event_type` ∈ `note_created`, `note_deleted`, `note_pinned`, `attachment_uploaded`, `attachment_deleted`. Payload includes the `note_id` / `attachment_id`, `created_by`, and `element_id` / `task_id` so the timeline reconstructs from history alone if notes are soft-deleted.

### UI

#### Components
- `ui/src/components/instance/InstanceTimeline.tsx` — append-only timeline component, used on instance detail page; ordered by `created_at DESC`, pinned notes float to top.
- `ui/src/components/instance/NoteComposer.tsx` — TipTap editor + drop zone + "post" button.
- `ui/src/components/task/TaskNotesPanel.tsx` — embedded in task detail UI; pre-scopes `element_id` + `task_id` so notes are bound to the right context.
- `ui/src/components/instance/AttachmentList.tsx` — shared between instance and task panels; same UX as Phase 18.
- `ui/src/api/instanceNotes.ts` + `ui/src/api/instanceAttachments.ts` — fetch wrappers.

#### Behaviour
- Drag-and-drop on the timeline drops uncategorised attachments at the instance level.
- Drag-and-drop on a task panel scopes attachments to that task.
- Note composer accepts in-line attachment uploads (paste a PDF, drop an image): file is uploaded immediately, returned attachment ID is stashed, then linked via `attachment_ids` when the note is posted.
- Soft-deleted notes render as "[deleted by author]" with the timestamp; admin/audit view can show the original body sourced from `execution_history`.
- "Add completion note" prompt appears on task complete dialog (skippable).

### Tests

#### Backend
- [ ] `note_create_round_trip` — POST note, GET timeline returns it with sanitized HTML.
- [ ] `note_filtered_by_element` — POST 3 notes (instance-level, element A, element B); GET with `element_id=A` returns only the element-A note plus instance-level? **Decision:** filter is exact-match — instance-level notes only returned when no `element_id` filter passed.
- [ ] `note_filtered_by_task` — same, with `task_id`.
- [ ] `note_soft_delete_within_grace` — author DELETE within grace window → row marked deleted, timeline returns "[deleted]" placeholder.
- [ ] `note_soft_delete_after_grace` — DELETE after `NOTE_EDIT_GRACE_SECONDS` → 403.
- [ ] `note_soft_delete_other_user` — non-author DELETE → 403.
- [ ] `note_html_sanitized` — same allowlist as Phase 18.
- [ ] `attachment_uploaded_to_instance` — POST 1 MiB PDF, listed in attachments.
- [ ] `attachment_linked_to_note` — POST attachment, then POST note with `attachment_ids=[…]` → note response includes attachment metadata.
- [ ] `attachment_dedup_within_instance` — same SHA twice on same instance → idempotent.
- [ ] `attachment_per_instance_total_cap` — uploads exceeding `INSTANCE_ATTACHMENT_MAX_TOTAL_BYTES` rejected with 413.
- [ ] `task_complete_with_note` — POST `/tasks/:id/complete` with `completion_note` → task advances and pinned note exists for that task.
- [ ] `task_close_preserves_notes` — task completed → tasks row deleted → notes still queryable, `task_id` is NULL.
- [ ] `instance_delete_cascades` — hard-delete instance → notes + attachments removed; `execution_history` rows preserved as audit fallback.
- [ ] `audit_history_records_note_lifecycle` — note_created / note_deleted / attachment_uploaded events all land in `execution_history`.

#### UI
- [ ] Timeline renders newest-first with pinned items pinned.
- [ ] Composer attaches in-line file upload before submit; failed upload prevents submit.
- [ ] Soft-delete button visible only to author and only within grace window.
- [ ] Task complete dialog shows optional "completion note" field that posts on confirm.

### Deliverable

Every running process instance carries a user-authored timeline. Workers can drop notes and attachments on the instance, on a step, or on a specific task; supervisors can pin critical context to the top; the task completion dialog captures rationale at the moment of decision. Together with Phase 18, the modeller's "what this should do" and the operator's "what actually happened" sit side-by-side in the UI.

---

## Definition of Done (per phase)

A phase is complete when:
- [ ] All tests in that phase pass
- [ ] All tests from previous phases still pass
- [ ] `cargo clippy -- -D warnings` clean
- [ ] `cargo fmt --check` clean
- [ ] CI pipeline green
- [ ] Phase spec marked complete in this document
