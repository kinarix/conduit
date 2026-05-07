-- Allow disabling a specific deployed version of a process.
-- Disabled versions cannot start NEW instances (manual, message, signal, timer).
-- Existing instances continue to run on whichever version they were started on.
ALTER TABLE process_definitions
    ADD COLUMN disabled_at TIMESTAMPTZ NULL;
