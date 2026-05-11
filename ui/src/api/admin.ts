import { apiFetch } from './client'

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

export interface AdminUser {
  id: string
  email: string
  auth_provider: string
  created_at: string
  roles: string[]
}

export interface AdminRole {
  id: string
  name: string
  org_id: string | null
  permissions: string[]
}

export const fetchAdminOrg = () => apiFetch<AdminOrg>('/api/v1/admin/org')

export const patchAdminOrg = (body: { name?: string; setup_completed?: boolean }) =>
  apiFetch<AdminOrg>('/api/v1/admin/org', { method: 'PATCH', body: JSON.stringify(body) })

export const fetchAuthConfig = () => apiFetch<AuthConfig>('/api/v1/admin/auth-config')

export const patchAuthConfig = (body: {
  provider: string
  oidc_issuer?: string | null
  oidc_client_id?: string | null
  oidc_client_secret?: string | null
  oidc_redirect_uri?: string | null
}) => apiFetch<AuthConfig>('/api/v1/admin/auth-config', { method: 'PATCH', body: JSON.stringify(body) })

export const listAdminUsers = () => apiFetch<AdminUser[]>('/api/v1/admin/users')

export interface CreateAdminUserBody {
  email: string
  auth_provider: 'internal' | 'external'
  password?: string
  external_id?: string
  role_ids?: string[]
  /** Platform admins (`org.create`) may target a different org. */
  org_id?: string
}

export const createAdminUser = (body: CreateAdminUserBody) =>
  apiFetch<AdminUser>('/api/v1/admin/users', {
    method: 'POST',
    body: JSON.stringify(body),
  })

export const removeAdminUser = (id: string) =>
  apiFetch<void>(`/api/v1/admin/users/${id}`, { method: 'DELETE' })

export const setUserRoles = (userId: string, roleIds: string[]) =>
  apiFetch<void>(`/api/v1/admin/users/${userId}/roles`, {
    method: 'PUT',
    body: JSON.stringify({ role_ids: roleIds }),
  })

export const listAdminRoles = () => apiFetch<AdminRole[]>('/api/v1/admin/roles')

export const createAdminRole = (name: string, permissions: string[]) =>
  apiFetch<AdminRole>('/api/v1/admin/roles', {
    method: 'POST',
    body: JSON.stringify({ name, permissions }),
  })

export const updateAdminRole = (id: string, name: string, permissions: string[]) =>
  apiFetch<AdminRole>(`/api/v1/admin/roles/${id}`, {
    method: 'PATCH',
    body: JSON.stringify({ name, permissions }),
  })

export const removeAdminRole = (id: string) =>
  apiFetch<void>(`/api/v1/admin/roles/${id}`, { method: 'DELETE' })
