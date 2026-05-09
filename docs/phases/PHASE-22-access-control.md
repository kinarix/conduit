# Phase 22 — Access Control & Authorization

**Goal:** authentication + tenant isolation. Every request identifies a `Principal`; every handler scopes work to that principal's org. Closes the critical "anyone can claim any org" hole left open by Phase 5.5.

**Status:** complete.

## Scope

In:
- JWT (HS256) issued by `POST /api/v1/auth/login`.
- Long-lived API keys (`ck_…`) for service accounts and CI.
- `Principal` extractor + middleware-style enforcement on every protected endpoint.
- Body-supplied `org_id` removed from all request types.
- Path-supplied `org_id` (e.g. `/orgs/{org_id}/secrets`) verified against `principal.org_id`.
- Bootstrap admin via env vars on first boot.
- Pluggable tenant mode: `CONDUIT_TENANT_ISOLATION = multi | single`.
- Test infrastructure: `tests/common/auth.rs`, JWT-authed reqwest client on `TestApp`.

Out (deferred):
- RBAC / per-handler permission gates → Phase 23.
- Worker authentication (`/external-tasks/*`) → Phase 24.
- External IdP / OIDC verification → not yet scheduled.
- JWT signing-key rotation with multiple `kid`s → not yet scheduled.

## What lands

### Schema

`migrations/020_api_keys.sql`:
```sql
CREATE TABLE api_keys (
    id           UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id      UUID        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    name         TEXT        NOT NULL,
    prefix       TEXT        NOT NULL,
    key_hash     TEXT        NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used_at TIMESTAMPTZ,
    revoked_at   TIMESTAMPTZ
);
CREATE UNIQUE INDEX idx_api_keys_prefix_active ON api_keys (prefix) WHERE revoked_at IS NULL;
```

The `users` table already had `password_hash` (Phase 5.5) — Phase 22 just starts populating it.

### New module: `src/auth/`

| File | Purpose |
|---|---|
| `src/auth/mod.rs` | `AuthSettings { jwt_keys, jwt_ttl, jwt_issuer, tenant_isolation }` |
| `src/auth/jwt.rs` | HS256 encode/decode; `Claims { sub, org, iat, exp, iss }` |
| `src/auth/password.rs` | `argon2id` hash + verify |
| `src/auth/api_key.rs` | `ck_…` generate / extract-prefix / verify |
| `src/auth/principal.rs` | `Principal { user_id, org_id, email, kind }` |
| `src/auth/bootstrap.rs` | first-boot admin seeding |

### New endpoints (`src/api/auth.rs`)

| Method + Path | Auth | Purpose |
|---|---|---|
| `POST /api/v1/auth/login` | public | `{email, password, org_slug}` → `{access_token, token_type, expires_in}` |
| `GET /api/v1/me` | required | Current principal summary |
| `POST /api/v1/api-keys` | required | Mint a key. **Plaintext returned once.** |
| `GET /api/v1/api-keys` | required | List keys for the calling user (no plaintext, no hash) |
| `DELETE /api/v1/api-keys/{id}` | required | Soft-revoke a key the caller owns |

### Principal extractor (`src/api/extractors.rs`)

`impl FromRequestParts<Arc<AppState>> for Principal` — reads `Authorization: Bearer <token>`, dispatches:
- Token starting with `ck_` → API-key path: `db::api_keys::lookup_by_prefix` → argon2 verify → fire-and-forget `last_used_at` update.
- Otherwise → JWT path: `auth::jwt::decode_token` (signature/issuer/expiry) → `db::users::find_by_id` (subject still exists) → org claim still matches user's row.

Any failure → `EngineError::Unauthenticated` (`U401`). Token is never logged. The body never tells the caller *which* check failed.

### Error codes

`src/error_codes.toml` adds:
- `U011 LoginFailed` — generic, never leaks email-vs-password.
- `U401 Unauthenticated` — generic, never leaks why.
- `U403 Forbidden` — reserved for Phase 23 RBAC; emitted in Phase 22 only by tenant-isolation guards on path-org mismatch.

### Tenant isolation

- Body `org_id` field removed from `DeployRequest`, `StartInstanceRequest`, `CreateUserRequest`, `CorrelateMessageRequest`, `BroadcastSignalRequest`, `CreateProcessGroupRequest`, etc.
- Path `org_id` (secrets, process_layouts) verified equal to `principal.org_id` — mismatch → 404.
- ID-based GETs (instances, definitions, tasks) fetch the resource and check `row.org_id == principal.org_id` — mismatch → 404. Never 403, so existence isn't leaked.

### Bootstrap

`src/auth/bootstrap.rs::run_if_needed`:
- If `users` count > 0 → no-op.
- Else if `CONDUIT_BOOTSTRAP_ADMIN_{EMAIL,PASSWORD,ORG_SLUG}` set → create org + internal admin, log warning.
- Else if `CONDUIT_TENANT_ISOLATION=single` → `anyhow::bail!`.
- Else → log warning that no users exist; operator must create out-of-band.

