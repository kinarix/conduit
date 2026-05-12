-- Scoped role assignments at process-group scope.
--
-- The tables in the previous migration grant roles either globally or
-- per-org. That covers "Alice is OrgAdmin of Acme" but not "Bob is a
-- Developer in the HR-Workflows group only". This table adds the third
-- scope level.
--
-- Org-level grants cascade into every pg; a pg-level grant applies only
-- inside that one pg. Permissions that are conceptually per-org
-- (auth_config, secret, user, role*) are rejected at INSERT time in
-- `db::role_assignments` — see `Permission::is_pg_scopable`. The CHECK
-- here can't enforce that without joining role_permissions, so the
-- application layer guards it.
--
-- `org_id` is denormalized for two reasons:
--   1. So the composite FK to org_members(user_id, org_id) gives us the
--      same membership-delete cascade as org_role_assignments.
--   2. So the Principal extractor can SELECT all of a user's pg-level
--      grants in the current org with a single indexed query, without
--      joining process_groups.
--
-- A trigger asserts org_id matches process_groups.org_id so the
-- denormalised value cannot drift.

CREATE TABLE process_group_role_assignments (
    id                 UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id            UUID        NOT NULL REFERENCES users(id)          ON DELETE CASCADE,
    role_id            UUID        NOT NULL REFERENCES roles(id)          ON DELETE CASCADE,
    process_group_id   UUID        NOT NULL REFERENCES process_groups(id) ON DELETE CASCADE,
    org_id             UUID        NOT NULL REFERENCES orgs(id)           ON DELETE CASCADE,
    granted_by         UUID                 REFERENCES users(id)          ON DELETE SET NULL,
    granted_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (user_id, role_id, process_group_id),
    FOREIGN KEY (user_id, org_id) REFERENCES org_members (user_id, org_id) ON DELETE CASCADE
);

CREATE INDEX pg_role_assignments_user_idx  ON process_group_role_assignments (user_id);
CREATE INDEX pg_role_assignments_pg_idx    ON process_group_role_assignments (process_group_id);
CREATE INDEX pg_role_assignments_user_org_idx
    ON process_group_role_assignments (user_id, org_id);

-- Trigger: org_id on the assignment must match the pg's org_id. Without
-- this a malicious INSERT could pin the membership FK to one org while
-- the pg row belongs to another.
CREATE OR REPLACE FUNCTION assert_pg_assignment_org_matches() RETURNS trigger AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM process_groups
         WHERE id = NEW.process_group_id
           AND org_id = NEW.org_id
    ) THEN
        RAISE EXCEPTION
          'process_group_id % does not belong to org_id %',
          NEW.process_group_id, NEW.org_id;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER pg_role_assignments_org_check
    BEFORE INSERT OR UPDATE ON process_group_role_assignments
    FOR EACH ROW EXECUTE FUNCTION assert_pg_assignment_org_matches();
