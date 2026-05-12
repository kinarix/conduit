-- API keys for service accounts and CI. A key is presented as a Bearer token
-- with prefix `ck_` followed by a random secret. Only the prefix and the
-- argon2 hash of the full plaintext are stored. The plaintext is returned
-- exactly once, at creation time.
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

CREATE INDEX idx_api_keys_user_id ON api_keys (user_id);

-- One active key per prefix. Revoked rows keep their prefix for audit.
CREATE UNIQUE INDEX idx_api_keys_prefix_active
    ON api_keys (prefix)
    WHERE revoked_at IS NULL;
