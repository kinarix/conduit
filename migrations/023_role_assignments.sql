-- Role assignments split by scope.
--
--   org_role_assignments    — role granted within a specific org (the most
--                             common case). org_id is part of the grant.
--   global_role_assignments — role granted across all orgs (platform-wide).
--                             No org_id; the grant applies wherever the
--                             user operates.
--
-- A user's effective permissions inside org X are:
--   (permissions from every global_role_assignments row)
--   ∪
--   (permissions from org_role_assignments rows where org_id = X)
--   ∪
--   (permissions from process_group_role_assignments rows inside org X —
--     see the next migration)
--
-- The composite FK to org_members(user_id, org_id) is the membership
-- precondition: removing a user from an org cascades their org-scoped
-- grants automatically.

CREATE TABLE org_role_assignments (
    id                  UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id             UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role_id             UUID        NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    org_id              UUID        NOT NULL REFERENCES orgs(id)  ON DELETE CASCADE,
    granted_by          UUID                 REFERENCES users(id) ON DELETE SET NULL,
    -- Org where the grant decision was made. Informational/audit only —
    -- the permission boundary is `org_id`, not this column. Useful for
    -- cross-org delegation audits.
    granted_in_org_id   UUID                 REFERENCES orgs(id)  ON DELETE SET NULL,
    granted_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (user_id, role_id, org_id),
    CONSTRAINT org_role_assignments_member_fk
        FOREIGN KEY (user_id, org_id) REFERENCES org_members (user_id, org_id) ON DELETE CASCADE
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
