# ADR-009 — Authentication and tenant isolation

**Status:** accepted (Phase 22)
**Date:** 2026-05-09
**Supersedes:** none. The structural plumbing in Phase 5.5 (`users`, `orgs`, `org_id` columns) explicitly deferred enforcement to a future phase. This ADR is that future phase.

## Context

Phase 5.5 added the schema for users and orgs but no enforcement: every endpoint was open, `org_id` was taken from request bodies and trusted blindly, and `owner_id` was recorded but never checked. Phase 22 closes the "anyone can call any endpoint and impersonate any org" hole.

Within the design space (JWT vs OIDC vs API keys vs sessions; permission-based vs role-based vs ABAC; per-deployment vs per-request tenancy mode), we picked the answers that minimise both new infrastructure and ongoing surface area while leaving room for RBAC (Phase 23) and worker tokens (Phase 24) to layer on without re-plumbing.

## Decision

### Authentication

Every endpoint except `/health`, `/metrics`, `/api/v1/auth/login`, and `/api/v1/external-tasks/*` (deferred to Phase 24) requires an `Authorization: Bearer <token>` header. The `Principal` extractor accepts two token shapes:

1. **Conduit-issued JWT (HS256).** `POST /api/v1/auth/login` validates `(org_slug, email, password)` against the `users` table (argon2id-hashed `password_hash`) and mints a JWT with claims `{sub: user_id, org: org_id, iat, exp, iss}`. TTL configurable via `CONDUIT_JWT_TTL_SECONDS` (default 3600). Signing key from `CONDUIT_JWT_SIGNING_KEY`.
2. **API key (`ck_<secret>`).** Long-lived per-user tokens for service accounts and CI. Stored as `(prefix, argon2(plaintext))`; the prefix is the cheap lookup, argon2 verifies the full string. Plaintext is returned exactly once at creation, never echoed afterwards.

External IdP / OIDC verification is **not** in scope for this round. The schema's `auth_provider='external'` column is the future expansion point.

### Authorisation

Phase 22 enforces tenant isolation only — every authenticated user is treated as full-access *within* their own org. Concretely:

- Handlers receive the `Principal` and use `principal.org_id` for all queries. Body-supplied `org_id` fields have been removed from request types (not silently ignored — the field is gone, so a stale client gets a JSON deserialisation error).
- For URLs that include an org segment (e.g. `/orgs/{org_id}/secrets`), the path `org_id` MUST equal `principal.org_id`; mismatch returns `404 NotFound` (we deliberately do not distinguish "no such resource" from "no such resource in your org").
- For ID-based endpoints (e.g. `GET /process-instances/{id}`), the resource is fetched and its `org_id` is compared to `principal.org_id` before any work proceeds. Mismatch → 404.

RBAC (per-handler permission checks like `principal.require(Permission::ProcessDeploy)`) is Phase 23. The `U403 Forbidden` code is reserved now to avoid renumbering later.

### Tenancy mode (pluggable)

A single env var, `CONDUIT_TENANT_ISOLATION = multi | single` (default `multi`), controls behaviour:

- `multi`: many orgs share one deployment. `Principal::org_id` is the security boundary. Cross-org access is impossible (every query is scoped).
- `single`: one org per deployment. The same code path runs, but the bootstrap admin env vars are required so the deployment is reachable on first boot.

There is **no `auth=disabled` bypass.** Tests mint real JWTs via `tests/common/auth::mint_jwt` so an authenticated path is always exercised.

### Bootstrap

When the `users` table is empty on startup:

- If `CONDUIT_BOOTSTRAP_ADMIN_{EMAIL,PASSWORD,ORG_SLUG}` are all set: an org and an internal-auth admin user are created. A loud `tracing::warn!` records the event.
- Else if `CONDUIT_TENANT_ISOLATION=single`: the process refuses to start. (A single-tenant deployment with no admin is unreachable.)
- Else: the process starts but logs a warning that no users exist yet. The operator must create the first user out-of-band (DB insert) before any API call succeeds.

## Consequences

- **No external IdP yet.** Customers who want OIDC must wait for a follow-up phase. The schema is ready (`auth_provider='external'`, `external_id`); only the verification path is missing.
- **One JWT signing key per deployment.** Rotation requires either (a) a brief outage to swap the key + invalidate every outstanding JWT, or (b) a follow-up to support multiple `kid`s. Phase 22 ships option (a). API keys are unaffected by JWT rotation.
- **Workers are still open** until Phase 24. The deferral is loud: the external-task endpoints are unauthenticated and `db::jobs::fetch_and_lock_many` does not filter by org. Operators running multi-tenant deployments must firewall worker traffic until Phase 24.
- **Body-`org_id` is a breaking change.** Clients previously sending `{"org_id": "...", ...}` will get a JSON-deserialisation error rather than silent miscoercion. This is the intended migration: the field is gone, so mistakes are visible.
- **Tests now require a JWT.** `spawn_test_app` builds a default principal and an authed reqwest client; tests use `app.client.clone()` and `app.principal.org_id`. Tests that need cross-tenant scenarios call `create_principal()` for a second org.

## Alternatives considered

- **External IdP / OIDC in v1.** Rejected: doubles the surface area (JWKS fetching, multi-issuer claim mapping, key rotation) for a population we don't yet have. Easy to add later — the schema is ready.
- **Bypass flag for tests / single-instance dev.** Rejected per advisor: bypass flags ship to prod by accident.
- **Two code paths for single- vs multi-tenant.** Rejected: one config flag, one extractor, one set of handlers. The single-tenant deployment runs the multi-tenant code with `org_id = root_org` everywhere.
- **403 instead of 404 for cross-tenant requests.** Rejected for ID-based endpoints: 403 leaks "this exists in another org you can't see." 404 is honest; 403 reserved for future "you're authenticated but lack the role" cases.
