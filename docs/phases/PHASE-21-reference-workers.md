# Phase 21 — Reference Workers (Sibling Repo)

## Status
Not started

## Prerequisites
Phase 7 (external-task API), Phase 17 (external-task long polling). Driven by [ADR-008](../adr/ADR-008-engine-stays-pure-bpmn.md).

## Goal
Ship reference worker implementations covering the most common integrations, so that "the engine doesn't have a REST connector" doesn't translate to "I have to write all integration code from scratch." Workers live in a top-level [`workers/`](../../workers/) directory of this repo as an independent Cargo project — same repo as the engine, separate build, separate `Cargo.lock`. Versioned together for now; will split into a sibling repo if/when their release cadence diverges.

These workers are reference implementations, not a runtime customers must use — they exist to make the worker pattern as ergonomic as a connector dropdown would have been.

## Scope

The reference workers are written in **Rust** so the SDK shares the engine's toolchain. Python and Node ports are out of scope for this phase.

### Layout: `workers/`

```
workers/
├── README.md                  ← worker pattern overview, quick-start
├── Cargo.toml                 ← workspace manifest
├── crates/
│   ├── conduit-worker/        ← library: fetch-and-lock + complete + fail loop
│   │   └── src/
│   │       ├── client.rs
│   │       ├── handler.rs     ← Handler trait
│   │       └── lib.rs
│   ├── http-worker/           ← binary: implements `http.call` (replaces <conduit:http>)
│   │   ├── src/main.rs
│   │   └── examples/
│   │       └── http-worker.bpmn
│   ├── csv-worker/            ← binary: csv.read / csv.write
│   ├── gcs-worker/            ← binary: gcs.read / gcs.write (service-account auth)
│   └── kafka-produce-worker/  ← binary: kafka.produce
└── triggers/                  ← inbound: external system → engine (planned)
    ├── kafka-consumer/        ← reads a topic, calls /messages/correlate
    └── webhook-receiver/      ← HTTP endpoint, calls /messages/correlate
```

The MVP that gates Phase 20 connector removal is just `crates/conduit-worker/` + `crates/http-worker/`. The other handlers ship incrementally.

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

Each handler's README in `conduit-workers/` documents the strategy explicitly so customers writing their own handlers know what shape to follow.

### Idempotency-key store
Several handlers (notably `http`) need a small piece of persistent state: a table mapping `(task_id, attempt)` → response. The reference workers default to a Postgres table next to the engine's database; a Redis adapter is provided for fleets that prefer it. **One store per worker fleet, not per process** — durability of the dedupe table is what makes a handler safe across worker restarts. Store schema and rotation policy are documented in `conduit-workers/docs/idempotency-store.md`.

### Step-level durability inside a single task
If a single Conduit task is itself a long-running, multi-step computation that needs replayable state (Temporal/Restate territory), customers compose two durable runtimes themselves: the worker becomes a thin adapter that starts a workflow in the durable runtime, polls until terminal, and completes the Conduit task. We do **not** ship a reference adapter for this in `conduit-workers/` — Conduit explicitly stays out of durable-workflow territory and we don't want a reference implementation that implies otherwise.

### What's intentionally out of scope
- Step-level checkpointing primitives in the engine.
- A built-in idempotency-key store served by the engine (it would couple worker dedupe state to engine deploys; bad).
- Exactly-once delivery semantics on the engine's external-task API (the BPMN-level "happens once" guarantee comes from idempotent side effects, not from the transport).

---

## Excluded
- Engine-side changes. Reference workers are downstream consumers of the existing external-task API.
- Worker SDKs in any language beyond Rust for this phase. (Python, Node, Go can come later if there's demand.)
- A reference Temporal/Restate adapter. Customers who want step-level durability inside a single task can compose that themselves; we don't ship a reference for it because it would imply Conduit owns the pattern.
- A "marketplace" or auto-discovery mechanism. Workers are deployed by the customer alongside the engine.

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
- [ ] `workers/` directory contains the Cargo workspace and a README
- [ ] Conduit `README.md` links to `workers/`
- [ ] `docs/MIGRATION.md` (Phase 20) references the reference HTTP worker by name and version
- [ ] At least one example BPMN in this repo's `examples/` uses the worker pattern end-to-end with a topic recognised by the reference worker
- [ ] Each handler's README documents its idempotency strategy (matching the table above)
- [ ] Idempotency-key store schema documented in `workers/docs/idempotency-store.md`
- [ ] Crash test per handler: kill worker mid-execution, confirm task completes correctly on retry without duplicating side effects
- [ ] No engine-side code changes required (this is the win)
