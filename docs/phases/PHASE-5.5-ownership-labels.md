# Phase 5.5 — Ownership + Labels

## Status
**Complete** — 2026-04-26

## Prerequisites
Phase 5 complete and all tests passing.

## Summary

Structural plumbing for multi-tenancy and filtered search. Every process definition and instance is now owned by an org, optionally attributed to a user, and tagged with arbitrary JSONB labels. Auth enforcement (middleware, JWTs) is explicitly out of scope — this phase installs the schema and wires the data through all layers.

## Checklist
- [x] Failing tests written
- [x] Implementation complete
- [x] All tests passing (77 tests)
- [x] cargo clippy clean
- [x] cargo fmt clean
- [ ] CI green

## What was built

### Schema changes (migrations modified in-place)

**`migrations/001_initial.sql`** — added `orgs` table:
```sql
CREATE TABLE orgs (
    id         UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    name       TEXT        NOT NULL,
    slug       TEXT        NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

**`migrations/002_core_schema.sql`** — added:
- `users` table with `auth_provider IN ('internal', 'external')`, `org_id` FK, `email` unique per org
- `org_id` + `owner_id` + `labels JSONB` on `process_definitions`; UNIQUE constraint scoped to `(org_id, process_key, version)`
- `org_id` + `labels JSONB` on `process_instances`
- GIN indexes on both `labels` columns for JSONB containment queries

### Rust layer

| File | Change |
|---|---|
| `src/db/models.rs` | Added `Org`, `User`; added `org_id`, `owner_id`, `labels` to `ProcessDefinition` and `ProcessInstance` |
| `src/db/orgs.rs` | New — `insert(pool, name, slug) → Org` |
| `src/db/users.rs` | New — `insert(pool, org_id, auth_provider, external_id, email) → User` |
| `src/db/process_definitions.rs` | `insert` gains `org_id`, `owner_id`, `labels`; `next_version` scoped to `org_id`; added `list_by_org` |
| `src/db/process_instances.rs` | `insert` gains `org_id`, `labels` |
| `src/engine/mod.rs` | `start_instance(definition_id, org_id, labels)` passes through to DB insert |
| `src/api/deployments.rs` | `DeployRequest` gains `org_id`, `owner_id?`, `labels?` |
| `src/api/instances.rs` | `StartInstanceRequest` gains `org_id`, `labels?` |
| `src/api/orgs.rs` | New — `POST /api/v1/orgs` → `201 Org` |
| `src/api/users.rs` | New — `POST /api/v1/users` → `201 User` |
| `src/config.rs` | Added `AUTH_PROVIDER` env var (`internal` \| `external`) |

### Tests

- `tests/common/mod.rs` — added `create_test_org(app) → Uuid` helper
- All existing tests updated to create an org and pass `org_id` through deploy + start requests
- 77 total tests passing (up from 51)

## Key decisions

- **Modified existing migrations** (not new files) — Phase 5.5 is structural plumbing that belongs with the original schema, not a schema evolution on top of it. DB must be reset with `docker-compose down -v` when developing locally.
- **`auth_provider` column** on `users` records how a user authenticates (`internal` = password in DB, `external` = IdP subject claim). No auth middleware wired up yet — this is purely structural.
- **`#[allow(clippy::too_many_arguments)]`** on `process_definitions::insert` — 8 args exceeds clippy's default max (7). A wrapper struct would be premature abstraction for a single DB insert function.
- **UNIQUE constraint scoped to org** — `(org_id, process_key, version)` allows two orgs to deploy the same `process_key` without collision.
- **`next_version` scoped to org** — version numbering restarts per org, consistent with the UNIQUE constraint.
