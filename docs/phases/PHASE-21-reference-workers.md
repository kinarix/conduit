# Phase 21 — Reference Workers (In-Tree, Polyglot)

## Status
Rust SDK MVP shipped (2026-05-08). Go / Python / Node / Java SDK scaffolds in progress.

## Prerequisites
Phase 7 (external-task API), Phase 17 (external-task long polling). Driven by [ADR-008](../adr/ADR-008-engine-stays-pure-bpmn.md).

## Goal
Ship reference worker SDKs so "the engine doesn't have a REST connector" doesn't translate to "I have to write all integration code from scratch." Workers live in-tree at [`workers/`](../../workers/), one subdirectory per language, each an independent build (its own `Cargo.toml` / `go.mod` / `pyproject.toml` / `package.json` / `pom.xml`). Versioned together with the engine for now; release cadence may diverge later.

The Rust SDK is the **reference**: Go / Python / Node / Java SDKs mirror its shape and conform to the wire contract documented once in [`workers/PROTOCOL.md`](../../workers/PROTOCOL.md).

These workers are reference implementations, not a runtime customers must use — they exist to make the worker pattern as ergonomic as a connector dropdown would have been.

## Scope

Each language ships at minimum a library with a `Client` (external-task API binding) and a `Handler` registration mechanism (trait / interface / decorator). An `http-worker` binary in each language is desirable for parity but library-only is acceptable as the starting scaffold.

### Layout: `workers/`

```
workers/
├── README.md                       ← polyglot index
├── PROTOCOL.md                     ← language-agnostic wire contract every SDK conforms to
├── docs/
│   └── idempotency-store.md        ← Postgres dedupe table schema (language-agnostic)
├── rust/                           ← reference SDK (Rust)
│   ├── Cargo.toml                  ← workspace manifest (independent of engine workspace)
│   └── crates/
│       ├── conduit-worker/         ← library: Client + Handler trait + run loop
│       ├── http-worker/            ← binary: http.call (replaces <conduit:http>)
│       ├── csv-worker/             ← scaffolded: csv.read / csv.write
│       ├── gcs-worker/             ← scaffolded: gcs.read / gcs.write
│       └── kafka-produce-worker/   ← scaffolded: kafka.produce
├── go/                             ← Go SDK (planned next)
│   ├── go.mod
│   ├── conduitworker/              ← library
│   └── cmd/http-worker/            ← binary
├── python/                         ← Python SDK (library scaffold)
├── node/                           ← Node SDK (library scaffold)
└── java/                           ← Java SDK (library scaffold)
```

Inbound trigger sidecars (`kafka-consumer`, `webhook-receiver`) still planned; they will live under each language's tree once the outbound SDKs land.

The MVP that gates Phase 20 connector removal is just `rust/crates/conduit-worker/` + `rust/crates/http-worker/`. Other handlers and other-language SDKs ship incrementally.

### Ergonomic API per language

Each SDK exposes the same conceptual API — `Client`, `Handler`, `Runner` — surfaced through the host language's idiomatic registration mechanism:

| Language | Registration |
|---|---|
| Rust | `#[handler(topic = "http.call")]` proc-macro on a struct (sugar over the `Handler` trait impl) |
| Java | `@TaskHandler(topic = "http.call")` annotation on a class (Spring-style discovery, optional) |
| Python | `@handler(topic="http.call")` decorator on an async function |
| Node (TypeScript) | `@Handler({ topic: "http.call" })` decorator on a class, or `defineHandler({ topic, handle })` builder |
| Go | `worker.Register("http.call", handlerFn)` — Go has no annotations; struct + registration is the idiomatic path |

The macro/decorator/annotation form is **sugar**, not a replacement for the trait/interface — every SDK keeps the underlying type usable directly so users can opt out of the framework-style API.

### Worker handlers shipped

| Handler | Topic convention | Worker config | Replaces |
|---|---|---|---|
| `http` | `http.call` | URL, method, auth (env or secret), headers, body template, response mapping | `<conduit:http>` |
| `csv.read` / `csv.write` | `csv.read` / `csv.write` | path, delimiter, has_header | (would-have-been Phase 21 connector) |
| `gcs.read` / `gcs.write` | `gcs.read` / `gcs.write` | bucket, object, GCS credentials path | (would-have-been Phase 22 connector) |
| `kafka.produce` | `kafka.produce` | brokers, topic, key, headers, payload from variables | (would-have-been Phase 24 connector) |

### Trigger sidecars (inbound)

| Sidecar | Purpose | Engine API used |
|---|---|---|
| `kafka-consumer` | Subscribe to a topic, translate each message into a `POST /messages/correlate` (or `POST /process-instances`) | `/api/v1/messages/correlate`, `/api/v1/process-instances` |
| `webhook-receiver` | HTTP endpoint that translates inbound webhooks the same way | Same |

These run as separate processes / containers, configured by environment, not by BPMN extensions.

### BPMN authoring pattern

Every worker-driven service task uses the existing extension shape:

```xml
<bpmn:serviceTask id="post_order">
  <bpmn:extensionElements>
    <conduit:taskTopic>http.call</conduit:taskTopic>
    <conduit:taskConfig>
      <conduit:url>https://api.example.com/orders</conduit:url>
      <conduit:method>POST</conduit:method>
      <conduit:authSecretRef>orders_api_token</conduit:authSecretRef>
    </conduit:taskConfig>
  </bpmn:extensionElements>
</bpmn:serviceTask>
```

`conduit:taskConfig` is parsed by the **worker**, not the engine — the engine treats it as opaque payload attached to the task and delivers it via fetch-and-lock. This is the existing Phase 7 contract; nothing new is added on the engine side.

## Durable Execution Semantics

