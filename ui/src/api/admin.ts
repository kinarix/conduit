import { apiFetch } from './client'

// ─── Org admin (settings + auth config) ──────────────────────────────────────

export interface AdminOrg {
  id: string
  name: string
  slug: string
  setup_completed: boolean
  created_at: string
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

export const patchAdminOrg = (orgId: string, body: { name?: string; setup_completed?: boolean }) =>
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

// ─── Org users (membership) ──────────────────────────────────────────────────

export interface OrgUser {
  id: string
  email: string
  auth_provider: string
  external_id: string | null
  password_hash: string | null
  created_at: string
}

export interface CreateOrgUserBody {
  email: string
  auth_provider: 'internal' | 'external'
  password?: string
  external_id?: string
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
