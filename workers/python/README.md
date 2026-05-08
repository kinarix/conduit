# Conduit worker SDK — Python

Python (3.10+) SDK for the Conduit external-task API. Mirrors the [Rust reference SDK](../rust/) and conforms to [`workers/PROTOCOL.md`](../PROTOCOL.md).

## Status

Library scaffold. Tests cover client round-trip, `Complete` and `BpmnError` reporting, and decorator-driven registration. No `http-worker` binary yet — the Rust [`http-worker`](../rust/crates/http-worker/) is the reference.

## Install

```bash
cd workers/python
python -m venv .venv && . .venv/bin/activate
pip install -e ".[test]"
pytest
```

## Quick start

```python
import asyncio
from conduit_worker import (
    Client, ClientConfig, Complete, ExternalTask, HandlerResult,
    Runner, RunnerConfig, Variable, handler,
)

@handler(topic="http.call")
async def http_call(task: ExternalTask) -> HandlerResult:
    # ... do work ...
    return Complete(variables=[Variable.string("status", "ok")])

async def main():
    async with Client(ClientConfig(base_url="http://localhost:8080")) as client:
        runner = Runner(client, RunnerConfig(worker_id="py-worker-1"))
        runner.discover(http_call)
        await runner.run()

asyncio.run(main())
```

## Idiomatic registration

The `@handler(topic=...)` decorator attaches `__conduit_topic__` to the function; `Runner.discover(*fns)` registers every decorated function. You can also use `runner.register(topic, fn)` directly without the decorator — the framework form is sugar over the underlying registration mechanism.

## Idempotency

Same contract as the other SDKs — see [`workers/PROTOCOL.md`](../PROTOCOL.md#at-least-once-delivery) and [`workers/docs/idempotency-store.md`](../docs/idempotency-store.md).
