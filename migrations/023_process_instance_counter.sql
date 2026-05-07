-- Per-(org, process_key) sequential counter for process instances.
-- The counter is a stable, human-friendly identifier shown in the UI in place
-- of the UUID. It is assigned atomically by a BEFORE INSERT trigger so all
-- existing INSERT call sites continue to work unchanged.

CREATE TABLE process_instance_counters (
    org_id        UUID   NOT NULL,
    process_key   TEXT   NOT NULL,
    next_counter  BIGINT NOT NULL DEFAULT 1,
    PRIMARY KEY (org_id, process_key)
);

ALTER TABLE process_instances ADD COLUMN counter BIGINT;

-- Backfill existing rows with row_number per (org_id, process_key) ordered by
-- started_at, then id (to break ties deterministically).
UPDATE process_instances pi
SET counter = sub.rn
FROM (
    SELECT pi2.id,
           ROW_NUMBER() OVER (
               PARTITION BY pd.org_id, pd.process_key
               ORDER BY pi2.started_at, pi2.id
           ) AS rn
    FROM process_instances pi2
    JOIN process_definitions pd ON pd.id = pi2.definition_id
) sub
WHERE pi.id = sub.id;

-- Seed the counters table with current max + 1 per (org_id, process_key).
INSERT INTO process_instance_counters (org_id, process_key, next_counter)
SELECT pd.org_id, pd.process_key, COALESCE(MAX(pi.counter), 0) + 1
FROM process_definitions pd
LEFT JOIN process_instances pi ON pi.definition_id = pd.id
GROUP BY pd.org_id, pd.process_key;

ALTER TABLE process_instances ALTER COLUMN counter SET NOT NULL;

CREATE INDEX idx_process_instances_counter
    ON process_instances (definition_id, counter);

-- Trigger: assign counter on insert if not already set. Uses INSERT ... ON
-- CONFLICT to atomically allocate the next value per (org_id, process_key).
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
