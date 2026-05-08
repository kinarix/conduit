CREATE TABLE tasks (
    id           UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    instance_id  UUID        NOT NULL REFERENCES process_instances (id) ON DELETE CASCADE,
    execution_id UUID        NOT NULL REFERENCES executions (id) ON DELETE CASCADE,
    element_id   TEXT        NOT NULL,
    name         TEXT,
    task_type    TEXT        NOT NULL CHECK (task_type IN ('user_task', 'service_task')),
    assignee     TEXT,
    state        TEXT        NOT NULL CHECK (state IN ('pending', 'active', 'completed', 'cancelled')),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    due_date     TIMESTAMPTZ,
    completed_at TIMESTAMPTZ
);

CREATE INDEX idx_tasks_instance_id      ON tasks (instance_id);
CREATE INDEX idx_tasks_execution_id     ON tasks (execution_id);
CREATE INDEX idx_tasks_state            ON tasks (state);
CREATE INDEX idx_tasks_pending          ON tasks (created_at) WHERE state = 'pending';
-- Hot path: resolve the live task for a given element on an instance.
CREATE INDEX idx_tasks_instance_element ON tasks (instance_id, element_id);
