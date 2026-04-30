CREATE TABLE process_instances (
    id            UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    org_id        UUID        NOT NULL REFERENCES orgs (id) ON DELETE RESTRICT,
    definition_id UUID        NOT NULL REFERENCES process_definitions (id) ON DELETE RESTRICT,
    state         TEXT        NOT NULL CHECK (state IN ('running', 'suspended', 'completed', 'error', 'cancelled')),
    labels        JSONB       NOT NULL DEFAULT '{}',
    started_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ended_at      TIMESTAMPTZ
);

CREATE INDEX idx_process_instances_definition_id ON process_instances (definition_id);
CREATE INDEX idx_process_instances_org_id        ON process_instances (org_id);
CREATE INDEX idx_process_instances_state         ON process_instances (state);
CREATE INDEX idx_process_instances_running       ON process_instances (started_at) WHERE state IN ('running', 'error');
CREATE INDEX idx_process_instances_labels        ON process_instances USING GIN (labels);
