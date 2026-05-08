# Phase 20 — Deprecate `<conduit:http>` Connector

## Status
Not started

## Prerequisites
Phase 16 (HTTP push connector — being deprecated), Phase 17 (external-task long polling), Phase 21 (reference workers — landing alongside).

## Goal
Begin removal of the in-engine HTTP connector to bring the engine back in line with [ADR-008: Engine stays pure BPMN](../adr/ADR-008-engine-stays-pure-bpmn.md). After this phase, `<conduit:http>` continues to work but emits a deployment-time deprecation warning, and customers have a documented migration path to the reference HTTP worker (Phase 21).

Removal of the code itself happens in a follow-up phase (working title: Phase 22 — Remove `<conduit:http>`), gated on the reference HTTP worker being production-ready and at least one external user confirming a successful migration.

## Scope

### Included
- Parser emits a deprecation warning (logged + included in the deployment response) when a definition contains `<conduit:http>`.
- New error code `U010: deprecated extension element` (warning class, not blocking).
- Migration guide section in `docs/MIGRATION.md` (new file): step-by-step from `<conduit:http>` to a worker-based `serviceTask` with `conduit:taskTopic`.
- `README.md` notes `<conduit:http>` as deprecated with a pointer to the migration guide.
- `CLAUDE.md` "Active workstreams" entry updated to reflect deprecation.

### Excluded
- Removal of `src/engine/http.rs` and the `http_task` job type (separate follow-up phase).
- Migration of any internal test fixtures that still use `<conduit:http>` — those test the connector's behaviour and stay valid until the connector is removed.
- Any change to runtime behaviour of `<conduit:http>` — deprecated does not mean broken.

## Public API Changes

### Deployment response

`POST /api/v1/deployments` adds a `warnings` array when deprecated extensions are present:

```json
{
  "id": "…",
  "process_definition_key": "…",
  "version": 3,
  "warnings": [
    {
      "code": "U010",
      "message": "<conduit:http> is deprecated and will be removed in a future release. See docs/MIGRATION.md for the worker-based replacement."
    }
  ]
}
```

The deployment still succeeds. Deployments with no deprecated extensions return an empty array (or omit the field — TBD during implementation; pick the option that matches the existing handler style).

### Error codes

| Code | Class | Meaning |
|---|---|---|
| `U010` | warning | Extension element is deprecated; deployment succeeded but should be migrated |

`U010` does not appear in any HTTP error response body — it's a warning attached to a successful 201, not a 4xx.

## Migration Guide (`docs/MIGRATION.md`, new)

Outline:
1. Install the reference HTTP worker (link to Phase 21 docs).
2. For each `<conduit:http>` task:
   - Replace `<conduit:http>` extensionElements with `<conduit:taskTopic>http-call</conduit:taskTopic>`.
   - Move the URL, method, auth, transforms into the worker's task config (worker-specific; example given).
3. Deploy the new BPMN; existing instances continue under the old connector until they complete.
4. After migrating all definitions, follow-up phase will remove the old connector.

## Test Plan
- One new integration test deploying a BPMN with `<conduit:http>` and asserting the warning appears in the response.
- One new integration test deploying a BPMN without any deprecated elements and asserting the warnings array is empty / absent.
- Existing HTTP connector tests untouched — runtime behaviour is unchanged.

## Verification Checklist
- [ ] Failing tests written
- [ ] Parser warning + `U010` code wired through deployment response
- [ ] `docs/MIGRATION.md` complete with worker-side example
- [ ] `README.md` deprecation note added
- [ ] `CLAUDE.md` updated
- [ ] `make test` green
- [ ] `cargo clippy` / `cargo fmt` clean
