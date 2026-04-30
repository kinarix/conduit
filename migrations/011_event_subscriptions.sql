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
