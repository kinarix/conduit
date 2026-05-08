# ADR-007: In-Process Connector Framework — Rejected

## Status
**Rejected** (2026-05-08). Superseded by [ADR-008: Engine stays pure BPMN](ADR-008-engine-stays-pure-bpmn.md).

This ADR is preserved as a record so the same direction is not re-proposed without revisiting the reasoning below.

## Context
On 2026-05-07 we considered generalising the existing HTTP push connector (`<conduit:http>`, Phase 16) into a `Connector` trait + registry, with first-party in-process implementations for REST, CSV, GCS, and Kafka producer/consumer. A scaffold and full design docs were drafted (see git history of this file at commit `82f6783^..` — note: never landed; reverted same-cycle).

## What Was Proposed
- A `Connector` trait + `ConnectorRegistry` for outbound integrations (engine → external system).
- A `Trigger` trait + leader-managed manager for inbound integrations (external system → engine).
- BPMN extensions `<conduit:connector type="…">` and `<conduit:trigger type="…">`.
- First-party connectors compiled into the binary: `rest`, `csv.read`, `csv.write`, `fs.read`, `fs.write`, `gcs.read`, `gcs.write`, `kafka.produce`, `kafka.consume`.
- Hard rename of the existing `<conduit:http>` extension to `<conduit:connector type="rest">`.

## Why It Was Rejected

### 1. Violates the engine's stated boundary
`CLAUDE.md` core principle #4: "Workers are external — engine orchestrates, workers execute." The proposal would have made the engine speak HTTP, CSV, GCS, and Kafka — all integration concerns. Each in-process connector pulls a third-party SDK (`reqwest`, `google-cloud-storage`, `rdkafka`) into the orchestrator's address space; a bug or memory leak in any one becomes a bug in the orchestrator. Camunda 8 / Zeebe shipped its connectors as a **separate process** (the Connector Runtime) for exactly this reason.

### 2. Engine release cadence couples to integration cadence
Every new connector or version bump of an integration SDK forces a Conduit release. With workers, integrations evolve independently of the engine and on the language ecosystem of the customer's choice.

### 3. The ergonomic argument doesn't hold
The original motivation for the in-process HTTP connector was avoiding "yet another worker process." But:
- A single reference worker binary (or sidecar container) costs roughly the same to operate as the engine.
- External-task long polling (Phase 17) already eliminates the latency overhead that would have been the technical case for in-process execution.
- Customers writing real integrations almost always need their own worker anyway (custom logic, internal APIs, secrets).

### 4. Two traits is two abstractions to maintain forever
The proposal correctly identified that outbound (request/response) and inbound (subscription) need different lifecycles. But that's evidence the engine shouldn't host either: workers do both naturally without the engine needing two separate frameworks.

### 5. The escape hatch was the right path all along
The proposal's "fallback for non-first-party logic" was the existing external-task pattern. If that's the right answer for the long tail, it's the right answer for the head too — there's no first-party integration we can host in-engine that a worker couldn't.

## Decision
Do not build the connector framework. Instead:

1. Treat the existing `<conduit:http>` connector (Phase 16) as legacy and plan its deprecation. See [`docs/phases/PHASE-20-deprecate-http-connector.md`](../phases/PHASE-20-deprecate-http-connector.md).
2. Ship reference worker SDKs (Rust as the reference; Go / Python / Node / Java to follow) covering REST, CSV, GCS, Kafka producer + consumer, in a top-level [`workers/`](../../workers/) directory of this repo. See [`docs/phases/PHASE-21-reference-workers.md`](../phases/PHASE-21-reference-workers.md).
3. Codify the principle in ADR-008.

## Consequences
- **Negative**: Conduit out-of-the-box does not "do" REST calls or read CSVs; integration requires running a worker. Mitigation: ship reference workers and clear docs.
- **Positive**: The engine's surface area remains BPMN-only. Integration SDKs, their CVEs, and their version churn live outside the orchestrator.
- **Positive**: Customers extend with their own workers in any language without forking the engine.

## References
- [ADR-008: Engine stays pure BPMN](ADR-008-engine-stays-pure-bpmn.md)
- [PHASE-20: Deprecate `<conduit:http>` connector](../phases/PHASE-20-deprecate-http-connector.md)
- [PHASE-21: Reference workers (in-tree under `workers/`)](../phases/PHASE-21-reference-workers.md)
- [Camunda 8 Connector Runtime](https://docs.camunda.io/docs/components/connectors/connector-runtime/) — out-of-process precedent
