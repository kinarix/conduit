CREATE TABLE process_groups (
    id         UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    org_id     UUID        NOT NULL REFERENCES orgs (id) ON DELETE CASCADE,
    name       TEXT        NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (org_id, name)
);
