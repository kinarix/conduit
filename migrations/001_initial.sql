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
