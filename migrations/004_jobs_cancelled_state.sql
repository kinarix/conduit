-- Phase 8: Add 'cancelled' state to jobs table (boundary timer cancellation).
ALTER TABLE jobs DROP CONSTRAINT jobs_state_check;
ALTER TABLE jobs ADD CONSTRAINT jobs_state_check
    CHECK (state IN ('pending', 'locked', 'completed', 'failed', 'cancelled'));

INSERT INTO schema_info (version, description)
VALUES (4, 'Add cancelled state to jobs');
