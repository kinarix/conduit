-- `setup_completed` gates the first-time setup wizard. Every new org is
-- created with FALSE; `db::orgs::create_org` flips it on wizard finish.
CREATE TABLE orgs (
    id              UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    name            TEXT        NOT NULL,
    slug            TEXT        NOT NULL UNIQUE,
    setup_completed BOOLEAN     NOT NULL DEFAULT FALSE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
