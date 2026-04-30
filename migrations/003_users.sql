-- auth_provider:
--   'internal' — email + password_hash stored here
--   'external' — external_id holds the IdP subject claim; password_hash is NULL
CREATE TABLE users (
    id            UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    org_id        UUID        NOT NULL REFERENCES orgs (id) ON DELETE CASCADE,
    auth_provider TEXT        NOT NULL CHECK (auth_provider IN ('internal', 'external')),
    external_id   TEXT,
    email         TEXT        NOT NULL,
    password_hash TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uq_users_org_email UNIQUE (org_id, email)
);

CREATE INDEX idx_users_org_id ON users (org_id);
