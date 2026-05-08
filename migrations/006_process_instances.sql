CREATE TABLE process_instances (
    id            UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    org_id        UUID        NOT NULL REFERENCES orgs (id) ON DELETE RESTRICT,
    definition_id UUID        NOT NULL REFERENCES process_definitions (id) ON DELETE RESTRICT,
    state         TEXT        NOT NULL CHECK (state IN ('running', 'suspended', 'completed', 'error', 'cancelled')),
    labels        JSONB       NOT NULL DEFAULT '{}',
    -- Per-(org, process_key) sequential, human-friendly identifier shown in
    -- the UI in place of the UUID. Assigned atomically by a BEFORE INSERT
    -- trigger so all INSERT call sites can omit the column.
    counter       BIGINT      NOT NULL,
    started_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ended_at      TIMESTAMPTZ
);

CREATE INDEX idx_process_instances_definition_id ON process_instances (definition_id);
CREATE INDEX idx_process_instances_org_id        ON process_instances (org_id);
CREATE INDEX idx_process_instances_state         ON process_instances (state);
CREATE INDEX idx_process_instances_running       ON process_instances (started_at) WHERE state IN ('running', 'error');
CREATE INDEX idx_process_instances_labels        ON process_instances USING GIN (labels);
CREATE INDEX idx_process_instances_counter       ON process_instances (definition_id, counter);

-- Counter table: next value to assign per (org_id, process_key).
CREATE TABLE process_instance_counters (
    org_id       UUID   NOT NULL,
    process_key  TEXT   NOT NULL,
    next_counter BIGINT NOT NULL DEFAULT 1,
    PRIMARY KEY (org_id, process_key)
);

-- Trigger: atomically allocate the next counter on insert if not already set.
CREATE OR REPLACE FUNCTION assign_process_instance_counter()
RETURNS TRIGGER AS $$
DECLARE
    v_org_id      UUID;
    v_process_key TEXT;
    v_assigned    BIGINT;
BEGIN
    IF NEW.counter IS NOT NULL THEN
        RETURN NEW;
    END IF;

    SELECT pd.org_id, pd.process_key
      INTO v_org_id, v_process_key
    FROM process_definitions pd
    WHERE pd.id = NEW.definition_id;

    INSERT INTO process_instance_counters (org_id, process_key, next_counter)
    VALUES (v_org_id, v_process_key, 2)
    ON CONFLICT (org_id, process_key) DO UPDATE
    SET next_counter = process_instance_counters.next_counter + 1
    RETURNING next_counter - 1 INTO v_assigned;

    NEW.counter := v_assigned;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER assign_process_instance_counter_trg
    BEFORE INSERT ON process_instances
    FOR EACH ROW
    EXECUTE FUNCTION assign_process_instance_counter();
