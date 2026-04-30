-- Audit log for process execution: variable changes, job state transitions,
-- message/signal correlations, errors, per-element snapshots.
CREATE TABLE process_events (
    id           UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    instance_id  UUID        NOT NULL REFERENCES process_instances (id) ON DELETE CASCADE,
    execution_id UUID,
    event_type   TEXT        NOT NULL,
    element_id   TEXT,
    occurred_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    payload      JSONB       NOT NULL DEFAULT '{}',
    metadata     JSONB       NOT NULL DEFAULT '{}'
);

CREATE INDEX idx_process_events_instance      ON process_events (instance_id, occurred_at);
CREATE INDEX idx_process_events_event_type    ON process_events (event_type);
CREATE INDEX idx_process_events_element       ON process_events (instance_id, element_id);
