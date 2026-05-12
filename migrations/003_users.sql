-- Users are global identities. Org membership is an explicit relation
-- (see `org_members`). Email is globally unique, case-insensitive.
--
-- auth_provider:
--   'internal' — email + password_hash stored here
--   'external' — external_id holds the IdP subject claim; password_hash is NULL
CREATE TABLE users (
    id            UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    auth_provider TEXT        NOT NULL CHECK (auth_provider IN ('internal', 'external')),
    external_id   TEXT,
    email         TEXT        NOT NULL,
    password_hash TEXT,
    name          TEXT,
    phone         TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX users_email_lower_key ON users (LOWER(email));
