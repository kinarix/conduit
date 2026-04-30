-- parent_id supports subprocess and parallel gateway
CREATE TABLE executions (
    id          UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    instance_id UUID        NOT NULL REFERENCES process_instances (id) ON DELETE CASCADE,
    parent_id   UUID        REFERENCES executions (id) ON DELETE CASCADE,
    element_id  TEXT        NOT NULL,
    state       TEXT        NOT NULL CHECK (state IN ('active', 'completed', 'cancelled')),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_executions_instance_id ON executions (instance_id);
CREATE INDEX idx_executions_parent_id   ON executions (parent_id);
