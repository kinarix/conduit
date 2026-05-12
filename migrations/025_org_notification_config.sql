-- Per-org notification (outbound email) configuration.
--
-- Stores the provider selection plus the credentials needed to send mail
-- on behalf of the org. The engine does not yet send mail itself — this
-- table is read by future BPMN email tasks / notification workers; for
-- now only the admin UI reads & writes it.
--
-- Secrets (`sendgrid_api_key`, `smtp_password`) are encrypted at rest
-- with the same ChaCha20-Poly1305 key used by `org_auth_config` and the
-- `secrets` table.
CREATE TABLE org_notification_config (
    org_id                UUID        PRIMARY KEY REFERENCES orgs(id) ON DELETE CASCADE,
    provider              TEXT        NOT NULL DEFAULT 'disabled'
                                      CHECK (provider IN ('disabled', 'sendgrid', 'smtp')),
    from_email            TEXT,
    from_name             TEXT,
    sendgrid_api_key_enc  BYTEA,
    smtp_host             TEXT,
    smtp_port             INTEGER,
    smtp_username         TEXT,
    smtp_password_enc     BYTEA,
    smtp_use_tls          BOOLEAN     NOT NULL DEFAULT TRUE,
    updated_at            TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
