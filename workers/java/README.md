# Conduit worker SDK — Java

Java 21+ SDK for the Conduit external-task API. Mirrors the [Rust reference SDK](../rust/) and conforms to [`workers/PROTOCOL.md`](../PROTOCOL.md).

## Status

Library scaffold (Maven). Tests cover client round-trip, annotation-driven registration, and BPMN-error reporting against an embedded `HttpServer`. No `http-worker` artifact yet — the Rust [`http-worker`](../rust/crates/http-worker/) is the reference.

## Build

```bash
cd workers/java
mvn -q test
```

## Quick start

```java
import io.conduit.worker.*;

@TaskHandler(topic = "http.call")
public class HttpCallHandler implements Handler {
  @Override
  public HandlerResult handle(ExternalTask task) {
    return HandlerResult.complete(Variable.string("status", "ok"));
  }

  public static void main(String[] args) throws Exception {
    Client client = new Client(new Client.Config("http://localhost:8080"));
    Runner runner = new Runner(client, new Runner.Config("java-worker-1"));
    runner.discover(new HttpCallHandler());
    runner.run();
  }
}
```

## Idiomatic registration

`@TaskHandler(topic = "...")` annotation on a class implementing `Handler`. `Runner.discover(Object...)` reads the annotation and registers the instance under that topic — no classpath scanning, no reflection magic, just `Class.getAnnotation`. `runner.register(topic, handler)` is the same registration without the annotation.

## Idempotency

Same contract as the other SDKs — see [`workers/PROTOCOL.md`](../PROTOCOL.md#at-least-once-delivery) and [`workers/docs/idempotency-store.md`](../docs/idempotency-store.md).
