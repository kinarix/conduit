CREATE TABLE roles (
    id     UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    org_id UUID REFERENCES orgs(id) ON DELETE CASCADE,  -- NULL = global built-in role
    name   TEXT NOT NULL,
    UNIQUE (org_id, name)
);
CREATE UNIQUE INDEX idx_roles_global_name ON roles (name) WHERE org_id IS NULL;

CREATE TABLE role_permissions (
    role_id    UUID NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    permission TEXT NOT NULL,
    PRIMARY KEY (role_id, permission),
    CHECK (permission IN (
        'process.deploy', 'process.disable',
        'instance.start', 'instance.cancel', 'instance.read',
        'task.complete',
        'decision.deploy',
        'secret.manage',
        'user.manage',
        'role.manage',
        'worker.manage',
        'org.manage'
    ))
);

CREATE TABLE user_roles (
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role_id    UUID NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    granted_by UUID REFERENCES users(id) ON DELETE SET NULL,
    granted_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, role_id)
);
CREATE INDEX idx_user_roles_user_id ON user_roles (user_id);

-- Seed the four global built-in roles.
INSERT INTO roles (org_id, name) VALUES
    (NULL, 'Admin'),
    (NULL, 'Deployer'),
    (NULL, 'Operator'),
    (NULL, 'Reader');

-- Admin: all permissions
INSERT INTO role_permissions (role_id, permission)
SELECT r.id, p.permission
FROM roles r
CROSS JOIN (VALUES
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
WHERE r.name = 'Admin' AND r.org_id IS NULL;

-- Deployer: deploy processes/decisions, start instances, read
INSERT INTO role_permissions (role_id, permission)
SELECT r.id, p.permission
FROM roles r
CROSS JOIN (VALUES
    ('process.deploy'), ('process.disable'),
    ('instance.start'), ('instance.read'),
    ('decision.deploy')
) AS p(permission)
WHERE r.name = 'Deployer' AND r.org_id IS NULL;

-- Operator: start instances, complete tasks, read
INSERT INTO role_permissions (role_id, permission)
SELECT r.id, p.permission
FROM roles r
CROSS JOIN (VALUES
    ('instance.start'), ('instance.cancel'), ('instance.read'),
    ('task.complete')
) AS p(permission)
WHERE r.name = 'Operator' AND r.org_id IS NULL;

-- Reader: read only
INSERT INTO role_permissions (role_id, permission)
SELECT r.id, 'instance.read'
FROM roles r
WHERE r.name = 'Reader' AND r.org_id IS NULL;