### Test infrastructure

`tests/common/mod.rs` now exposes:
- `TestApp { address, pool, principal, client }` — `client` has the principal's JWT in default headers.
- `create_principal(&pool, slug_prefix) -> TestPrincipal { user_id, org_id, token }` — for tests that need a second org.
- `auth::mint_jwt`, `auth::authed_client` for fine-grained scenarios.

`tests/auth_test.rs` covers:
- Login success, wrong password (U011), unknown user (U011), external-auth user attempting login.
- JWT missing / garbage / expired / wrong-signature / deleted-user → U401.
- API key mint → use → revoke → reject; cross-user listing; cross-user revoke.
- `/me` returns the principal summary; `auth_kind` distinguishes JWT from API key.
- Cross-org instance GET and cross-org secret GET both return 404.
- `/health` and `/auth/login` remain public.

### Engine method updates

`engine::correlate_message` and `engine::broadcast_signal` already accepted `org_id` (Phase 10/11). The handlers now pass `principal.org_id` instead of `req.org_id`. No engine signature changes required.

### DB layer updates

| Function | Change |
|---|---|
| `db::tasks::list_pending_paginated` | Now takes `org_id`; SQL JOINs `process_instances` to scope to org. |
| `db::users::insert` | Now takes `Option<&str> password_hash`. |
| `db::users::find_credentials_by_org_slug_and_email` | New — login lookup. |
| `db::users::find_by_id` | New — used by extractor to confirm JWT subject still exists. |
| `db::users::count` | New — bootstrap idempotency check. |
| `db::orgs::get_by_id` | New — `/orgs` listing under principal scope. |
| `db::process_groups::get_by_id` | New — tenant-isolation guards. |
| `db::api_keys::*` | New module — insert / lookup_by_prefix / list_by_user / revoke / touch_last_used. |

## Configuration

| Env var | Required? | Default | Purpose |
|---|---|---|---|
| `CONDUIT_JWT_SIGNING_KEY` | yes | — | HS256 signing/verification key |
| `CONDUIT_JWT_TTL_SECONDS` | no | `3600` | Access-token lifetime |
| `CONDUIT_JWT_ISSUER` | no | `"conduit"` | `iss` claim |
| `CONDUIT_TENANT_ISOLATION` | no | `multi` | `multi` or `single` |
| `CONDUIT_BOOTSTRAP_ADMIN_EMAIL` | required when single + no users | — | First-boot admin |
| `CONDUIT_BOOTSTRAP_ADMIN_PASSWORD` | ↑ | — | First-boot admin |
| `CONDUIT_BOOTSTRAP_ADMIN_ORG_SLUG` | ↑ | — | First-boot admin |

## Verification

End-to-end:
1. `make test` — 471 tests across 25 suites pass.
2. `tests/auth_test.rs` covers the auth matrix.
3. Manual smoke (single-tenant):
   ```bash
   docker-compose up -d
   CONDUIT_JWT_SIGNING_KEY=$(openssl rand -base64 64) \
   CONDUIT_TENANT_ISOLATION=single \
   CONDUIT_BOOTSTRAP_ADMIN_EMAIL=admin@local \
   CONDUIT_BOOTSTRAP_ADMIN_PASSWORD=hunter2-please \
   CONDUIT_BOOTSTRAP_ADMIN_ORG_SLUG=root \
   cargo run
   # Then:
   curl -X POST localhost:8080/api/v1/auth/login \
     -H "Content-Type: application/json" \
     -d '{"email":"admin@local","password":"hunter2-please","org_slug":"root"}'
   # → {"access_token":"eyJ...","token_type":"Bearer","expires_in":3600}
   curl localhost:8080/api/v1/me -H "Authorization: Bearer eyJ..."
   curl localhost:8080/api/v1/deployments     # → 401 U401
   ```

## Migration notes for clients

- Strip `"org_id": "..."` from every request body. The field is rejected.
- Add `Authorization: Bearer <token>` to every call (except `/health`, `/auth/login`).
- Header `X-Org-Id` (previously read by the decisions API) is no longer consulted.
- Workers (`/external-tasks/*`) are unaffected by Phase 22 — they are still unauthenticated until Phase 24. Firewall accordingly in multi-tenant deployments.

## What's next

- **Phase 23 — RBAC**: `roles` and `user_roles` tables; permission enum; `principal.require(perm)`. Built-in roles: Admin / Deployer / Operator / Reader. The bootstrap admin gets `Admin`.
- **Phase 24 — Worker tokens**: `workers` table; topic-scoped tokens (`wt_…`); `db::jobs::fetch_and_lock_many` gains `org_id` + `allowed_topics`. Coordinated with the reference HTTP worker (Phase 21, sibling repo).
