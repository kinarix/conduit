-- Timer-triggered process instantiation (separate from job-based timers)
CREATE TABLE timer_start_triggers (
    id                    UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    definition_id         UUID        NOT NULL REFERENCES process_definitions (id) ON DELETE CASCADE,
    element_id            TEXT        NOT NULL,
    timer_expression      TEXT        NOT NULL,
    repetitions_remaining INTEGER,
    due_at                TIMESTAMPTZ NOT NULL,
    state                 TEXT        NOT NULL DEFAULT 'pending'
                                          CHECK (state IN ('pending', 'fired', 'cancelled')),
    locked_by             TEXT,
    locked_until          TIMESTAMPTZ,
    created_at            TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_timer_start_triggers_due ON timer_start_triggers (due_at)
    WHERE state = 'pending';
