# ADR-002: Web Framework — Axum

## Status
Proposed (pending Phase 0 spike)

## Context
Need an HTTP framework for the engine's REST API.

## Decision
Use **Axum 0.7.x**.

## Candidates Evaluated

| Framework | Pros | Cons |
|---|---|---|
| Axum | Native Tokio, excellent ergonomics, Tower middleware, type-safe routing | Newer than Actix |
| Actix-web | Mature, very fast, large ecosystem | Different actor model, less idiomatic with Tokio |
| Warp | Filter-based composability | Less active, smaller ecosystem |

## Rationale
- Built by the Tokio team — native integration, no adaptation layer
- Type-safe extractors catch errors at compile time
- Tower middleware ecosystem (tracing, CORS, auth)
- Shared state via `State<T>` extractor is clean and ergonomic
- Active development, growing community

## Spike Validation
- [ ] Build minimal API: POST /instances, GET /instances/:id, POST /instances/:id/complete
- [ ] Test shared state (Arc<Engine>) across handlers
- [ ] Test error propagation (EngineError → HTTP response)
- [ ] Load test: 1000 concurrent requests, measure p50/p95/p99 latency
