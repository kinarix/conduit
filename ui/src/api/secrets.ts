import { apiFetch } from './client'

export interface SecretMetadata {
  id: string
  org_id: string
  name: string
  created_at: string
  updated_at: string
}

export const fetchSecrets = (orgId: string) =>
  apiFetch<SecretMetadata[]>(`/api/v1/orgs/${orgId}/secrets`)

export const createSecret = (
  orgId: string,
  body: { name: string; value: string },
) =>
  apiFetch<SecretMetadata>(`/api/v1/orgs/${orgId}/secrets`, {
    method: 'POST',
    body: JSON.stringify(body),
  })

export const deleteSecret = (orgId: string, name: string) =>
  apiFetch<void>(
    `/api/v1/orgs/${orgId}/secrets/${encodeURIComponent(name)}`,
    { method: 'DELETE' },
  )
