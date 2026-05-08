# Conduit reference workers

Reference SDKs and workers for the [Conduit](../README.md) BPMN engine. Conduit orchestrates; **workers execute**. A `serviceTask` carries a `<conduit:taskTopic>` that the engine uses purely as a routing label — it knows nothing about HTTP, gRPC, Kafka, or any other transport. The SDKs here implement the polling loop, the `Handler` registration ergonomics, and the standard handlers that do the actual side effects.

The boundary is recorded in [ADR-008](../docs/adr/ADR-008-engine-stays-pure-bpmn.md). The phase that scopes this directory is [PHASE-21](../docs/phases/PHASE-21-reference-workers.md). The **wire contract** every SDK conforms to is [`PROTOCOL.md`](PROTOCOL.md) — read that before writing a new SDK or porting handlers.

## Languages

| Language | Path | Status | Reference handlers |
|---|---|---|---|
| **Rust** | [`rust/`](rust/) | MVP — library + `#[handler]` macro + http-worker; csv/gcs/kafka scaffolded | `http.call` |
| **Go** | [`go/`](go/) | scaffolded | (planned) `http.call` |
| **Python** | [`python/`](python/) | scaffolded | (planned) `http.call` |
| **Node (TypeScript)** | [`node/`](node/) | scaffolded | (planned) `http.call` |
| **Java** | [`java/`](java/) | scaffolded | (planned) `http.call` |

Rust is the **reference SDK**: its API shape is what the others mirror, and the wire contract in `PROTOCOL.md` is derived from what the Rust client does. Each language directory is an independent build with its own dependency manifest — they do not share a build system, only the wire contract.

## Idiomatic registration

Each SDK exposes the same conceptual `Handler` (a function from `ExternalTask` to `Complete | BpmnError | failure`) through the host language's most idiomatic registration form. The framework-style sugar is optional — every SDK keeps the underlying type usable directly.

### Rust — proc-macro on an async fn

```rust
use conduit_worker::{handler, ExternalTask, HandlerError, HandlerResult, Variable};

#[handler(topic = "http.call")]
async fn http_call(_task: &ExternalTask) -> Result<HandlerResult, HandlerError> {
    // ...
    Ok(HandlerResult::complete(vec![Variable::string("status", "ok")]))
}

// Generated: pub struct HttpCallHandler; impl Handler for HttpCallHandler { ... }
// let runner = Runner::new(client, Arc::new(HttpCallHandler), RunnerConfig::new("w-1"));
```

### Java — annotation on a class

```java
import io.conduit.worker.TaskHandler;
import io.conduit.worker.ExternalTask;
import io.conduit.worker.HandlerResult;

@TaskHandler(topic = "http.call")
public class HttpCallHandler {
  public HandlerResult handle(ExternalTask task) {
    // ...
    return HandlerResult.complete(Map.of("status", "ok"));
  }
}
```

### Python — decorator on an async function

```python
from conduit_worker import handler, HandlerResult

@handler(topic="http.call")
async def http_call(task):
    # ...
    return HandlerResult.complete({"status": "ok"})
```

### Node (TypeScript) — decorator or builder

```ts
import { Handler, HandlerResult, defineHandler } from "@conduit/worker";

@Handler({ topic: "http.call" })
export class HttpCallHandler {
  async handle(task) {
    return HandlerResult.complete({ status: "ok" });
  }
}

// or, without decorators:
export const httpCall = defineHandler({
  topic: "http.call",
  async handle(task) {
    return HandlerResult.complete({ status: "ok" });
  },
});
```

### Go — register-by-name

```go
import "github.com/kinarix/conduit/workers/go/conduitworker"

func main() {
    runner := conduitworker.NewRunner(client, "go-worker-1")
    runner.Register("http.call", func(task *conduitworker.ExternalTask) (*conduitworker.Result, error) {
        return conduitworker.Complete(map[string]any{"status": "ok"}), nil
    })
    runner.Run(ctx)
}
```

## Quick start

The fastest path is the Rust http-worker against a local engine:

```bash
# Terminal 1: engine
cargo run    # from the repo root

# Terminal 2: worker
cd workers/rust
cp crates/http-worker/examples/worker.yaml worker.yaml
# edit engine.url and any handler entries
cargo run -p http-worker -- --config worker.yaml
```

For Go / Python / Node / Java, follow the README in each language directory.

## Idempotency

Conduit's external-task API is **at-least-once**. A worker that crashes mid-call gets the task re-delivered after the lock TTL expires. SDKs do not protect side effects automatically — every handler must be written to be idempotent under retry.

Standard strategies per topic:

| Handler | Strategy |
|---|---|
| `http.call` | `Idempotency-Key` header derived from `task.id` (template configurable). For non-idempotent verbs the upstream service is expected to honour the header per the [Idempotency-Key RFC draft](https://www.ietf.org/archive/id/draft-ietf-httpapi-idempotency-key-header-04.html) / Stripe convention. |
| `csv.read` / `gcs.read` | Naturally idempotent. |
| `csv.write` | Write to `{path}.tmp.{task_id}` and atomic-rename. |
| `gcs.write` | `ifGenerationMatch=0` for create-only; generation-match for updates. |
| `kafka.produce` | `enable.idempotence=true` + deterministic message key from `task.id`. |

A `(task_id, attempt) → response` dedupe table — used by `http.call` for replay-safe writes — is defined in [`docs/idempotency-store.md`](docs/idempotency-store.md). The schema is **language-agnostic**: any SDK reads/writes the same Postgres table.

## Repository layout

```
workers/
├── README.md                    ← you are here
├── PROTOCOL.md                  ← wire contract every SDK conforms to
├── docs/
│   └── idempotency-store.md     ← Postgres dedupe table (language-agnostic)
├── rust/                        ← Rust SDK (reference)
├── go/                          ← Go SDK
├── python/                      ← Python SDK
├── node/                        ← Node SDK (TypeScript)
└── java/                        ← Java SDK
```

The top-level `workers/` directory has no shared build system on purpose — each language directory is independently buildable and testable. Engine + Rust SDK are the only two artifacts versioned together; other SDKs are versioned per their own ecosystem conventions.

## Contributing a new SDK

1. Read [`PROTOCOL.md`](PROTOCOL.md) end to end.
2. Read the Rust SDK ([`rust/crates/conduit-worker/src/`](rust/crates/conduit-worker/src/)) — it is the reference.
3. Mirror the conceptual API (`Client`, `Handler`, `Runner`) using your language's most idiomatic form.
4. Provide an idiomatic registration mechanism — proc-macro / annotation / decorator / function — as sugar over the underlying type.
5. Ship at minimum: library, README, one integration test against a mock engine.
6. An `http-worker` binary with parity to the Rust one is desirable but not required for the initial drop.

## License

Apache-2.0 — same as the Conduit engine.