The end-to-end goal is a system where any worker process can crash at any moment without corrupting state or losing work. This is achieved in two cleanly separated layers:

### Layer 1 — Orchestration durability (engine, already shipped)
- Every BPMN state change is persisted in PostgreSQL inside one transaction.
- `jobs.locked_until` gives every worker fetch a TTL. A crashed worker's task is reclaimed by another worker (or the same one on restart) once the lock expires.
- Long polling (Phase 17) shrinks the time between completion and the next fetch without weakening these semantics.

The engine alone gives you "every BPMN step happens at-least-once and exactly the right number of times overall." This layer requires no work in the reference workers — it's the contract of the external-task API.

### Layer 2 — Side-effect idempotency (per worker handler)
At-least-once delivery only translates into "feels exactly-once" if each worker's side effects are idempotent under retry. The reference workers ship the standard pattern per handler:

| Handler | Idempotency strategy |
|---|---|
| `http` | `Idempotency-Key` header derived from `task_id` (RFC draft + Stripe convention). Worker maintains a small dedupe table keyed by `(task_id, attempt)` so a successful response is replayed on retry instead of re-issued. GET/HEAD pass through unchanged |
| `csv.read` | Naturally idempotent |
| `csv.write` | Write to `{path}.tmp.{task_id}`, then atomic rename. Retry overwrites the temp file safely; the final rename is idempotent (target either doesn't exist or already holds the expected content) |
| `gcs.read` | Pin to a generation when consistency matters; otherwise naturally idempotent |
| `gcs.write` | `ifGenerationMatch=0` for create-only writes; generation-match for updates. Object versioning enabled on the bucket gives a safety net for accidental overwrites |
| `kafka.produce` | `enable.idempotence=true` on the client (in-session dedupe). For cross-restart dedupe, deterministic message key derived from `task_id`, plus a transactional producer when the broker supports it |
| `kafka.consume` (trigger) | Manual offset commit *after* `POST /messages/correlate` returns 2xx. Engine-side correlation is documented as "the BPMN handles duplicate messages" — same contract as any at-least-once message bus |
| `webhook-receiver` (trigger) | Use the webhook's delivery ID as the correlation-key suffix. Duplicate deliveries from the source map to the same engine call |

Each handler's README in `workers/<lang>/` documents the strategy explicitly so customers writing their own handlers know what shape to follow.

### Idempotency-key store
Several handlers (notably `http`) need a small piece of persistent state: a table mapping `(task_id, attempt)` → response. The reference workers default to a Postgres table next to the engine's database; a Redis adapter is provided for fleets that prefer it. **One store per worker fleet, not per process** — durability of the dedupe table is what makes a handler safe across worker restarts. Store schema and rotation policy are documented in [`workers/docs/idempotency-store.md`](../../workers/docs/idempotency-store.md) (language-agnostic — same table is read/written by every SDK).

### Step-level durability inside a single task
If a single Conduit task is itself a long-running, multi-step computation that needs replayable state (Temporal/Restate territory), customers compose two durable runtimes themselves: the worker becomes a thin adapter that starts a workflow in the durable runtime, polls until terminal, and completes the Conduit task. We do **not** ship a reference adapter for this — Conduit explicitly stays out of durable-workflow territory and we don't want a reference implementation that implies otherwise.

### What's intentionally out of scope
- Step-level checkpointing primitives in the engine.
- A built-in idempotency-key store served by the engine (it would couple worker dedupe state to engine deploys; bad).
- Exactly-once delivery semantics on the engine's external-task API (the BPMN-level "happens once" guarantee comes from idempotent side effects, not from the transport).

---

## Excluded
- Engine-side changes. Reference workers are downstream consumers of the existing external-task API.
- A reference Temporal/Restate adapter. Customers who want step-level durability inside a single task can compose that themselves; we don't ship a reference for it because it would imply Conduit owns the pattern.
- A "marketplace" or auto-discovery mechanism. Workers are deployed by the customer alongside the engine.
- Server-language SDKs beyond Rust + Go + Python + Node + Java for this phase. (Other languages — .NET, Ruby, Elixir — can come later if there's demand.)

## Test Plan (under `workers/`)
- Each handler has integration tests against:
  - A live Conduit engine (via testcontainers driving the engine binary).
  - The relevant external dependency (mockito for `http`, fake-gcs-server for `gcs`, redpanda or kafka container for `kafka.produce`).
- A smoke test that runs the engine + worker + a tiny BPMN end-to-end for each handler.

## Engine-side work (this repo, outside `workers/`)
- Add a "Workers" section to `README.md` linking to `workers/`.
- Add a section to `docs/ARCHITECTURE.md` clarifying the worker boundary (referencing ADR-008).
- Optional: a `docs/WORKERS.md` with the quick-start and a snippet of each handler.

## Verification Checklist
- [x] `workers/` directory contains per-language SDK trees + a polyglot README
- [x] `workers/PROTOCOL.md` documents the wire contract every SDK conforms to
- [x] Conduit `README.md` links to `workers/`
- [x] `docs/MIGRATION.md` (Phase 20) references the reference HTTP worker (Rust) by path
- [x] At least one example BPMN in this repo's `examples/` uses the worker pattern end-to-end with a topic recognised by the reference worker
- [x] Each handler's README in the Rust SDK documents its idempotency strategy (matching the table above)
- [x] Idempotency-key store schema documented in `workers/docs/idempotency-store.md`
- [ ] Each non-Rust SDK has a library scaffold (Client + Handler/decorator/annotation) and a README
- [ ] Crash test per handler: kill worker mid-execution, confirm task completes correctly on retry without duplicating side effects
- [ ] No engine-side code changes required (this is the win)
