-- Extends the permission set with process.model and seeds 6 new built-in roles.
-- process.model separates draft authoring from production promotion:
--   - Modeller role: create/edit BPMN & DMN drafts, cannot promote to production
--   - process.deploy is still required to promote a draft version

-- 1. Extend the CHECK constraint to include process.model
ALTER TABLE role_permissions DROP CONSTRAINT role_permissions_permission_check;
ALTER TABLE role_permissions ADD CONSTRAINT role_permissions_permission_check
    CHECK (permission IN (
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

-- 2. Grant process.model to Admin retroactively
INSERT INTO role_permissions (role_id, permission)
SELECT r.id, 'process.model' FROM roles r WHERE r.name = 'Admin' AND r.org_id IS NULL;

-- 3. Seed the 6 new global built-in roles
INSERT INTO roles (org_id, name) VALUES
    (NULL, 'Developer'),
    (NULL, 'Modeller'),
    (NULL, 'Process Manager'),
    (NULL, 'Worker'),
    (NULL, 'Infrastructure Admin'),
    (NULL, 'User Admin');

-- Developer: full dev cycle (model, deploy, run, debug)
INSERT INTO role_permissions (role_id, permission)
SELECT r.id, p.permission FROM roles r
CROSS JOIN (VALUES
    ('process.model'), ('process.deploy'), ('process.disable'),
    ('instance.start'), ('instance.cancel'), ('instance.read'),
    ('task.complete'), ('decision.deploy')
) AS p(permission)
WHERE r.name = 'Developer' AND r.org_id IS NULL;

-- Modeller: design only — create/edit drafts, observe results
INSERT INTO role_permissions (role_id, permission)
SELECT r.id, p.permission FROM roles r
CROSS JOIN (VALUES ('process.model'), ('instance.read')) AS p(permission)
WHERE r.name = 'Modeller' AND r.org_id IS NULL;

-- Process Manager: full lifecycle without modelling
INSERT INTO role_permissions (role_id, permission)
SELECT r.id, p.permission FROM roles r
CROSS JOIN (VALUES
    ('process.deploy'), ('process.disable'),
    ('instance.start'), ('instance.cancel'), ('instance.read'),
    ('task.complete'), ('decision.deploy')
) AS p(permission)
WHERE r.name = 'Process Manager' AND r.org_id IS NULL;

-- Worker: task completion only (automated service accounts)
INSERT INTO role_permissions (role_id, permission)
SELECT r.id, 'task.complete' FROM roles r WHERE r.name = 'Worker' AND r.org_id IS NULL;

-- Infrastructure Admin: secrets + worker registrations
INSERT INTO role_permissions (role_id, permission)
SELECT r.id, p.permission FROM roles r
CROSS JOIN (VALUES ('secret.manage'), ('worker.manage')) AS p(permission)
WHERE r.name = 'Infrastructure Admin' AND r.org_id IS NULL;

-- User Admin: org membership management without full org.manage
INSERT INTO role_permissions (role_id, permission)
SELECT r.id, p.permission FROM roles r
CROSS JOIN (VALUES ('user.manage'), ('role.manage')) AS p(permission)
WHERE r.name = 'User Admin' AND r.org_id IS NULL;
