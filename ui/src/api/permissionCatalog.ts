/**
 * Catalog of every permission the platform recognises, with a one-line
 * description for the help tooltip / sidebar.
 *
 * Mirrored from `migrations/021_roles.sql` and the `Permission` enum in
 * `src/auth/permission.rs`. There is no API endpoint that returns this
 * catalog — keep this list in sync with the Rust side manually.
 *
 * Consumers:
 *   - Org admin → Manage → Roles (custom-role builder)
 *   - Platform → Roles (cross-highlight catalog)
 */

export interface PermissionDetail {
  name: string
  description: string
}

export const PERMISSION_DETAILS: PermissionDetail[] = [
  { name: 'org.create',              description: 'Create new organisations (global-only).' },
  { name: 'org.read',                description: 'View organisation details.' },
  { name: 'org.update',              description: 'Rename the organisation, change settings.' },
  { name: 'org.delete',              description: 'Delete the organisation and all its data.' },
  { name: 'org_member.create',       description: 'Add users as members of the organisation.' },
  { name: 'org_member.read',         description: 'View the org member list.' },
  { name: 'org_member.delete',       description: 'Remove users from the organisation.' },
  { name: 'user.create',             description: 'Create new global user identities.' },
  { name: 'user.read',               description: 'View user identities and metadata.' },
  { name: 'user.update',             description: 'Update user details (email, password).' },
  { name: 'user.delete',             description: 'Delete user identities globally.' },
  { name: 'user.reset_password',     description: 'Reset another user’s password.' },
  { name: 'role.create',             description: 'Create custom role definitions.' },
  { name: 'role.read',               description: 'View role definitions.' },
  { name: 'role.update',             description: 'Update custom role definitions.' },
  { name: 'role.delete',             description: 'Delete custom role definitions.' },
  { name: 'role_assignment.create',  description: 'Grant roles to users.' },
  { name: 'role_assignment.read',    description: 'View role grants (audit).' },
  { name: 'role_assignment.delete',  description: 'Revoke role grants.' },
  { name: 'auth_config.read',        description: 'View authentication settings.' },
  { name: 'auth_config.update',      description: 'Configure auth providers (OIDC, etc.).' },
  { name: 'notification_config.read',   description: 'View notification provider settings.' },
  { name: 'notification_config.update', description: 'Configure notification provider (SendGrid, SMTP).' },
  { name: 'process.create',          description: 'Create process definitions.' },
  { name: 'process.read',            description: 'View process definitions.' },
  { name: 'process.update',          description: 'Edit process definitions and drafts.' },
  { name: 'process.delete',          description: 'Delete process definition versions.' },
  { name: 'process.deploy',          description: 'Promote drafts to production.' },
  { name: 'process.disable',         description: 'Disable/enable specific versions.' },
  { name: 'process_group.create',    description: 'Create process groups.' },
  { name: 'process_group.read',      description: 'View process groups.' },
  { name: 'process_group.update',    description: 'Rename process groups.' },
  { name: 'process_group.delete',    description: 'Delete process groups.' },
  { name: 'instance.read',           description: 'View instances and their state.' },
  { name: 'instance.start',          description: 'Start new process instances.' },
  { name: 'instance.cancel',         description: 'Cancel running instances.' },
  { name: 'instance.pause',          description: 'Pause running instances.' },
  { name: 'instance.resume',         description: 'Resume suspended instances.' },
  { name: 'instance.delete',         description: 'Delete instances and their history.' },
  { name: 'task.read',               description: 'View user tasks.' },
  { name: 'task.complete',           description: 'Complete user tasks.' },
  { name: 'task.update',             description: 'Claim or reassign tasks.' },
  { name: 'external_task.execute',   description: 'Workers: fetch, complete, fail, extend.' },
  { name: 'decision.create',         description: 'Create decision (DMN) definitions.' },
  { name: 'decision.read',           description: 'View decision definitions.' },
  { name: 'decision.update',         description: 'Edit decision tables.' },
  { name: 'decision.delete',         description: 'Delete decision definitions.' },
  { name: 'decision.deploy',         description: 'Deploy DMN versions.' },
  { name: 'secret.create',           description: 'Create encrypted secrets.' },
  { name: 'secret.read_metadata',    description: 'View secret names and timestamps.' },
  { name: 'secret.read_plaintext',   description: 'Read the actual secret value.' },
  { name: 'secret.update',           description: 'Update secret values.' },
  { name: 'secret.delete',           description: 'Delete secrets.' },
  { name: 'api_key.manage',          description: 'Admin: create/list/revoke API keys.' },
  { name: 'process_layout.read',     description: 'View modeller layout data.' },
  { name: 'process_layout.update',   description: 'Save modeller layout data.' },
  { name: 'message.correlate',       description: 'Send messages to running instances.' },
  { name: 'signal.broadcast',        description: 'Broadcast signals across instances.' },
]

export const PERMISSION_NAMES: string[] = PERMISSION_DETAILS.map(p => p.name)

export const PERMISSION_DESCRIPTION: Record<string, string> = Object.fromEntries(
  PERMISSION_DETAILS.map(p => [p.name, p.description]),
)
