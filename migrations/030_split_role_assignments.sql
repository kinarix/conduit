-- Phase 23.1 — RBAC redesign: split role assignments by scope.
--
-- The old user_roles table (user_id, role_id) was scope-free: it could not
-- express "Alice is OrgOwner of Org A but only Reader of Org B". We replace
-- it with two purpose-built tables.
--
--   org_role_assignments    — role granted within a specific org (the most
--                             common case). org_id is part of the grant.
--   global_role_assignments — role granted across all orgs (platform-wide).
--                             No org_id; the grant applies wherever the user
--                             operates.
--
-- A user's effective permissions inside org X are:
--   (permissions from every global_role_assignments row)
--   ∪
--   (permissions from org_role_assignments rows where org_id = X)

DROP TABLE IF EXISTS user_roles;

CREATE TABLE org_role_assignments (
    id                  UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id             UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role_id             UUID        NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    org_id              UUID        NOT NULL REFERENCES orgs(id)  ON DELETE CASCADE,
    granted_by          UUID                 REFERENCES users(id) ON DELETE SET NULL,
    -- Org where the grant decision was made. Informational/audit only — the
    -- permission boundary is `org_id`, not this column. Useful for compliance
    -- ("which org's admin made this grant") and for cross-org delegation
    -- audits later.
    granted_in_org_id   UUID                 REFERENCES orgs(id)  ON DELETE SET NULL,
    granted_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (user_id, role_id, org_id)
);

CREATE INDEX org_role_assignments_user_idx ON org_role_assignments (user_id);
CREATE INDEX org_role_assignments_org_idx  ON org_role_assignments (org_id);
CREATE INDEX org_role_assignments_user_org_idx
    ON org_role_assignments (user_id, org_id);

CREATE TABLE global_role_assignments (
    id          UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id     UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role_id     UUID        NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    granted_by  UUID                 REFERENCES users(id) ON DELETE SET NULL,
    granted_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (user_id, role_id)
);

CREATE INDEX global_role_assignments_user_idx ON global_role_assignments (user_id);
