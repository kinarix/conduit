CREATE TABLE process_definitions (
    id               UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    org_id           UUID        NOT NULL REFERENCES orgs (id) ON DELETE RESTRICT,
    owner_id         UUID        REFERENCES users (id) ON DELETE SET NULL,
    process_group_id UUID        NOT NULL REFERENCES process_groups (id) ON DELETE RESTRICT,
    process_key      TEXT        NOT NULL,
    version          INTEGER     NOT NULL,
    name             TEXT,
    bpmn_xml         TEXT        NOT NULL,
    status           TEXT        NOT NULL DEFAULT 'deployed'
                                     CHECK (status IN ('draft', 'deployed')),
    labels           JSONB       NOT NULL DEFAULT '{}',
    deployed_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uq_process_definitions_org_key_version UNIQUE (org_id, process_key, version)
);

CREATE INDEX idx_process_definitions_key           ON process_definitions (process_key);
CREATE INDEX idx_process_definitions_org_id        ON process_definitions (org_id);
CREATE INDEX idx_process_definitions_process_group ON process_definitions (process_group_id);
CREATE INDEX idx_process_definitions_labels        ON process_definitions USING GIN (labels);

-- Only one draft allowed per (org_id, process_key)
CREATE UNIQUE INDEX process_definitions_one_draft_per_key
    ON process_definitions (org_id, process_key)
    WHERE status = 'draft';
