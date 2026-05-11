-- Phase 23.1 — RBAC redesign: close the membership cascade.
--
-- Migration 030 created `org_role_assignments` with FKs to `users` and `orgs`
-- but not to `org_members`. Removing a user from an org therefore left
-- orphaned `org_role_assignments` rows. The plan and the doc comment on
-- `db::org_members::delete` both require that removing membership cascades
-- the role grants. Add the composite FK now.
--
-- Safe to run on the empty schema from migration 027: there are no rows yet.

ALTER TABLE org_role_assignments
    ADD CONSTRAINT org_role_assignments_member_fk
    FOREIGN KEY (user_id, org_id)
    REFERENCES org_members (user_id, org_id)
    ON DELETE CASCADE;
