-- Phase 2: Core DB Schema
-- All engine tables. State fields use TEXT + CHECK (not ENUM) to avoid migration pain.

-- auth_provider controls how the user authenticates:
--   'internal' — email + password_hash stored here
--   'external' — external_id holds the IdP subject claim; password_hash is NULL
CREATE TABLE users (
    id            UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    org_id        UUID        NOT NULL REFERENCES orgs (id) ON DELETE CASCADE,
    auth_provider TEXT        NOT NULL CHECK (auth_provider IN ('internal', 'external')),
    external_id   TEXT,
    email         TEXT        NOT NULL,
    password_hash TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uq_users_org_email UNIQUE (org_id, email)
);

CREATE INDEX idx_users_org_id ON users (org_id);

-- ----------------------------------------------------------------------------

CREATE TABLE process_definitions (
    id          UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    org_id      UUID        NOT NULL REFERENCES orgs (id) ON DELETE RESTRICT,
    owner_id    UUID        REFERENCES users (id) ON DELETE SET NULL,
    process_key TEXT        NOT NULL,
    version     INTEGER     NOT NULL,
    name        TEXT,
    bpmn_xml    TEXT        NOT NULL,
    labels      JSONB       NOT NULL DEFAULT '{}',
    deployed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uq_process_definitions_org_key_version UNIQUE (org_id, process_key, version)
);

CREATE INDEX idx_process_definitions_key    ON process_definitions (process_key);
CREATE INDEX idx_process_definitions_org_id ON process_definitions (org_id);
CREATE INDEX idx_process_definitions_labels ON process_definitions USING GIN (labels);

-- ----------------------------------------------------------------------------

CREATE TABLE process_instances (
    id            UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    org_id        UUID        NOT NULL REFERENCES orgs (id) ON DELETE RESTRICT,
    definition_id UUID        NOT NULL REFERENCES process_definitions (id) ON DELETE RESTRICT,
    state         TEXT        NOT NULL CHECK (state IN ('running', 'completed', 'error', 'cancelled')),
    labels        JSONB       NOT NULL DEFAULT '{}',
    started_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ended_at      TIMESTAMPTZ
);

CREATE INDEX idx_process_instances_definition_id ON process_instances (definition_id);
CREATE INDEX idx_process_instances_org_id        ON process_instances (org_id);
CREATE INDEX idx_process_instances_state         ON process_instances (state);
CREATE INDEX idx_process_instances_running       ON process_instances (started_at) WHERE state IN ('running', 'error');
CREATE INDEX idx_process_instances_labels        ON process_instances USING GIN (labels);

-- ----------------------------------------------------------------------------

-- parent_id supports subprocess and parallel gateway (Phase 9/12)
CREATE TABLE executions (
    id          UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    instance_id UUID        NOT NULL REFERENCES process_instances (id) ON DELETE CASCADE,
    parent_id   UUID        REFERENCES executions (id) ON DELETE CASCADE,
    element_id  TEXT        NOT NULL,
    state       TEXT        NOT NULL CHECK (state IN ('active', 'completed', 'cancelled')),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_executions_instance_id ON executions (instance_id);
CREATE INDEX idx_executions_parent_id   ON executions (parent_id);

-- ----------------------------------------------------------------------------

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

-- ----------------------------------------------------------------------------

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

CREATE INDEX idx_tasks_instance_id  ON tasks (instance_id);
CREATE INDEX idx_tasks_execution_id ON tasks (execution_id);
CREATE INDEX idx_tasks_state        ON tasks (state);
CREATE INDEX idx_tasks_pending      ON tasks (created_at) WHERE state = 'pending';

-- ----------------------------------------------------------------------------

CREATE TABLE jobs (
    id            UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    instance_id   UUID        NOT NULL REFERENCES process_instances (id) ON DELETE CASCADE,
    execution_id  UUID        NOT NULL REFERENCES executions (id) ON DELETE CASCADE,
    job_type      TEXT        NOT NULL CHECK (job_type IN ('timer', 'external_task')),
    topic         TEXT,
    due_date      TIMESTAMPTZ NOT NULL,
    locked_by     TEXT,
    locked_until  TIMESTAMPTZ,
    retries       INTEGER     NOT NULL DEFAULT 3,
    retry_count   INTEGER     NOT NULL DEFAULT 0,
    error_message TEXT,
    state         TEXT        NOT NULL CHECK (state IN ('pending', 'locked', 'completed', 'failed')),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_jobs_instance_id  ON jobs (instance_id);
CREATE INDEX idx_jobs_state        ON jobs (state);
-- Hot path for job executor: only unlocked pending jobs matter
CREATE INDEX idx_jobs_due_date_unlocked ON jobs (due_date) WHERE locked_until IS NULL;

-- ----------------------------------------------------------------------------

CREATE TABLE event_subscriptions (
    id              UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    instance_id     UUID        NOT NULL REFERENCES process_instances (id) ON DELETE CASCADE,
    execution_id    UUID        NOT NULL REFERENCES executions (id) ON DELETE CASCADE,
    event_type      TEXT        NOT NULL CHECK (event_type IN ('message', 'signal')),
    event_name      TEXT        NOT NULL,
    correlation_key TEXT,
    element_id      TEXT        NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_event_subscriptions_instance_id ON event_subscriptions (instance_id);
CREATE INDEX idx_event_subscriptions_event_name  ON event_subscriptions (event_name);
CREATE INDEX idx_event_subscriptions_message     ON event_subscriptions (event_name, correlation_key) WHERE event_type = 'message';

-- ----------------------------------------------------------------------------

INSERT INTO schema_info (version, description) VALUES (2, 'Core schema — orgs, users, process_definitions, process_instances, executions, variables, tasks, jobs, event_subscriptions');
