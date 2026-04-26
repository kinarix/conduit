# Phase 1 — Foundation

## Goal
A running Rust service: connects to PostgreSQL, runs migrations, serves GET /health.
No process logic yet. Just the skeleton everything else builds on.

## Duration
1 week

## Prerequisites
- Phase 0 complete (all ADRs accepted)
- Docker installed locally

## Tasks

### 1.1 Project Initialisation
```bash
cargo new conduit
cd conduit
# Set up Cargo.toml with all dependencies
# Set up workspace if needed later
```

### 1.2 Directory Structure
Create exactly the structure in CLAUDE.md.

### 1.3 Config Module
- Read all config from environment variables
- Use dotenvy for .env file loading
- Fail fast with clear error if required vars missing
- Provide defaults for optional vars

### 1.4 Error Module
- Single `EngineError` enum for all error types
- `impl IntoResponse` for axum integration
- Never expose internal error details to HTTP callers
- Always log internal errors with tracing

### 1.5 DB Module
- Create PgPool from DATABASE_URL
- Set connection pool limits (min=2, max=10 to start)
- Test DB connectivity on startup, fail fast if unreachable

### 1.6 Migrations
- migrations/001_initial.sql
- Verify: `CREATE EXTENSION IF NOT EXISTS "uuid-ossp"`
- Run automatically on startup before serving requests

### 1.7 Health Endpoint
- GET /health
- Check DB with `SELECT 1`
- Return JSON with status, db_status, version
- Return 200 even if DB is degraded (let caller decide)

### 1.8 Main
- Load config
- Init tracing (structured, env-filter controlled)
- Connect DB pool
- Run migrations
- Build Axum router
- Bind and serve

### 1.9 Docker Compose
- PostgreSQL 16 with health check
- Named volume for data persistence
- .env.example with all required variables

### 1.10 CI Pipeline
- GitHub Actions workflow
- Steps: checkout, rust toolchain, cache, fmt check, clippy, test
- PostgreSQL service in CI

## Tests

### Unit Tests
```rust
// config.rs
#[test]
fn config_fails_without_database_url() { ... }

// error.rs  
#[test]
fn not_found_error_returns_404() { ... }
```

### Integration Tests
```rust
// tests/health_test.rs
#[tokio::test]
async fn health_returns_ok_when_db_connected() { ... }

#[tokio::test]
async fn health_returns_degraded_when_db_down() { ... }
```

## Status: Complete (2026-04-26) — CI pipeline pending

## Acceptance Criteria
- [x] `docker-compose up -d` starts PostgreSQL
- [x] `cargo run` starts the engine with no errors
- [x] `curl http://localhost:8080/health` returns `{"status":"ok"}`
- [x] `cargo test` passes all tests
- [x] `cargo clippy -- -D warnings` is clean
- [x] `cargo fmt --check` is clean
- [ ] CI pipeline is green — `.github/workflows/ci.yml` not yet created

## What was built
- `src/lib.rs` — exposes public modules so integration tests can import `conduit::`
- `src/config.rs` — env-driven config with dotenvy, fail-fast on missing DATABASE_URL
- `src/error.rs` — `EngineError` enum with `IntoResponse`, never leaks DB internals
- `src/db.rs` — PgPool with configurable min/max connections and acquire timeout
- `src/api/health.rs` — GET /health with SELECT 1 DB probe, returns status/database/version
- `src/main.rs` — config → tracing → DB → migrations → Axum → serve
- `migrations/001_initial.sql` — uuid-ossp extension, schema baseline
- `docker-compose.yml` — Postgres 16 with health check and named volume
- `Makefile` — targets: db, migrate, test, check, fmt, lint, build, run, clean
- `tests/common/mod.rs` — `spawn_test_app()` helper using real DB
- `tests/health_test.rs` — happy-path health check + version field tests

## Known gaps
- `.env.example` not created
- `health_returns_degraded_when_db_down` integration test not implemented
- CI pipeline not created

## Files to Create
```
Cargo.toml
.env.example
.gitignore
docker-compose.yml
.github/workflows/ci.yml
migrations/001_initial.sql
src/main.rs
src/config.rs
src/error.rs
src/db.rs
src/api/mod.rs
src/api/health.rs
tests/common/mod.rs
tests/health_test.rs
```
