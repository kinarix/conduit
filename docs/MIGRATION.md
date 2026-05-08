# Migration Guide

This document collects breaking and deprecation migrations for Conduit. Each section is keyed by the change.

---

## `<conduit:http>` → worker-based serviceTask (Phase 20, deprecation)

**Status**: deprecated, runtime still works.
**Code surfaced**: `U010` in the deployment response `warnings` array.
**Removal**: planned for a follow-up phase, gated on at least one external user confirming a successful migration to the reference HTTP worker.

### Why
Per [ADR-008](adr/ADR-008-engine-stays-pure-bpmn.md), the engine speaks BPMN; workers speak protocols. The in-engine HTTP connector (Phase 16) was an inherited exception — it pulls `reqwest`, TLS state, retry classification, and HTTP status semantics into the orchestrator's address space, all of which belong in a worker process. We're moving it out.

### What changes (deployment-time)
A definition that contains `<conduit:http>` still deploys, still runs, still calls your URL. The deployment response now also includes:

```json
{
  "id": "…",
  "key": "…",
  "version": 3,
  "status": "deployed",
  "deployed_at": "2026-05-08T12:34:56Z",
  "warnings": [
    {
      "code": "U010",
      "element_id": "call_api",
      "message": "<conduit:http> is deprecated. Migrate to a worker-based serviceTask (conduit:taskTopic). See docs/MIGRATION.md."
    }
  ]
}
```

The same warning is also written to the engine log (`tracing::warn!`) at deploy time, with the process key and element id as structured fields, so existing log-based dashboards pick it up automatically.

A deployment with no deprecated elements returns `"warnings": []`.

### What changes (runtime)
**Nothing.** Existing process instances using `<conduit:http>` continue to fire HTTP calls exactly as before. There is no schema change, no job-row rewrite, no behavioural difference. The deprecation warning is purely a deployment-time advisory.

### How to migrate

#### 1. Replace the BPMN extension

Before (`<conduit:http>`):

```xml
<bpmn:serviceTask id="call_api">
  <bpmn:extensionElements>
    <conduit:http method="POST"
                  authType="bearer"
                  secretRef="orders_api_token"
                  xmlns:conduit="http://conduit.io/ext">
      <conduit:url>https://api.example.com/orders</conduit:url>
      <conduit:requestTransform>{ body: { items: .vars.items } }</conduit:requestTransform>
      <conduit:responseTransform>{ order_id: .body.id }</conduit:responseTransform>
      <conduit:retry max="3" backoffMs="500"/>
    </conduit:http>
  </bpmn:extensionElements>
</bpmn:serviceTask>
```

After (worker-based):

```xml
<bpmn:serviceTask id="call_api">
  <bpmn:extensionElements>
    <conduit:taskTopic xmlns:conduit="http://conduit.io/ext">http.call</conduit:taskTopic>
  </bpmn:extensionElements>
</bpmn:serviceTask>
```

The HTTP-call configuration (URL, method, auth, transforms, retries) **moves out of the BPMN** and into the worker's job config. The engine just delivers the task; the worker owns the protocol.

#### 2. Run the reference HTTP worker

The reference HTTP worker ships in the [`conduit-workers`](https://github.com/kinarix/conduit-workers) sibling repository (Phase 21). Two things need configuring:

- The engine address + auth credentials (so the worker can fetch tasks).
- The mapping from process variables to HTTP-call configuration (URL template, headers, body shape, response-to-variables mapping).

A representative `worker.yaml`:

```yaml
engine:
  url: https://conduit.example.com
  api_key_env: CONDUIT_API_KEY

handlers:
  http.call:
    url_template: "{{var:api_base}}/orders"
    method: POST
    auth:
      type: bearer
      secret_ref: orders_api_token
    request_template:
      body:
        items: "{{var:items}}"
    response_mapping:
      order_id: "$.id"
    retry:
      max: 3
      backoff_ms: 500
    idempotency:
      enabled: true
      key_template: "task-{{task_id}}"
```

The `idempotency.key_template` ensures retries don't duplicate side effects — see [PHASE-21](phases/PHASE-21-reference-workers.md) "Durable Execution Semantics" for the full picture.

#### 3. Drain in-flight HTTP connector tasks

If you're migrating a live system:

1. Deploy the new BPMN version with `<conduit:taskTopic>` to a **new** definition version. Old running instances continue under the old version (and still hit the engine's HTTP path).
2. Start the worker fleet so new instances pick up the new pattern.
3. Wait for old instances to drain naturally, or actively cancel and restart them under the new version if appropriate.
4. Once `SELECT count(*) FROM jobs WHERE job_type = 'http_task' AND state IN ('pending', 'locked')` returns 0, the runtime path is unused. The follow-up phase will remove the engine-side code.

#### 4. Verify

After migration, the deployment response should return `"warnings": []`. Search engine logs for `code=U010` to confirm no other definitions are still using `<conduit:http>` in your system.

### What to do if you can't migrate yet

`<conduit:http>` keeps working for the foreseeable future — there is no immediate action required. The warning lets you plan the migration on your own timeline. The follow-up "remove" phase will not ship until at least one external user has confirmed a clean migration to the reference HTTP worker.

### See also
- [ADR-008: Engine stays pure BPMN](adr/ADR-008-engine-stays-pure-bpmn.md) — the principle this migration reinforces
- [ADR-007: In-process connector framework — rejected](adr/ADR-007-connector-architecture.md) — record of the design we considered and rejected
- [Phase 20 spec](phases/PHASE-20-deprecate-http-connector.md)
- [Phase 21 spec — reference workers](phases/PHASE-21-reference-workers.md)
