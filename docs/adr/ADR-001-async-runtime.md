# ADR-001: Async Runtime — Tokio

## Status
Proposed (pending Phase 0 spike)

## Context
The BPM engine requires an async runtime for:
- Handling concurrent HTTP requests
- Running the job executor as a background task
- Managing concurrent token advancements
- Timer accuracy for process timeouts

## Decision
Use **Tokio 1.x** as the async runtime.

## Candidates Evaluated

| Runtime | Pros | Cons |
|---|---|---|
| Tokio | Industry standard, vast ecosystem, excellent performance, native support in Axum/SQLx/reqwest | Slightly heavier than alternatives |
| async-std | Simpler API | Less ecosystem support, less active development |
| smol | Very lightweight | Niche, limited ecosystem |

## Rationale
- Tokio is the de facto standard — virtually all async Rust crates target it
- Axum (chosen web framework) is built on Tokio
- SQLx (chosen DB driver) has Tokio runtime feature
- `tokio::spawn` is essential for the job executor background loop
- `tokio::time` provides accurate timers needed for job scheduling
- Large community, active development, battle-tested in production

## Spike Validation
- [ ] Spawn 10,000 tasks simultaneously — measure throughput
- [ ] Timer accuracy test: 1ms, 10ms, 1s, 1min intervals
- [ ] Memory usage under concurrent load
- [ ] Compatibility check with all other chosen libraries

## Consequences
- All async code targets the Tokio runtime
- `#[tokio::main]` entry point
- `#[tokio::test]` for async tests
