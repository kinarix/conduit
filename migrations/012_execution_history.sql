CREATE TABLE execution_history (
    id           UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    instance_id  UUID        NOT NULL REFERENCES process_instances (id) ON DELETE CASCADE,
    execution_id UUID        NOT NULL,
    element_id   TEXT        NOT NULL,
    element_type TEXT        NOT NULL,
    entered_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    left_at      TIMESTAMPTZ,
    worker_id    TEXT
);

CREATE INDEX idx_execution_history_instance_id  ON execution_history (instance_id);
CREATE INDEX idx_execution_history_execution_id ON execution_history (execution_id);
-- Hot path: close out the open history row for an execution on token advance.
CREATE INDEX idx_execution_history_active       ON execution_history (execution_id) WHERE left_at IS NULL;
