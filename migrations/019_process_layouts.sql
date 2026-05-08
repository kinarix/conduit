CREATE TABLE process_layouts (
    org_id      UUID        NOT NULL REFERENCES orgs (id) ON DELETE CASCADE,
    process_key TEXT        NOT NULL,
    layout_data JSONB       NOT NULL DEFAULT '{}',
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (org_id, process_key)
);
