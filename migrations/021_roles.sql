-- Role catalog: roles + their permissions + built-in role seeds.
--
-- Roles are either GLOBAL (org_id IS NULL) — the eight built-ins below,
-- shared across every org — or org-scoped custom roles created via the API.
--
-- Permissions are a fixed catalog, enumerated in the CHECK constraint and
-- mirrored by the `Permission` enum in `src/auth/permission.rs`. A unit
-- test asserts the two stay in sync; update both when adding a permission.
--
-- Naming rules:
--   - CRUD verbs (create / read / update / delete) for reference data.
--   - Domain verbs only when authorization meaningfully differs from CRUD
--     (state transitions, designed-vs-promoted artifacts, read-metadata
--     vs read-plaintext).

CREATE TABLE roles (
    id     UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    org_id UUID REFERENCES orgs(id) ON DELETE CASCADE,  -- NULL = global built-in
    name   TEXT NOT NULL,
    UNIQUE (org_id, name)
);
CREATE UNIQUE INDEX idx_roles_global_name ON roles (name) WHERE org_id IS NULL;

CREATE TABLE role_permissions (
    role_id    UUID NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    permission TEXT NOT NULL,
    PRIMARY KEY (role_id, permission),
    CHECK (permission IN (
        -- org
        'org.create', 'org.read', 'org.update', 'org.delete',
        -- org membership
        'org_member.create', 'org_member.read', 'org_member.delete',
        -- users (global identity)
        'user.create', 'user.read', 'user.update', 'user.delete',
        'user.reset_password',
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
    ))
);

-- Seed the global built-in roles. Custom org-scoped roles are created via
-- the API; these eight are the shared baseline.
INSERT INTO roles (org_id, name) VALUES
    (NULL, 'PlatformAdmin'),
    (NULL, 'OrgOwner'),
    (NULL, 'OrgAdmin'),
    (NULL, 'Developer'),
    (NULL, 'Operator'),
    (NULL, 'Modeller'),
    (NULL, 'Reader'),
    (NULL, 'Worker');

-- PlatformAdmin: every permission in the catalog. Enumerated explicitly so
-- the role's power is auditable directly from this file.
INSERT INTO role_permissions (role_id, permission)
SELECT r.id, p.permission
FROM roles r
CROSS JOIN (VALUES
    ('org.create'), ('org.read'), ('org.update'), ('org.delete'),
    ('org_member.create'), ('org_member.read'), ('org_member.delete'),
    ('user.create'), ('user.read'), ('user.update'), ('user.delete'),
    ('user.reset_password'),
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
    ('user.reset_password'),
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

-- OrgAdmin: manage org users / roles / auth-config (including creating new
-- global user identities to invite into the org). No process / instance /
-- secret powers.
INSERT INTO role_permissions (role_id, permission)
SELECT r.id, p.permission
FROM roles r
CROSS JOIN (VALUES
    ('org.read'), ('org.update'),
    ('org_member.create'), ('org_member.read'), ('org_member.delete'),
    ('user.create'), ('user.read'),
    ('user.reset_password'),
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
