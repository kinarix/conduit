-- Phase 16: HTTP connector secrets.
-- Org-scoped, app-encrypted credentials referenced by name from BPMN.
-- Values are encrypted via ChaCha20-Poly1305 with a random per-row nonce;
-- the master key lives in the CONDUIT_SECRETS_KEY env var, not in the DB.
CREATE TABLE secrets (
    id              UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    org_id          UUID        NOT NULL REFERENCES orgs (id) ON DELETE CASCADE,
    name            TEXT        NOT NULL,
    value_encrypted BYTEA       NOT NULL,
    nonce           BYTEA       NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (org_id, name)
);

CREATE INDEX idx_secrets_org_id ON secrets (org_id);
