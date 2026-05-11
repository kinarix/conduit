-- Phase 23.1 — RBAC redesign: full permission catalog + reseed built-ins.
--
-- The CHECK constraint on role_permissions.permission was a tight list of 14
-- strings. The new catalog has ~50 entries spanning every domain object.
-- Naming rules:
--   - CRUD verbs (create / read / update / delete) for reference data.
--   - Domain verbs only when authorization meaningfully differs from CRUD
--     (state transitions, designed-vs-promoted artifacts, read-metadata vs
--     read-plaintext).
--
-- Single source of truth lives here. src/auth/permission.rs has a test that
-- parses this file at compile time to assert the Rust enum and the SQL CHECK
-- list stay in sync.

-- 1. Drop old CHECK constraints (021 and 024 each added their own).
ALTER TABLE role_permissions
    DROP CONSTRAINT IF EXISTS role_permissions_permission_check;
ALTER TABLE role_permissions
    DROP CONSTRAINT IF EXISTS role_permissions_permission_check1;
ALTER TABLE role_permissions
    DROP CONSTRAINT IF EXISTS role_permissions_permission_check2;

-- 2. New CHECK with the full catalog.
ALTER TABLE role_permissions
    ADD CONSTRAINT role_permissions_permission_check
    CHECK (permission IN (
        -- org
        'org.create', 'org.read', 'org.update', 'org.delete',
        -- org membership
        'org_member.create', 'org_member.read', 'org_member.delete',
        -- users (global identity)
        'user.create', 'user.read', 'user.update', 'user.delete',
        -- role definitions
        'role.create', 'role.read', 'role.update', 'role.delete',
        -- role assignments
        'role_assignment.create', 'role_assignment.read', 'role_assignment.delete',
        -- per-org auth provider config
        'auth_config.read', 'auth_config.update',
        -- process definitions (BPMN)
        'process.create', 'process.read', 'process.update', 'process.delete',
        'process.deploy', 'process.disable',
        -- process groups
        'process_group.create', 'process_group.read',
        'process_group.update', 'process_group.delete',
        -- process instances
        'instance.read', 'instance.start', 'instance.cancel',
        'instance.pause', 'instance.resume', 'instance.delete',
        -- user tasks
        'task.read', 'task.complete', 'task.update',
        -- external (worker) tasks
        'external_task.execute',
        -- decisions (DMN)
        'decision.create', 'decision.read', 'decision.update',
        'decision.delete', 'decision.deploy',
        -- secrets (split read into metadata vs plaintext for compliance)
        'secret.create', 'secret.read_metadata', 'secret.read_plaintext',
        'secret.update', 'secret.delete',
        -- api keys (admin-managed; self-management is implicit per user)
        'api_key.manage',
        -- process layout (BPMN diagram positions)
        'process_layout.read', 'process_layout.update',
        -- business events
        'message.correlate',
        'signal.broadcast'
    ));

-- 3. Drop legacy built-in role rows. Migration 027 already wiped custom roles.
DELETE FROM role_permissions
 WHERE role_id IN (SELECT id FROM roles WHERE org_id IS NULL);
DELETE FROM roles WHERE org_id IS NULL;

-- 4. Seed new built-in role templates. All have org_id IS NULL and are shared
--    across every org. They are NOT copied per-org; orgs that need bespoke
--    permission bundles create custom org-scoped roles.
INSERT INTO roles (org_id, name) VALUES
    (NULL, 'PlatformAdmin'),
    (NULL, 'OrgOwner'),
    (NULL, 'OrgAdmin'),
    (NULL, 'Developer'),
    (NULL, 'Operator'),
    (NULL, 'Modeller'),
    (NULL, 'Reader'),
    (NULL, 'Worker');

-- 5. Permission seeds per role.

-- PlatformAdmin: every permission in the catalog. Use a CROSS JOIN against
-- the CHECK list to keep this future-proof — when migration 032+ adds a
-- permission, PlatformAdmin gets it automatically only if we re-run a seed;
-- for now we enumerate explicitly so the role's power is auditable in SQL.
INSERT INTO role_permissions (role_id, permission)
SELECT r.id, p.permission
FROM roles r
CROSS JOIN (VALUES
    ('org.create'), ('org.read'), ('org.update'), ('org.delete'),
    ('org_member.create'), ('org_member.read'), ('org_member.delete'),
    ('user.create'), ('user.read'), ('user.update'), ('user.delete'),
    ('role.create'), ('role.read'), ('role.update'), ('role.delete'),
    ('role_assignment.create'), ('role_assignment.read'), ('role_assignment.delete'),
    ('auth_config.read'), ('auth_config.update'),
    ('process.create'), ('process.read'), ('process.update'), ('process.delete'),
    ('process.deploy'), ('process.disable'),
    ('process_group.create'), ('process_group.read'),
    ('process_group.update'), ('process_group.delete'),
    ('instance.read'), ('instance.start'), ('instance.cancel'),
    ('instance.pause'), ('instance.resume'), ('instance.delete'),
    ('task.read'), ('task.complete'), ('task.update'),
    ('external_task.execute'),
    ('decision.create'), ('decision.read'), ('decision.update'),
    ('decision.delete'), ('decision.deploy'),
    ('secret.create'), ('secret.read_metadata'), ('secret.read_plaintext'),
    ('secret.update'), ('secret.delete'),
    ('api_key.manage'),
    ('process_layout.read'), ('process_layout.update'),
    ('message.correlate'), ('signal.broadcast')
) AS p(permission)
WHERE r.name = 'PlatformAdmin' AND r.org_id IS NULL;

