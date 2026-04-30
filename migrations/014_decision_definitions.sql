CREATE TABLE decision_definitions (
    id           UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    org_id       UUID        NOT NULL REFERENCES orgs (id),
    decision_key TEXT        NOT NULL,
    version      INT         NOT NULL DEFAULT 1,
    name         TEXT,
    dmn_xml      TEXT        NOT NULL,
    deployed_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (org_id, decision_key, version)
);

CREATE INDEX idx_decision_definitions_key
    ON decision_definitions (org_id, decision_key, version DESC);
