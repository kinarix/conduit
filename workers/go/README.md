# Conduit worker SDK — Go

Go SDK for the Conduit external-task API. Mirrors the [Rust reference SDK](../rust/) and conforms to [`workers/PROTOCOL.md`](../PROTOCOL.md).

## Status

Library scaffold. Tests cover the `Client` round-trip and the `Runner` dispatch loop. The `cmd/http-worker` binary is a placeholder — for production use, the Rust [`http-worker`](../rust/crates/http-worker/) is the reference.

## Layout

```
workers/go/
├── go.mod
├── conduitworker/         ← library: Client, Runner, types
└── cmd/
    └── http-worker/       ← scaffolded binary
```

## Quick start

```bash
cd workers/go
go test ./...
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
