# Phase 21 — Reference Workers (Sibling Repo)

## Status
Not started

## Prerequisites
Phase 7 (external-task API), Phase 17 (external-task long polling). Driven by [ADR-008](../adr/ADR-008-engine-stays-pure-bpmn.md).

## Goal
Ship reference worker implementations covering the most common integrations, so that "the engine doesn't have a REST connector" doesn't translate to "I have to write all integration code from scratch." Workers live in a sibling repository (`conduit-workers`, separate from this repo) so their release cadence, language footprint, and dependency surface stay decoupled from the engine.

These workers are reference implementations, not a runtime customers must use — they exist to make the worker pattern as ergonomic as a connector dropdown would have been.

## Scope

### Sibling repo: `conduit-workers/`

```
conduit-workers/
├── README.md                  ← worker pattern overview, quick-start
├── python/
│   ├── pyproject.toml
│   ├── conduit_worker/
│   │   ├── __init__.py
│   │   ├── client.py          ← fetch-and-lock + complete + fail
│   │   ├── handlers/
│   │   │   ├── http.py        ← REST calls (replaces <conduit:http>)
│   │   │   ├── csv_io.py      ← CSV read/write
│   │   │   ├── gcs.py         ← GCS read/write (service-account auth)
│   │   │   └── kafka_produce.py
│   │   └── __main__.py
│   └── examples/
│       └── http-worker.bpmn   ← matching BPMN sample
├── node/
│   ├── package.json
│   ├── src/
│   │   ├── client.ts
│   │   ├── handlers/
│   │   │   ├── http.ts
│   │   │   ├── csv.ts
│   │   │   ├── gcs.ts
│   │   │   └── kafkaProduce.ts
│   │   └── index.ts
│   └── examples/
└── triggers/                  ← inbound: external system → engine
    ├── kafka-consumer/        ← reads a topic, calls /messages/correlate
    └── webhook-receiver/      ← HTTP endpoint, calls /messages/correlate
```

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

### Step-level durability inside a single task — the escape hatch
When a single Conduit task is itself a long-running, multi-step computation that needs replayable state (Temporal/Restate territory), customers compose two durable runtimes:

```
Conduit serviceTask  →  worker fetches → starts/joins a Temporal workflow
                                       → polls Temporal until terminal
                                       → completes Conduit task with result
```

The worker is a thin adapter; durable execution within the task lives in Temporal/Restate. Conduit doesn't try to absorb that capability — two layers, cleanly composed. A reference adapter (`conduit-workers/python/adapters/temporal/`) ships as part of this phase to demonstrate the pattern; we explicitly do **not** build a "durable workflows" feature into Conduit itself.

### What's intentionally out of scope
- Step-level checkpointing primitives in the engine.
- A built-in idempotency-key store served by the engine (it would couple worker dedupe state to engine deploys; bad).
- Exactly-once delivery semantics on the engine's external-task API (the BPMN-level "happens once" guarantee comes from idempotent side effects, not from the transport).

---

## Excluded
- Engine-side changes. Reference workers are downstream consumers of the existing external-task API.
- Worker SDKs in any language beyond Python and Node for v1. (Go can come later.)
- A "marketplace" or auto-discovery mechanism. Workers are deployed by the customer alongside the engine.

## Test Plan (in the sibling repo)
- Each handler has integration tests against:
  - A live Conduit engine (via testcontainers from the sibling repo).
  - The relevant external dependency (wiremock for `http`, fake-gcs-server for `gcs`, redpanda or kafka container for `kafka.produce`).
- A smoke test that runs the engine + worker + a tiny BPMN end-to-end for each handler.

## Engine-side work (this repo)
- Add a "Workers" section to `README.md` linking to the sibling repo.
- Add a section to `docs/ARCHITECTURE.md` clarifying the worker boundary (referencing ADR-008).
- Optional: a `docs/WORKERS.md` with the quick-start and a snippet of each handler.

## Verification Checklist (this repo's view)
- [ ] Sibling repo `conduit-workers` exists, public, with README
- [ ] `README.md` links to it
- [ ] `docs/MIGRATION.md` (Phase 20) references the reference HTTP worker by name and version
- [ ] At least one example BPMN in this repo's `examples/` uses the worker pattern end-to-end with a topic recognised by the reference worker
- [ ] Each handler's README documents its idempotency strategy (matching the table above)
- [ ] Idempotency-key store schema documented in `conduit-workers/docs/idempotency-store.md`
- [ ] Temporal adapter example present and tested (proves the "wrap a durable workflow" pattern works)
- [ ] Crash test per handler: kill worker mid-execution, confirm task completes correctly on retry without duplicating side effects
- [ ] No engine-side code changes required (this is the win)
