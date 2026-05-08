# ADR-008: Engine Stays Pure BPMN — Workers Handle Protocols

## Status
Accepted (2026-05-08). Supersedes [ADR-007](ADR-007-connector-architecture.md).

## Context
A recurring temptation when extending a BPM engine is to absorb integration logic into the engine itself: REST clients, file readers, message-queue producers and consumers. This is how the previous generation of engines (Camunda 7, Activiti, Flowable) accumulated their hundreds of "connectors" and the JVM middleware baggage that came with them. Conduit's whole reason to exist is to be the opposite of that.

The principle was already in `CLAUDE.md` from Phase 0 ("Workers are external — engine orchestrates, workers execute") but had drifted in Phase 16 with the in-process `<conduit:http>` connector, and was about to drift further with a generalised connector framework. ADR-007 records the rejected expansion. This ADR records the principle so the question is settled.

## Decision
**The engine speaks BPMN. Workers speak protocols.**

Concretely:

1. The engine's responsibilities are: parsing BPMN, advancing tokens, holding variables, scheduling timers, correlating messages and signals, evaluating gateways and DMN decisions, persisting history, exposing a REST API for orchestration and external task fetch-and-lock.
2. The engine does **not** make HTTP calls to external systems, read files, talk to GCS / S3, produce or consume from message queues, or hold credentials for external systems beyond its own database.
3. All integration with external systems happens in **worker processes** that:
   - Authenticate to the engine and poll `/api/v1/external-tasks` (or use long polling, Phase 17).
   - Execute the integration in their native ecosystem (Python `requests`, Node `kafkajs`, Go `google-cloud-go`, etc.).
   - Report completion or BPMN error back via the engine API.
4. Inbound integrations (a Kafka topic delivering messages, a webhook firing) are likewise the responsibility of workers / sidecars that translate external events into calls on the engine's `/messages/correlate`, `/signals/broadcast`, or `/process-instances` endpoints.

## Rationale

### Process boundary == failure boundary
A bug in `rdkafka`, a TLS misconfiguration in `reqwest`, or a memory leak in a CSV parser must not be able to take the orchestrator down. Out-of-process workers are crash domains; in-process connectors aren't.

### Release cadence
Worker logic and integration SDKs evolve much faster than orchestration semantics. Coupling them to the engine's release cycle slows both.

### Language fit
HTTP integrations are most cleanly written in Python or TypeScript. Kafka in Java or Go. CSV/Excel work tends to live wherever the data team already works. Forcing all of these into Rust to live in the engine is hostile to the people who actually write the integrations.

### Operational story
Customers running Conduit in production already operate workers (it's the whole point of Phase 7's external-task pattern). One more worker process is not the marginal cost it might appear to be — and a single reference worker can cover REST + CSV + GCS for an organisation that doesn't want to write their own.

### Camunda 8 / Zeebe got this right
Zeebe ships with a "Connector Runtime" — a separate process that hosts the protocol-handling logic. The decision of "engine in one process, integrations in another" is mature, externally validated, and the right shape for Conduit too.

## Consequences

### Out of scope (forever, unless this ADR is superseded)
- In-engine REST / GraphQL clients
- In-engine file readers (CSV, JSON, Excel, Parquet, …)
- In-engine cloud-storage clients (GCS, S3, Azure Blob)
- In-engine message-queue producers and consumers (Kafka, NATS, RabbitMQ, SQS)
- In-engine DB clients beyond the engine's own PostgreSQL
- A "connector marketplace" or any plugin loading mechanism — WASM, dylib, or otherwise

### What we ship instead
- A solid external-task API with long polling (Phase 17) so workers feel responsive.
- Reference worker implementations in Python and Node covering the most common integrations (Phase 21).
- A `<conduit:taskTopic>` extension that makes the worker pattern as ergonomic as a connector dropdown would have been.

### Existing exception: `<conduit:http>` (Phase 16)
This is the one inherited violation of the principle. It is being deprecated, not extended — see Phase 20. New violations are not accepted.

### Engine ergonomics still matter
This ADR is not a license to make integrations painful. Long polling, BPMN error round-tripping from workers, and clean fetch-and-lock semantics are what we invest in instead.

## Validation
This ADR is invalidated only by a concrete user need that workers genuinely cannot satisfy. "It would be more convenient" doesn't count; "we tried with workers and hit a fundamental limitation" would. If that day comes, supersede this ADR — don't quietly bolt on a connector.

## References
- `CLAUDE.md` Core Design Principles (#4 in particular)
- [ADR-007: In-process connector framework — rejected](ADR-007-connector-architecture.md)
- [PHASE-20: Deprecate `<conduit:http>` connector](../phases/PHASE-20-deprecate-http-connector.md)
- [PHASE-21: Reference workers](../phases/PHASE-21-reference-workers.md)
