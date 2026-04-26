# ADR-003: Database Driver — SQLx

## Status
Proposed (pending Phase 0 spike)

## Context
The engine's entire state lives in PostgreSQL. Need a reliable, async-native driver.

## Decision
Use **SQLx 0.7.x** with PostgreSQL.

## Candidates Evaluated

| Library | Pros | Cons |
|---|---|---|
| SQLx | Compile-time checked queries, async native, no ORM overhead, migrations built in | Raw SQL (not a con for us) |
| Diesel | Mature ORM, type-safe query builder | Sync-first (async via wrapper), heavy |
| SeaORM | Async ORM, active record pattern | Newer, less battle-tested |

## Rationale
- Compile-time query checking catches SQL errors before runtime
- Native async — no sync-wrapped threadpool overhead
- `FOR UPDATE SKIP LOCKED` works exactly as written (critical for job executor)
- Built-in migration runner (`sqlx migrate`) — zero additional dependency
- `sqlx::test` macro makes testing with real DB trivial
- Raw SQL gives full control over query performance

## Spike Validation
- [ ] Concurrent transaction test: 10 writers to same row, verify serialisation
- [ ] FOR UPDATE SKIP LOCKED: 10 concurrent readers, verify no double-processing
- [ ] Transaction rollback on error: verify atomicity
- [ ] Migration: fresh DB, existing DB (idempotent)
- [ ] Query performance: EXPLAIN on all common patterns
