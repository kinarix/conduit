-- execution_id NOT NULL: process-level vars scope to root execution
CREATE TABLE variables (
    id           UUID  PRIMARY KEY DEFAULT uuid_generate_v4(),
    instance_id  UUID  NOT NULL REFERENCES process_instances (id) ON DELETE CASCADE,
    execution_id UUID  NOT NULL REFERENCES executions (id) ON DELETE CASCADE,
    name         TEXT  NOT NULL,
    value_type   TEXT  NOT NULL CHECK (value_type IN ('string', 'integer', 'boolean', 'json')),
    value        JSONB NOT NULL,
    CONSTRAINT uq_variables_execution_name UNIQUE (execution_id, name)
);

CREATE INDEX idx_variables_instance_id  ON variables (instance_id);
CREATE INDEX idx_variables_execution_id ON variables (execution_id);
