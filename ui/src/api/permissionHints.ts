/**
 * Built-in role → permission map, mirrored from `migrations/021_roles.sql`.
 * Used only to display "roles that typically grant this permission" in
 * error toasts when a 403 lands. Custom org-scoped roles may also hold
 * any of these permissions; this map is a hint, not a permissions
 * reference.
 */

const ROLE_PERMISSIONS: Record<string, readonly string[]> = {
  PlatformAdmin: [
    'org.create', 'org.read', 'org.update', 'org.delete',
    'org_member.create', 'org_member.read', 'org_member.delete',
    'user.create', 'user.read', 'user.update', 'user.delete', 'user.reset_password',
    'role.create', 'role.read', 'role.update', 'role.delete',
    'role_assignment.create', 'role_assignment.read', 'role_assignment.delete',
    'auth_config.read', 'auth_config.update',
    'process.create', 'process.read', 'process.update', 'process.delete',
    'process.deploy', 'process.disable',
    'process_group.create', 'process_group.read', 'process_group.update', 'process_group.delete',
    'instance.read', 'instance.start', 'instance.cancel',
    'instance.pause', 'instance.resume', 'instance.delete',
    'task.read', 'task.complete', 'task.update',
    'external_task.execute',
    'decision.create', 'decision.read', 'decision.update', 'decision.delete', 'decision.deploy',
    'secret.create', 'secret.read_metadata', 'secret.read_plaintext', 'secret.update', 'secret.delete',
    'api_key.manage',
    'process_layout.read', 'process_layout.update',
    'message.correlate', 'signal.broadcast',
  ],
  OrgOwner: [
    'org.read', 'org.update', 'org.delete',
    'org_member.create', 'org_member.read', 'org_member.delete',
    'user.read', 'user.reset_password',
    'role.create', 'role.read', 'role.update', 'role.delete',
    'role_assignment.create', 'role_assignment.read', 'role_assignment.delete',
    'auth_config.read', 'auth_config.update',
    'process.create', 'process.read', 'process.update', 'process.delete',
    'process.deploy', 'process.disable',
    'process_group.create', 'process_group.read', 'process_group.update', 'process_group.delete',
    'instance.read', 'instance.start', 'instance.cancel',
    'instance.pause', 'instance.resume', 'instance.delete',
    'task.read', 'task.complete', 'task.update',
    'external_task.execute',
    'decision.create', 'decision.read', 'decision.update', 'decision.delete', 'decision.deploy',
    'secret.create', 'secret.read_metadata', 'secret.read_plaintext', 'secret.update', 'secret.delete',
    'api_key.manage',
    'process_layout.read', 'process_layout.update',
    'message.correlate', 'signal.broadcast',
  ],
  OrgAdmin: [
    'org.read', 'org.update',
    'org_member.create', 'org_member.read', 'org_member.delete',
    'user.create', 'user.read', 'user.reset_password',
    'role.create', 'role.read', 'role.update', 'role.delete',
    'role_assignment.create', 'role_assignment.read', 'role_assignment.delete',
    'auth_config.read', 'auth_config.update',
    'api_key.manage',
  ],
  Developer: [
    'org.read',
    'process.create', 'process.read', 'process.update', 'process.delete',
    'process.deploy', 'process.disable',
    'process_group.create', 'process_group.read', 'process_group.update', 'process_group.delete',
    'instance.read', 'instance.start', 'instance.cancel',
    'instance.pause', 'instance.resume',
    'task.read', 'task.complete', 'task.update',
    'decision.create', 'decision.read', 'decision.update', 'decision.delete', 'decision.deploy',
    'secret.read_metadata',
    'process_layout.read', 'process_layout.update',
    'message.correlate', 'signal.broadcast',
  ],
  Operator: [
    'org.read',
    'process.read',
    'process_group.read',
    'instance.read', 'instance.start', 'instance.cancel',
    'instance.pause', 'instance.resume',
    'task.read', 'task.complete', 'task.update',
    'decision.read',
    'process_layout.read',
    'message.correlate', 'signal.broadcast',
  ],
  Modeller: [
    'org.read',
    'process.create', 'process.read', 'process.update',
    'process_group.create', 'process_group.read', 'process_group.update',
    'decision.create', 'decision.read', 'decision.update',
    'process_layout.read', 'process_layout.update',
    'instance.read',
  ],
  Reader: [
    'org.read', 'org_member.read', 'user.read', 'role.read', 'role_assignment.read',
    'process.read', 'process_group.read', 'instance.read', 'task.read', 'decision.read',
    'secret.read_metadata', 'process_layout.read',
  ],
  Worker: [
    'external_task.execute', 'process.read', 'decision.read',
  ],
}

/**
 * Built-in roles that hold `perm`, in display order. Excludes PlatformAdmin
 * since it's not a typical grant — anyone seeing this hint is acting inside
 * an org. Returns [] for unknown permissions.
 */
export function rolesWithPermission(perm: string): string[] {
  return Object.entries(ROLE_PERMISSIONS)
    .filter(([role, perms]) => role !== 'PlatformAdmin' && perms.includes(perm))
    .map(([role]) => role)
}
