# Conduit external-task protocol

This document is the language-agnostic wire contract that every reference SDK in `workers/` conforms to. Read this before writing a new SDK; if the wire shape ever changes, this document changes first and every SDK follows.

The contract is the engine's existing **external-task API** — workers are clients of `/api/v1/external-tasks/*`. The engine doesn't know about workers as a concept beyond "polls for work and reports back," so this document describes the HTTP surface, the JSON shapes, and the semantic guarantees workers can rely on.

## Base URL and authentication

- Base URL: configurable, e.g. `http://localhost:8080`. All paths below are relative to this.
- Authentication: optional `Authorization: Bearer <token>` header. The reference engine accepts unauthenticated requests in dev; production deployments enforce a token. SDKs MUST treat the token as a sensitive value (mark headers sensitive, redact in logs).
- Content type: `application/json` request and response, UTF-8.

## Endpoints

### 1. `POST /api/v1/external-tasks/fetch-and-lock`

Long-poll for tasks. The engine returns 0..N tasks whose `locked_until` is null or has expired, locks them to `worker_id` for `lock_duration_secs`, and ships their current process variables.

**Request**
```json
{
  "worker_id": "http-worker-7d2",
  "topic": "http.call",
  "max_jobs": 10,
  "lock_duration_secs": 30
}
```

| Field | Required | Notes |
|---|---|---|
| `worker_id` | yes | Stable identifier for this worker process. The engine uses it to scope locks; reusing the ID across restarts is fine. |
| `topic` | optional | Filter by `<conduit:taskTopic>`. Omit to receive any topic (rarely useful). |
| `max_jobs` | optional, default 10, capped at 100 | Upper bound on tasks returned this round. |
| `lock_duration_secs` | optional, default 30 | TTL on the lock. The worker must complete / fail / extend within this window or the task is reclaimable. |

**Response — `200 OK`**
```json
[
  {
    "id": "0a3...",
    "topic": "http.call",
    "instance_id": "...",
    "execution_id": "...",
    "locked_until": "2026-05-08T10:00:30Z",
    "retries": 3,
    "retry_count": 0,
    "variables": [
      { "name": "order_id", "value_type": "String", "value": "ord-42" },
      { "name": "amount",   "value_type": "Long",   "value": 1500 }
    ]
  }
]
```

`variables` is the full set of process-instance variables visible to this task (same scope rules as a `serviceTask` in the engine).

If no tasks are available, the engine may either return `[]` immediately or hold the request open for a short interval (Phase 17 long polling). SDKs should treat both as normal and back off briefly before re-polling on `[]`.

### 2. `POST /api/v1/external-tasks/{id}/complete`

Report the task as completed successfully. Optionally write process variables back to the instance.

**Request**
```json
{
  "worker_id": "http-worker-7d2",
  "variables": [
    { "name": "http_status", "value_type": "Long",   "value": 201 },
    { "name": "order",       "value_type": "Json",   "value": { "id": 42 } }
  ]
}
```

`worker_id` must match the worker that holds the lock; the engine rejects the call otherwise. `variables` is optional — omit or send `[]` to complete with no variable changes.

**Response — `204 No Content`**.

### 3. `POST /api/v1/external-tasks/{id}/failure`

Report a transient/system failure. The engine decrements `retries` (which started at the BPMN-defined limit) and re-locks the task for another worker after the lock TTL. When `retries` reaches 0, the task transitions to `failed` and stops being re-delivered.

**Request**
```json
{
  "worker_id": "http-worker-7d2",
  "error_message": "Connection refused: api.example.com:443"
}
```

**Response — `204 No Content`**.

Use this for failures the BPMN does not need to know about — connection errors, timeouts, transient 5xx upstream responses. The process instance keeps waiting; nothing in the BPMN graph branches on the failure.

### 4. `POST /api/v1/external-tasks/{id}/bpmn-error`

Throw a BPMN error. This routes the process down a `boundaryErrorEvent` if one matches `error_code`, or terminates the instance with an error if not.

**Request**
```json
{
  "worker_id": "http-worker-7d2",
  "error_code": "PAYMENT_DECLINED",
  "error_message": "card_declined",
  "variables": [
    { "name": "decline_reason", "value_type": "String", "value": "insufficient_funds" }
  ]
}
```

**Response — `204 No Content`**.

