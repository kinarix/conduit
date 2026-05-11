-- Phase 23.1 — RBAC redesign: explicit org membership.
--
-- A user "belongs to" an org iff there is an org_members row. Membership is
-- the precondition for org-scoped role assignment: every row in
-- org_role_assignments(user_id, org_id) must have a matching row here
-- (enforced by the FK on org_role_assignments in migration 030 plus
-- application-side checks in db/role_assignments.rs).
--
-- A row in this table without any role assignments is meaningful: it
-- represents a user who has been invited / added to an org but does not yet
-- have any permissions inside it. Useful for the "invited, awaiting role"
-- onboarding state.

CREATE TABLE org_members (
    user_id     UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    org_id      UUID        NOT NULL REFERENCES orgs(id)  ON DELETE CASCADE,
    invited_by  UUID                 REFERENCES users(id) ON DELETE SET NULL,
    joined_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, org_id)
);

CREATE INDEX org_members_org_idx  ON org_members (org_id);
CREATE INDEX org_members_user_idx ON org_members (user_id);
