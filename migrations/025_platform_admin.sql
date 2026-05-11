-- Splits the platform-admin and org-admin concerns.
--
-- The existing `Admin` role used to bundle multi-tenant operator powers
-- (create orgs, provision users) with org-level powers (manage processes,
-- instances, secrets, etc.). After this migration:
--
--   * `Admin`     — platform admin. Only holds the new `org.create`
--                   permission: create orgs, list all orgs, create users in
--                   any org. Cannot view processes, instances, tasks, or
--                   secrets inside an org.
--
--   * `Org Admin` — new built-in role holding the original 13 org-level
--                   permissions (process.model, instance.read, …, org.manage).
--                   Wears the "full org owner" hat.
--
-- A hidden `_platform` org (orgs.is_system = TRUE) hosts the platform admin.
-- It is excluded from regular org listings and cannot be deleted.

-- 1. Add `org.create` to the permission CHECK constraint.
ALTER TABLE role_permissions DROP CONSTRAINT role_permissions_permission_check;
ALTER TABLE role_permissions ADD CONSTRAINT role_permissions_permission_check
    CHECK (permission IN (
        'org.create',
        'process.model',
        'process.deploy', 'process.disable',
        'instance.start', 'instance.cancel', 'instance.read',
        'task.complete',
        'decision.deploy',
        'secret.manage',
        'user.manage',
        'role.manage',
        'worker.manage',
        'org.manage'
    ));

-- 2. Flag column for the hidden platform org.
ALTER TABLE orgs ADD COLUMN is_system BOOLEAN NOT NULL DEFAULT FALSE;

-- 3. Seed the `_platform` org. setup_completed is forced TRUE — there's no
--    wizard for the platform org itself; the instance-setup wizard creates
--    *other* orgs.
INSERT INTO orgs (name, slug, is_system, setup_completed)
VALUES ('Platform', '_platform', TRUE, TRUE)
ON CONFLICT (slug) DO NOTHING;

-- 4. Strip the existing `Admin` role of all its org-level permissions and
--    grant it `org.create` instead. The role keeps its UUID, so any users
--    already assigned to it stay assigned — they just have different perms.
DELETE FROM role_permissions
WHERE role_id IN (SELECT id FROM roles WHERE name = 'Admin' AND org_id IS NULL);

INSERT INTO role_permissions (role_id, permission)
SELECT r.id, 'org.create' FROM roles r WHERE r.name = 'Admin' AND r.org_id IS NULL;

-- 5. Add the new `Org Admin` role with the full set of 13 org-level perms.
INSERT INTO roles (org_id, name) VALUES (NULL, 'Org Admin');

INSERT INTO role_permissions (role_id, permission)
SELECT r.id, p.permission FROM roles r
CROSS JOIN (VALUES
    ('process.model'),
    ('process.deploy'), ('process.disable'),
    ('instance.start'), ('instance.cancel'), ('instance.read'),
    ('task.complete'),
    ('decision.deploy'),
    ('secret.manage'),
    ('user.manage'),
    ('role.manage'),
    ('worker.manage'),
    ('org.manage')
) AS p(permission)
WHERE r.name = 'Org Admin' AND r.org_id IS NULL;
