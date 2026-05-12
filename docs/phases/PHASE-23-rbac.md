# Phase 23 — Role-Based Access Control

Builds on Phase 22 (AuthN + path-based org context) by adding fine-grained authorization. Phase 22 answered "who is calling?". Phase 23 answers "what are they allowed to do?".

Phase 23 shipped in two checkpoints:

- **23.1** — backend permission catalog, three-scope role assignments, path-based org context, UI scoped-RBAC refactor.
- **23.2** — member cascade, integration tests, formatting cleanup.

## Goals

1. Authorize every protected endpoint against an explicit permission, not against role names or tenant ownership alone.
2. Allow grants at three levels — **global**, **organisation**, **process group** — with cascade so users don't have to be re-granted in every nested scope.
3. Preserve tenant isolation: a user must be a member of an org (or a global admin) to operate in it.
4. Keep the catalog small, enumerable, and asserted at startup so adding a permission can't be forgotten in the migration.

## What shipped

### Schema (migrations 021–024)

- `021_roles.sql` — `roles` (org-scoped or global), `role_permissions` with a CHECK that enumerates the catalog, and seed rows for eight built-in global roles.
- `022_org_members.sql` — `(user_id, org_id, invited_by, joined_at)`, indexed on both keys.
- `023_role_assignments.sql` — `global_role_assignments` and `org_role_assignments`.
- `024_process_group_role_assignments.sql` — PG-scoped grants; composite FK to `org_members(user_id, org_id)` makes PG grants follow membership, plus a trigger that asserts the assignment's `org_id` equals the PG's `org_id`.

### Permission catalog

`src/auth/permission.rs` defines the `Permission` enum with 55 variants. Each variant maps to a dotted string used in the database and on the wire. A startup test (`permission_catalog_in_sync_with_migration`) parses `021_roles.sql`'s CHECK block and asserts the two sets are equal.

Partition:

- **29 org-only** (with one — `OrgCreate` — global-only): organisation, membership, user identity, role/role-assignment management, auth config, process-group creation, secrets, API-key management, message/signal correlation.
- **26 org+PG**: process / decision / instance / task / external-task / process-group read-write / process layout.

`Permission::is_pg_scopable()` and `Permission::is_global_only()` codify the partition; a unit test rejects any permission whose classification disagrees with its membership in the org-only list.

### Built-in roles

Seeded by `021_roles.sql`:

| Role | Permissions | Notes |
|------|-------------|-------|
| `PlatformAdmin` | all 55 | Granted only globally. Bootstrap admin. Bypasses membership. |
| `OrgOwner` | 51 | All in-org powers minus `org.create`, `user.create`, `user.update`, `user.delete`. |
| `OrgAdmin` | 18 | Identity / role / auth config management only. |
| `Developer` | 29 | Full process and decision lifecycle inside the org. |
| `Operator` | 15 | Run/monitor; read-only on designs. |
| `Modeller` | 13 | Design drafts; no deploy, no run. |
| `Reader` | 12 | Every `*.read*` permission. |
| `Worker` | 3 | `external_task.execute`, `process.read`, `decision.read`. |

Orgs may also define custom roles drawn from the same catalog via `POST /api/v1/orgs/{org_id}/roles`. Custom role names are unique within an org.

### Principal

`src/auth/principal.rs::Principal` carries:

```rust
user_id:           Uuid
email:             String
current_org_id:    Option<Uuid>     // from path /api/v1/orgs/{org_id}/…
is_global_admin:   bool
permissions:       HashSet<Permission>   // global ∪ org (if current_org_id is set)
pg_permissions:    HashMap<Uuid, HashSet<Permission>>   // per PG, extras only
kind:              PrincipalKind    // Jwt or ApiKey
```

Permission checks:

- `require(perm)` — succeeds if `perm ∈ permissions`. Used on org-only checks.
- `require_in_pg(perm, pg_id)` — succeeds if `perm ∈ permissions` (cascade) OR `perm ∈ pg_permissions[pg_id]`.
- `pg_ids_with(perm)` — for filtered list endpoints. Returns `None` if the permission is held at org scope (cascades), or `Some(set)` of allowed PGs otherwise.

### Path-based org context

Every org-scoped endpoint is nested under `/api/v1/orgs/{org_id}/…`. The extractor parses `{org_id}` from the matched route via `RawPathParams`. When present:

1. Parse as UUID.
2. If not a global admin, check `org_members` — `403` if missing.
3. Load org-level permissions.
4. Load PG-level grants for every PG the user holds extras in (within this org).

When absent, `current_org_id = None` and only global permissions are loaded. Handlers that need an org context but find none return `400` ("missing org context").

### Member cascade

Removing a user from `org_members` triggers CASCADE deletes on:

- `org_role_assignments(user_id, org_id)` for the removed pair.
- `process_group_role_assignments` joined to PGs of that org.

Enforced by composite foreign keys in `023_role_assignments.sql` and `024_process_group_role_assignments.sql`: each scoped-grant table carries `FOREIGN KEY (user_id, org_id) REFERENCES org_members(user_id, org_id) ON DELETE CASCADE`, so a `DELETE FROM org_members` row removal cascades to every grant that user held inside that org. No separate trigger needed.

### Tests

- `tests/rbac_global_test.rs` — global admin bypasses membership; org admin without grants is rejected.
- `tests/rbac_scoped_test.rs` — org grant cascades to PG; PG-only grant is confined to that PG.
- `tests/membership_test.rs` — removing a member CASCADE-clears their grants.
- `tests/bootstrap_test.rs` — env-var bootstrap creates a global admin, not an org member.
- `tests/auth_test.rs` — JWT vs API key vs missing header surface the right status codes.

## Out of scope (deferred)

- **OIDC runtime flow.** Schema and config API exist (`020_org_auth_config.sql`); token-exchange endpoints, JWKS fetching, and user provisioning are deferred.
- **Worker token model.** Currently `external_task.execute` is gated by the same Principal extraction; a future phase may introduce per-worker scoped tokens.
- **Multi-`kid` JWT rotation.** Single signing key today; rotation requires restart.

## Pointers

- ADR: `docs/adr/ADR-009-auth-architecture.md` (Amendment 2026-05-12).
- Permission catalog: `src/auth/permission.rs`.
- Principal: `src/auth/principal.rs`.
- Bootstrap: `src/auth/bootstrap.rs`.
- User-facing docs: [Administration](https://conduit.kinarix.com/docs/admin/) on the website.
