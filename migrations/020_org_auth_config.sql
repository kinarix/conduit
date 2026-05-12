-- Per-org authentication configuration.
-- Initially only 'internal' (email + password) is fully implemented;
-- 'oidc' fields are stored here and will be wired to an OIDC flow later.
CREATE TABLE org_auth_config (
    org_id                  UUID        PRIMARY KEY REFERENCES orgs(id) ON DELETE CASCADE,
    provider                TEXT        NOT NULL DEFAULT 'internal'
                                        CHECK (provider IN ('internal', 'oidc')),
    oidc_issuer             TEXT,
    oidc_client_id          TEXT,
    oidc_client_secret_enc  BYTEA,      -- ChaCha20-Poly1305, same key as secrets table
    oidc_redirect_uri       TEXT,
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
