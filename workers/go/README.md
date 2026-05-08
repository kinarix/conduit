# Conduit worker SDK — Go

Go SDK for the Conduit external-task API. Mirrors the [Rust reference SDK](../rust/) and conforms to [`workers/PROTOCOL.md`](../PROTOCOL.md).

## Status

Library + reference `http-worker` binary. Tests cover the `Client` round-trip, the `Runner` dispatch loop, and the http-worker handler against an `httptest` mock engine (`go test ./...` → 8 tests pass). The Rust [`http-worker`](../rust/crates/http-worker/) is the reference; the Go binary mirrors its `worker.yaml` schema.

## Layout

```
workers/go/
├── go.mod
├── conduitworker/         ← library: Client, Runner, types
└── cmd/
    └── http-worker/       ← reference binary: http.call (mirrors Rust)
        ├── main.go
        ├── config.go      ← worker.yaml schema
        ├── handler.go     ← HTTP send + response mapping
        ├── render.go      ← {{var:NAME}} / {{task_id}} substitution + jsonpath
        └── examples/
            └── worker.yaml
```

## Quick start

```bash
cd workers/go
go test ./...
```

Run the reference binary against a Conduit engine on localhost:

```bash
cp cmd/http-worker/examples/worker.yaml worker.yaml
# edit engine.url and any handler entries
CONDUIT_ORDERS_TOKEN=$(cat ~/.secrets/orders) \
  go run ./cmd/http-worker -config worker.yaml
```

```go
package main

import (
    "context"
    "github.com/kinarix/conduit/workers/go/conduitworker"
)

func main() {
    client := conduitworker.NewClient(conduitworker.ClientConfig{
        BaseURL: "http://localhost:8080",
    })
    runner := conduitworker.NewRunner(client, "my-worker-1")
    runner.Register("my.topic", func(ctx context.Context, task *conduitworker.ExternalTask) (*conduitworker.Result, error) {
        return conduitworker.Complete(conduitworker.VarString("status", "ok")), nil
    })
    runner.Run(context.Background())
}
```

## Idiomatic registration

Go has no annotations; `Runner.Register("topic", fn)` is the registration form across the ecosystem (`http.HandleFunc`, `mux.Handle`, etc.). The Rust proc-macro and Java annotation forms reduce to the same shape underneath.

## Idempotency

Same contract as the other SDKs — see [`workers/PROTOCOL.md`](../PROTOCOL.md#at-least-once-delivery) and [`workers/docs/idempotency-store.md`](../docs/idempotency-store.md).
