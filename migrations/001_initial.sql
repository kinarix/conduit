-- Migration 001: Initial schema
-- Creates extension and schema tracking table

CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Track schema version (proves migrations are running)
CREATE TABLE IF NOT EXISTS schema_info (
    version     INTEGER PRIMARY KEY,
    applied_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    description VARCHAR     NOT NULL
);

INSERT INTO schema_info (version, description)
VALUES (1, 'Initial schema — foundation phase');

-- ----------------------------------------------------------------------------

CREATE TABLE orgs (
    id         UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    name       TEXT        NOT NULL,
    slug       TEXT        NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
