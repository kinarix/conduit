import { apiFetch } from './client'

// ─── Org admin (settings + auth config) ──────────────────────────────────────

export interface AdminOrg {
  id: string
  name: string
  slug: string
  setup_completed: boolean
  created_at: string
  admin_email: string | null
  admin_name: string | null
  support_email: string | null
  description: string | null
}

export interface AuthConfig {
  provider: 'internal' | 'oidc'
  oidc_issuer: string | null
  oidc_client_id: string | null
  oidc_client_secret_set: boolean
  oidc_redirect_uri: string | null
}

export const fetchAdminOrg = (orgId: string) =>
  apiFetch<AdminOrg>(`/api/v1/orgs/${orgId}/admin/org`)

/**
 * PATCH for the admin org settings.
 *
 * Contact / description fields use `string | null | undefined`:
 * - `undefined` (or omitted): leave the column untouched server-side.
 * - `null` or `""`: clear to NULL.
 * - non-empty `string`: trim + set.
 */
export const patchAdminOrg = (
  orgId: string,
  body: {
    name?: string
    setup_completed?: boolean
    admin_email?: string | null
    admin_name?: string | null
    support_email?: string | null
    description?: string | null
  },
) =>
  apiFetch<AdminOrg>(`/api/v1/orgs/${orgId}/admin/org`, {
    method: 'PATCH',
    body: JSON.stringify(body),
  })

export const fetchAuthConfig = (orgId: string) =>
  apiFetch<AuthConfig>(`/api/v1/orgs/${orgId}/admin/auth-config`)

export const patchAuthConfig = (orgId: string, body: {
  provider: string
  oidc_issuer?: string | null
  oidc_client_id?: string | null
  oidc_client_secret?: string | null
  oidc_redirect_uri?: string | null
}) =>
  apiFetch<AuthConfig>(`/api/v1/orgs/${orgId}/admin/auth-config`, {
    method: 'PATCH',
    body: JSON.stringify(body),
  })

// ─── Notification (email) config ────────────────────────────────────────────

export interface NotificationConfig {
  provider: 'disabled' | 'sendgrid' | 'smtp'
  from_email: string | null
  from_name: string | null
  /** True if a SendGrid API key has been stored. Plaintext is never returned. */
  sendgrid_api_key_set: boolean
  smtp_host: string | null
  smtp_port: number | null
  smtp_username: string | null
  smtp_password_set: boolean
  smtp_use_tls: boolean
}

export const fetchNotificationConfig = (orgId: string) =>
  apiFetch<NotificationConfig>(`/api/v1/orgs/${orgId}/admin/notification-config`)

/**
 * Update the org's notification provider config. Omit `sendgrid_api_key`
 * / `smtp_password` to preserve previously stored secrets while editing
 * other fields; pass an empty string to clear is NOT supported by the
 * server (clearing a secret is done by switching the provider).
 */
export const patchNotificationConfig = (orgId: string, body: {
  provider: NotificationConfig['provider']
  from_email?: string | null
  from_name?: string | null
  sendgrid_api_key?: string
  smtp_host?: string | null
  smtp_port?: number | null
  smtp_username?: string | null
  smtp_password?: string
  smtp_use_tls?: boolean
}) =>
  apiFetch<NotificationConfig>(`/api/v1/orgs/${orgId}/admin/notification-config`, {
    method: 'PATCH',
    body: JSON.stringify(body),
  })

// ─── Org users (membership) ──────────────────────────────────────────────────

export interface OrgUser {
  id: string
  email: string
  auth_provider: string
  external_id: string | null
  password_hash: string | null
  name: string | null
  phone: string | null
  created_at: string
}

export interface CreateOrgUserBody {
  email: string
  auth_provider: 'internal' | 'external'
  password?: string
  external_id?: string
  /** Optional display name. */
  name?: string
  /** Optional contact phone (free text). */
  phone?: string
}

export const listOrgUsers = (orgId: string) =>
  apiFetch<OrgUser[]>(`/api/v1/orgs/${orgId}/users`)

export const createOrgUser = (orgId: string, body: CreateOrgUserBody) =>
  apiFetch<OrgUser>(`/api/v1/orgs/${orgId}/users`, {
    method: 'POST',
    body: JSON.stringify(body),
  })

export const removeOrgUser = (orgId: string, userId: string) =>
  apiFetch<void>(`/api/v1/orgs/${orgId}/users/${userId}`, { method: 'DELETE' })

/**
 * Admin reset of an org member's password. Caller needs `user.reset_password`
 * scoped to this org (or globally). Rejected for non-members, external-auth
 * users, and platform-admin targets when the caller is not a platform admin.
 */
export const resetOrgUserPassword = (orgId: string, userId: string, newPassword: string) =>
  apiFetch<void>(`/api/v1/orgs/${orgId}/users/${userId}/reset-password`, {
    method: 'POST',
    body: JSON.stringify({ new_password: newPassword }),
  })

/**
 * Global password reset — usable only by platform admins. Targets any user
 * regardless of org membership.
 */
export const resetGlobalUserPassword = (userId: string, newPassword: string) =>
  apiFetch<void>(`/api/v1/admin/users/${userId}/reset-password`, {
    method: 'POST',
    body: JSON.stringify({ new_password: newPassword }),
  })

// ─── Members ─────────────────────────────────────────────────────────────────

export interface OrgMember {
  user_id: string
  org_id: string
  invited_by: string | null
  joined_at: string
}

export const listOrgMembers = (orgId: string) =>
  apiFetch<OrgMember[]>(`/api/v1/orgs/${orgId}/members`)

