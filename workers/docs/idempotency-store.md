# Idempotency store

For handlers whose side effect cannot be made idempotent purely by an `Idempotency-Key` header (e.g. an upstream API that ignores the header), the worker fleet maintains a small dedupe table keyed by `(task_id, attempt)` with the response payload. On retry, a successful prior response is replayed instead of re-issuing the call.

> **One store per worker fleet, not per process.** Durability of the dedupe table is what makes a handler safe across worker restarts. Two workers that handle the same topic against the same engine MUST share the same store.

## Schema (Postgres)

```sql
CREATE TABLE worker_idempotency (
    task_id     UUID NOT NULL,
    attempt     INT  NOT NULL,
    handler     TEXT NOT NULL,             -- e.g. "http.call"
    request_hash BYTEA NOT NULL,           -- sha256 of canonical request body
    response_status SMALLINT NOT NULL,
    response_body BYTEA NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at  TIMESTAMPTZ NOT NULL,      -- created_at + retention TTL
    PRIMARY KEY (task_id, attempt)
);

CREATE INDEX worker_idempotency_expires_at_idx
    ON worker_idempotency (expires_at);
```

## Retention

Default: 14 days from `created_at`. Run a periodic cleanup job:

```sql
DELETE FROM worker_idempotency WHERE expires_at < NOW();
```

If your retry policy spans longer than 14 days, raise the TTL accordingly. Conduit's default `retries=3` with a few minutes between attempts means most rows are ephemeral.

## Wire-up (planned)

The `http-worker` MVP relies on `Idempotency-Key` only. The dedupe-table path is wired up in a follow-up commit; this doc records the schema so customers can stand it up against the same Postgres instance the engine uses (different database, or just a different schema, is fine).

## Redis adapter (planned)

For fleets that prefer Redis, the same shape lands as a `(SET task:{task_id}:{attempt} {payload} EX 1209600)` plus a small Lua check-and-replay script. Pending.
