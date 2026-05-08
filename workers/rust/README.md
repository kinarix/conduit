# Conduit reference workers — Rust

Reference workers for the Conduit BPMN engine, written in Rust. This is the **reference SDK** — its shape is what `workers/PROTOCOL.md` codifies and what the Go / Python / Node / Java SDKs mirror.

Conduit orchestrates; **workers execute**. A `serviceTask` carries a `<conduit:taskTopic>` that the engine uses purely as a routing label — it knows nothing about HTTP, gRPC, Kafka, or any other transport. The crates here implement the workers that do the actual side effects.

The boundary is recorded in [ADR-008](../../docs/adr/ADR-008-engine-stays-pure-bpmn.md). Phase 21 scopes this work: see [`PHASE-21`](../../docs/phases/PHASE-21-reference-workers.md). For the language-agnostic wire contract, see [`workers/PROTOCOL.md`](../PROTOCOL.md).

## Layout

```
workers/rust/
├── Cargo.toml                          ← workspace manifest (independent of engine workspace)
└── crates/
    ├── conduit-worker/                 ← library: Client + Handler trait + run loop
    ├── http-worker/                    ← binary: http.call (replaces <conduit:http>)
    ├── csv-worker/                     ← scaffolded
    ├── gcs-worker/                     ← scaffolded
    └── kafka-produce-worker/           ← scaffolded
```

| Crate | Topic | Status |
|---|---|---|
| `http-worker` | `http.call` | MVP |
| `csv-worker` | `csv.read`, `csv.write` | scaffolded — implementation pending |
| `gcs-worker` | `gcs.read`, `gcs.write` | scaffolded — implementation pending |
| `kafka-produce-worker` | `kafka.produce` | scaffolded — implementation pending |

## Quick start (http-worker)

```bash
# 1. Run a Conduit engine somewhere (default: http://localhost:8080).
# 2. Deploy a BPMN that uses the worker pattern:
#      <serviceTask><extensionElements>
#        <conduit:taskTopic>http.call</conduit:taskTopic>
#      </extensionElements></serviceTask>
#    See crates/http-worker/examples/http-worker.bpmn.
# 3. Configure the worker:
cp crates/http-worker/examples/worker.yaml worker.yaml
# edit engine.url and the http.call handler entry

# 4. Run.
CONDUIT_ORDERS_TOKEN=$(cat ~/.secrets/orders) \
  cargo run -p http-worker -- --config worker.yaml
```

## Writing a custom worker

Implement [`Handler`](crates/conduit-worker/src/handler.rs):

```rust
use async_trait::async_trait;
use conduit_worker::{ExternalTask, Handler, HandlerError, HandlerResult, Variable};

struct MyHandler;

#[async_trait]
impl Handler for MyHandler {
    fn topic(&self) -> &str { "my.topic" }

    async fn handle(&self, task: &ExternalTask) -> Result<HandlerResult, HandlerError> {
        // ... do work ...
        Ok(HandlerResult::Complete {
            variables: vec![Variable::string("status", "ok")],
        })
    }
}
```

Then start a [`Runner`](crates/conduit-worker/src/runner.rs):

```rust
use conduit_worker::{Client, ClientConfig, Runner, RunnerConfig};

let client = Client::new(ClientConfig::new("http://localhost:8080"))?;
let runner = Runner::new(client, std::sync::Arc::new(MyHandler), RunnerConfig::new("my-worker-1"));
runner.run().await;
```

## Idempotency

Conduit's external-task API is at-least-once: a worker that crashes mid-call gets the task re-delivered once the lock TTL expires. To turn that into "feels exactly-once," every handler must be idempotent under retry. Strategies per handler:

| Handler | Strategy |
|---|---|
| `http.call` | `Idempotency-Key` header derived from `task.id` (template configurable). For non-idempotent verbs the upstream service is expected to honour the header per the [Idempotency-Key RFC draft](https://www.ietf.org/archive/id/draft-ietf-httpapi-idempotency-key-header-04.html) / Stripe convention. |
| `csv.read` | Naturally idempotent. |
| `csv.write` | Write to `{path}.tmp.{task_id}` and atomic-rename. |
| `gcs.read` | Naturally idempotent (pin to a generation when consistency matters). |
| `gcs.write` | `ifGenerationMatch=0` for create-only; generation-match for updates. |
| `kafka.produce` | `enable.idempotence=true` + deterministic message key from `task.id`. |

A `(task_id, attempt) → response` dedupe table is documented in [`../docs/idempotency-store.md`](../docs/idempotency-store.md) for handlers that need replay-safe writes (notably `http.call`).

## License

Apache-2.0 — see [LICENSE](../../LICENSE). Same license as the Conduit engine.
