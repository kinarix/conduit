# ADR-006: Database Migrations — SQLx Migrate

## Status
Proposed (pending Phase 0 spike)

## Context
The DB schema evolves with each phase. Need reliable, repeatable migrations.

## Decision
Use **SQLx built-in migration runner** (`sqlx migrate`).

## Candidates Evaluated

| Tool | Pros | Cons |
|---|---|---|
| SQLx migrate | Built into SQLx (no extra dep), runs at startup, embedded in binary | Less GUI tooling |
| Refinery | Language-agnostic, good CLI | Extra dependency |
| Flyway | Industry standard, great tooling | External Java tool, extra ops burden |

## Rationale
- Already using SQLx — zero extra dependency
- `sqlx::migrate!()` macro embeds migrations into binary at compile time
- Runs automatically at startup — no ops step needed
- Checksums prevent partial/corrupted migrations
- Works perfectly in CI (just needs a DB)

## Migration Conventions
- Files in `migrations/` directory
- Naming: `NNN_description.sql` (e.g. `001_initial.sql`, `002_process_definitions.sql`)
- Sequential numbering — never reuse a number
- Never modify an applied migration — always add a new one
- Each migration is a single transaction where possible

## Spike Validation
- [ ] Fresh DB: migrations apply in correct order
- [ ] Existing DB with some migrations: only new ones run
- [ ] Corrupt checksum: migration refuses to run, good error message
- [ ] CI pipeline: migrations run before tests
