# Conduit worker SDK — Node (TypeScript)

Node 20+ SDK for the Conduit external-task API. Mirrors the [Rust reference SDK](../rust/) and conforms to [`workers/PROTOCOL.md`](../PROTOCOL.md).

## Status

Library scaffold (TypeScript ESM). Tests cover client round-trip and runner dispatch. No `http-worker` binary yet — the Rust [`http-worker`](../rust/crates/http-worker/) is the reference.

## Install

```bash
cd workers/node
npm install
npm test
```

## Quick start

```typescript
import {
  Client,
  HandlerResult,
  Runner,
  Variable,
  defineHandler,
} from "@conduit/worker";

const httpCall = defineHandler({
  topic: "http.call",
  async handle(task) {
    return HandlerResult.complete(Variable.string("status", "ok"));
  },
});

const client = new Client({ baseUrl: "http://localhost:8080" });
const runner = new Runner(client, { workerId: "ts-worker-1" });
runner.register(httpCall);
await runner.run();
```

## Idiomatic registration

`defineHandler({ topic, handle })` is the registration form. It returns a plain `HandlerDefinition` so a handler module can export many handlers and the calling code wires them up. `runner.register(topic, fn)` is equivalent.

A TypeScript decorator form (`@Handler({ topic: "..." })`) is intentionally not provided in v0.1: the standard decorators proposal is still settling and the builder form covers the same ergonomic ground without forcing `experimentalDecorators` on consumers.

## Idempotency

Same contract as the other SDKs — see [`workers/PROTOCOL.md`](../PROTOCOL.md#at-least-once-delivery) and [`workers/docs/idempotency-store.md`](../docs/idempotency-store.md).
