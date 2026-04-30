CREATE TABLE parallel_join_state (
    id                UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    instance_id       UUID        NOT NULL REFERENCES process_instances (id) ON DELETE CASCADE,
    fork_execution_id UUID        NOT NULL UNIQUE REFERENCES executions (id) ON DELETE CASCADE,
    expected_count    INT         NOT NULL,
    arrived_count     INT         NOT NULL DEFAULT 0,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_parallel_join_state_instance ON parallel_join_state (instance_id);
