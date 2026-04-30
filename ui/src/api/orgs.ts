import { apiFetch } from './client'

export interface Org {
  id: string
  name: string
  slug: string
  created_at: string
}

export const fetchOrgs = () => apiFetch<Org[]>('/api/v1/orgs')

export const createOrg = (body: { name: string; slug: string }) =>
  apiFetch<Org>('/api/v1/orgs', { method: 'POST', body: JSON.stringify(body) })

export const deleteOrg = (id: string) =>
  apiFetch<void>(`/api/v1/orgs/${id}`, { method: 'DELETE' })