-- OrgOwner: everything inside an org except org.create.
INSERT INTO role_permissions (role_id, permission)
SELECT r.id, p.permission
FROM roles r
CROSS JOIN (VALUES
    ('org.read'), ('org.update'), ('org.delete'),
    ('org_member.create'), ('org_member.read'), ('org_member.delete'),
    ('user.read'),
    ('role.create'), ('role.read'), ('role.update'), ('role.delete'),
    ('role_assignment.create'), ('role_assignment.read'), ('role_assignment.delete'),
    ('auth_config.read'), ('auth_config.update'),
    ('process.create'), ('process.read'), ('process.update'), ('process.delete'),
    ('process.deploy'), ('process.disable'),
    ('process_group.create'), ('process_group.read'),
    ('process_group.update'), ('process_group.delete'),
    ('instance.read'), ('instance.start'), ('instance.cancel'),
    ('instance.pause'), ('instance.resume'), ('instance.delete'),
    ('task.read'), ('task.complete'), ('task.update'),
    ('external_task.execute'),
    ('decision.create'), ('decision.read'), ('decision.update'),
    ('decision.delete'), ('decision.deploy'),
    ('secret.create'), ('secret.read_metadata'), ('secret.read_plaintext'),
    ('secret.update'), ('secret.delete'),
    ('api_key.manage'),
    ('process_layout.read'), ('process_layout.update'),
    ('message.correlate'), ('signal.broadcast')
) AS p(permission)
WHERE r.name = 'OrgOwner' AND r.org_id IS NULL;

-- OrgAdmin: manage org users / roles / auth-config, but can't delete the org
-- itself and has no process/instance/secret powers.
INSERT INTO role_permissions (role_id, permission)
SELECT r.id, p.permission
FROM roles r
CROSS JOIN (VALUES
    ('org.read'), ('org.update'),
    ('org_member.create'), ('org_member.read'), ('org_member.delete'),
    ('user.read'),
    ('role.create'), ('role.read'), ('role.update'), ('role.delete'),
    ('role_assignment.create'), ('role_assignment.read'), ('role_assignment.delete'),
    ('auth_config.read'), ('auth_config.update'),
    ('api_key.manage')
) AS p(permission)
WHERE r.name = 'OrgAdmin' AND r.org_id IS NULL;

-- Developer: full process / decision / instance lifecycle.
INSERT INTO role_permissions (role_id, permission)
SELECT r.id, p.permission
FROM roles r
CROSS JOIN (VALUES
    ('org.read'),
    ('process.create'), ('process.read'), ('process.update'), ('process.delete'),
    ('process.deploy'), ('process.disable'),
    ('process_group.create'), ('process_group.read'),
    ('process_group.update'), ('process_group.delete'),
    ('instance.read'), ('instance.start'), ('instance.cancel'),
    ('instance.pause'), ('instance.resume'),
    ('task.read'), ('task.complete'), ('task.update'),
    ('decision.create'), ('decision.read'), ('decision.update'),
    ('decision.delete'), ('decision.deploy'),
    ('secret.read_metadata'),
    ('process_layout.read'), ('process_layout.update'),
    ('message.correlate'), ('signal.broadcast')
) AS p(permission)
WHERE r.name = 'Developer' AND r.org_id IS NULL;

-- Operator: run + monitor; no design or deploy.
INSERT INTO role_permissions (role_id, permission)
SELECT r.id, p.permission
FROM roles r
CROSS JOIN (VALUES
    ('org.read'),
    ('process.read'),
    ('process_group.read'),
    ('instance.read'), ('instance.start'), ('instance.cancel'),
    ('instance.pause'), ('instance.resume'),
    ('task.read'), ('task.complete'), ('task.update'),
    ('decision.read'),
    ('process_layout.read'),
    ('message.correlate'), ('signal.broadcast')
) AS p(permission)
WHERE r.name = 'Operator' AND r.org_id IS NULL;

-- Modeller: design BPMN/DMN drafts, cannot promote.
INSERT INTO role_permissions (role_id, permission)
SELECT r.id, p.permission
FROM roles r
CROSS JOIN (VALUES
    ('org.read'),
    ('process.create'), ('process.read'), ('process.update'),
    ('process_group.create'), ('process_group.read'),
    ('process_group.update'),
    ('decision.create'), ('decision.read'), ('decision.update'),
    ('process_layout.read'), ('process_layout.update'),
    ('instance.read')
) AS p(permission)
WHERE r.name = 'Modeller' AND r.org_id IS NULL;

-- Reader: every read* permission.
INSERT INTO role_permissions (role_id, permission)
SELECT r.id, p.permission
FROM roles r
CROSS JOIN (VALUES
    ('org.read'),
    ('org_member.read'),
    ('user.read'),
    ('role.read'),
    ('role_assignment.read'),
    ('process.read'),
    ('process_group.read'),
    ('instance.read'),
    ('task.read'),
    ('decision.read'),
    ('secret.read_metadata'),
    ('process_layout.read')
) AS p(permission)
WHERE r.name = 'Reader' AND r.org_id IS NULL;

-- Worker: service account for external task execution.
INSERT INTO role_permissions (role_id, permission)
SELECT r.id, p.permission
FROM roles r
CROSS JOIN (VALUES
    ('external_task.execute'),
    ('process.read'),
    ('decision.read')
) AS p(permission)
WHERE r.name = 'Worker' AND r.org_id IS NULL;