Use this when the BPMN models the failure as a domain outcome (HTTP 4xx that callers branch on, "no rows found", "policy violation", etc.). The contrast with `failure`:

| Concern | `failure` | `bpmn-error` |
|---|---|---|
| Decrements retries? | Yes | No |
| Re-delivered? | Yes (until retries=0) | No |
| Branches the BPMN? | No | Yes (if a matching boundary event exists) |

### 5. `POST /api/v1/external-tasks/{id}/extend-lock`

Refresh the lock without completing. Use this for handlers that legitimately need longer than `lock_duration_secs`.

**Request**
```json
{
  "worker_id": "http-worker-7d2",
  "lock_duration_secs": 60
}
```

**Response — `204 No Content`**.

## Variable shape

Every variable on the wire is `{ name, value_type, value }`. `value_type` is one of:

| `value_type` | JSON `value` shape |
|---|---|
| `String` | string |
| `Long` | integer (i64) |
| `Double` | number (f64) |
| `Boolean` | boolean |
| `Json` | any JSON value (object, array, etc.) |
| `Null` | `null` |

SDKs SHOULD provide constructors per type (`Variable.string("name", "value")`, etc.) so users don't have to remember the strings.

## Error responses

Non-2xx responses from the engine carry the structured-error envelope documented in [`CLAUDE.md`](../CLAUDE.md):

```json
{ "code": "U001", "message": "...", "action": "..." }
```

`U`-prefix codes are user/client errors (4xx). `S`-prefix codes are system errors (5xx) and never leak internals. SDKs SHOULD surface `code` and `message` in their thrown error so users have an actionable handle.

## Semantics workers must understand

### At-least-once delivery
The engine guarantees a task is delivered **at least once** per attempt. A worker that crashes between starting the side effect and calling `complete` will see the task re-delivered to itself or another worker after the lock TTL. **Side effects must be idempotent under retry** — see the per-handler strategies in [PHASE-21](../docs/phases/PHASE-21-reference-workers.md) and the dedupe-table schema in [`docs/idempotency-store.md`](docs/idempotency-store.md).

### `worker_id` is a lock owner, not an identity
Two workers with the same `worker_id` will fight over locks. Run with distinct IDs in production. Hostname + PID + UUID is a fine convention.

### Lock TTL is a soft contract, not a hard one
If a worker exceeds `lock_duration_secs` without calling `extend-lock`, the engine considers the lock expired and may hand the task to another worker. The original worker's eventual `complete` call will be rejected (the task is no longer locked to it) — SDKs SHOULD log this distinctly so users can spot under-sized lock TTLs.

### Order is not preserved
`fetch-and-lock` returns tasks in an unspecified order. Workers MUST NOT assume in-order delivery within a single instance, much less across instances.

### Variables are a snapshot
The `variables` shipped with a task reflect the instance state at lock time. If another path of the same instance writes a variable while the worker is processing, that write isn't visible until the next `fetch-and-lock`. The `complete` call's `variables` field is the worker's full reply — the engine merges it into the instance.

## SDK conformance checklist

A new SDK in `workers/<lang>/` SHOULD provide:

- [ ] `Client` wrapping the 5 endpoints above
- [ ] Configurable base URL, optional bearer token, request timeout
- [ ] `Handler` interface / trait / abstract class with `topic()` + `handle(task) -> Complete | BpmnError | failure`
- [ ] `Runner` (or equivalent) that loops fetch-and-lock → handle → report, with configurable poll interval and lock duration
- [ ] Idiomatic registration sugar — `#[handler]` proc-macro (Rust), `@TaskHandler` annotation (Java), `@handler` decorator (Python), `@Handler` decorator or builder (Node), `Register(topic, fn)` (Go)
- [ ] At least one integration test against a mock engine (record-replay or a localhost fake) covering: fetch returns a task → handler runs → complete is called with the right body
- [ ] README documenting quick-start + idempotency expectations

## Versioning

This protocol is implicitly versioned with the engine. A new endpoint or a breaking change to an existing one must:

1. Update this document first.
2. Add an `S` or `U` error code if the change introduces one.
3. Land alongside an SDK update for at least the Rust reference, with the new shape mirrored to other SDKs in a follow-up.

Backward compatibility is preserved by additive changes (new optional fields, new endpoints) wherever possible.
