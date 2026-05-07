ALTER TABLE decision_definitions
    ADD COLUMN process_group_id UUID REFERENCES process_groups(id) ON DELETE SET NULL;

CREATE INDEX idx_decision_definitions_group
    ON decision_definitions(org_id, process_group_id, decision_key, version DESC);
