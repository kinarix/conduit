CREATE TABLE jobs (
    id                    UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    instance_id           UUID        NOT NULL REFERENCES process_instances (id) ON DELETE CASCADE,
    execution_id          UUID        NOT NULL REFERENCES executions (id) ON DELETE CASCADE,
    job_type              TEXT        NOT NULL CHECK (job_type IN ('timer', 'external_task', 'http_task', 'send_message')),
    topic                 TEXT,
    due_date              TIMESTAMPTZ NOT NULL,
    timer_expression      TEXT,
    repetitions_remaining INTEGER,
    locked_by             TEXT,
    locked_until          TIMESTAMPTZ,
    retries               INTEGER     NOT NULL DEFAULT 3,
    retry_count           INTEGER     NOT NULL DEFAULT 0,
    error_message         TEXT,
    state                 TEXT        NOT NULL CHECK (state IN ('pending', 'locked', 'completed', 'failed', 'cancelled')),
    created_at            TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_jobs_instance_id ON jobs (instance_id);
CREATE INDEX idx_jobs_state       ON jobs (state);
-- Hot path for job executor: only unlocked pending jobs matter
CREATE INDEX idx_jobs_due_date_unlocked ON jobs (due_date) WHERE locked_until IS NULL;