export const addOrgMember = (orgId: string, userId: string) =>
  apiFetch<OrgMember>(`/api/v1/orgs/${orgId}/members`, {
    method: 'POST',
    body: JSON.stringify({ user_id: userId }),
  })

export const removeOrgMember = (orgId: string, userId: string) =>
  apiFetch<void>(`/api/v1/orgs/${orgId}/members/${userId}`, { method: 'DELETE' })

// ─── Roles ───────────────────────────────────────────────────────────────────

export interface AdminRole {
  id: string
  name: string
  org_id: string | null
  permissions: string[]
}

export const listBuiltinRoles = () =>
  apiFetch<AdminRole[]>('/api/v1/roles')

export const listOrgRoles = (orgId: string) =>
  apiFetch<AdminRole[]>(`/api/v1/orgs/${orgId}/roles`)

export const createOrgRole = (orgId: string, name: string, permissions: string[]) =>
  apiFetch<AdminRole>(`/api/v1/orgs/${orgId}/roles`, {
    method: 'POST',
    body: JSON.stringify({ name, permissions }),
  })

export const updateOrgRole = (orgId: string, roleId: string, name: string, permissions: string[]) =>
  apiFetch<AdminRole>(`/api/v1/orgs/${orgId}/roles/${roleId}`, {
    method: 'PATCH',
    body: JSON.stringify({ name, permissions }),
  })

export const removeOrgRole = (orgId: string, roleId: string) =>
  apiFetch<void>(`/api/v1/orgs/${orgId}/roles/${roleId}`, { method: 'DELETE' })

// ─── Role assignments ────────────────────────────────────────────────────────

export interface OrgRoleAssignment {
  id: string
  user_id: string
  role_id: string
  org_id: string
  granted_by: string | null
  granted_in_org_id: string | null
  granted_at: string
}

export interface GlobalRoleAssignment {
  id: string
  user_id: string
  role_id: string
  granted_by: string | null
  granted_at: string
}

export const listOrgRoleAssignments = (orgId: string) =>
  apiFetch<OrgRoleAssignment[]>(`/api/v1/orgs/${orgId}/role-assignments`)

export const grantOrgRole = (orgId: string, userId: string, roleId: string) =>
  apiFetch<OrgRoleAssignment>(`/api/v1/orgs/${orgId}/role-assignments`, {
    method: 'POST',
    body: JSON.stringify({ user_id: userId, role_id: roleId }),
  })

export const revokeOrgRole = (orgId: string, assignmentId: string) =>
  apiFetch<void>(`/api/v1/orgs/${orgId}/role-assignments/${assignmentId}`, { method: 'DELETE' })

export const listGlobalRoleAssignments = () =>
  apiFetch<GlobalRoleAssignment[]>('/api/v1/admin/global-role-assignments')

export const grantGlobalRole = (userId: string, roleId: string) =>
  apiFetch<GlobalRoleAssignment>('/api/v1/admin/global-role-assignments', {
    method: 'POST',
    body: JSON.stringify({ user_id: userId, role_id: roleId }),
  })

export const revokeGlobalRole = (assignmentId: string) =>
  apiFetch<void>(`/api/v1/admin/global-role-assignments/${assignmentId}`, { method: 'DELETE' })

// ─── Platform admins ────────────────────────────────────────────────────────

export interface PlatformAdmin {
  user_id: string
  email: string
  name: string | null
  auth_provider: string
  user_created_at: string
  assignment_id: string
  granted_at: string
  granted_by: string | null
}

export interface CreatePlatformAdminBody {
  email: string
  auth_provider: 'internal' | 'external'
  password?: string
  external_id?: string
  name?: string
  phone?: string
}

export const listPlatformAdmins = () =>
  apiFetch<PlatformAdmin[]>('/api/v1/admin/platform-admins')

export const createPlatformAdmin = (body: CreatePlatformAdminBody) =>
  apiFetch<{
    user_id: string
    email: string
    name: string | null
    auth_provider: string
    assignment_id: string
  }>('/api/v1/admin/platform-admins', {
    method: 'POST',
    body: JSON.stringify(body),
  })

export const patchPlatformAdmin = (
  userId: string,
  body: { email?: string; name?: string },
) =>
  apiFetch<{
    user_id: string
    email: string
    name: string | null
  }>(`/api/v1/admin/platform-admins/${userId}`, {
    method: 'PATCH',
    body: JSON.stringify(body),
  })

export const revokePlatformAdmin = (userId: string) =>
  apiFetch<void>(`/api/v1/admin/platform-admins/${userId}`, {
    method: 'DELETE',
  })

// ─── Process-group-scoped role assignments ──────────────────────────────────

export interface PgRoleAssignment {
  id: string
  user_id: string
  role_id: string
  process_group_id: string
  org_id: string
  granted_by: string | null
  granted_at: string
}

export const listPgRoleAssignments = (orgId: string, pgId: string) =>
  apiFetch<PgRoleAssignment[]>(
    `/api/v1/orgs/${orgId}/process-groups/${pgId}/role-assignments`,
  )

export const grantPgRole = (orgId: string, pgId: string, userId: string, roleId: string) =>
  apiFetch<PgRoleAssignment>(
    `/api/v1/orgs/${orgId}/process-groups/${pgId}/role-assignments`,
    { method: 'POST', body: JSON.stringify({ user_id: userId, role_id: roleId }) },
  )

export const revokePgRole = (orgId: string, pgId: string, assignmentId: string) =>
  apiFetch<void>(
    `/api/v1/orgs/${orgId}/process-groups/${pgId}/role-assignments/${assignmentId}`,
    { method: 'DELETE' },
  )
