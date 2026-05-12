-- A user "belongs to" an org iff there is an org_members row. Membership is
-- the precondition for org-scoped role assignment: every row in
-- `org_role_assignments(user_id, org_id)` (and the pg-scoped table) has a
-- composite FK back here.
--
-- A row in this table without any role assignments is meaningful: it
-- represents a user who has been invited / added to an org but does not yet
-- have any permissions inside it — the "invited, awaiting role" state.
CREATE TABLE org_members (
    user_id     UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    org_id      UUID        NOT NULL REFERENCES orgs(id)  ON DELETE CASCADE,
    invited_by  UUID                 REFERENCES users(id) ON DELETE SET NULL,
    joined_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, org_id)
);

CREATE INDEX org_members_org_idx  ON org_members (org_id);
CREATE INDEX org_members_user_idx ON org_members (user_id);
