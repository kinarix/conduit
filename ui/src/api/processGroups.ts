import { apiFetch } from './client'

export interface ProcessGroup {
  id: string
  org_id: string
  name: string
  created_at: string
}

export function fetchProcessGroups(orgId: string): Promise<ProcessGroup[]> {
  return apiFetch(`/api/v1/orgs/${orgId}/process-groups`)
}

export function createProcessGroup(orgId: string, name: string): Promise<ProcessGroup> {
  return apiFetch(`/api/v1/orgs/${orgId}/process-groups`, {
    method: 'POST',
    body: JSON.stringify({ name }),
  })
}

export function renameProcessGroup(orgId: string, id: string, name: string): Promise<ProcessGroup> {
  return apiFetch(`/api/v1/orgs/${orgId}/process-groups/${id}`, {
    method: 'PUT',
    body: JSON.stringify({ name }),
  })
}

export function deleteProcessGroup(orgId: string, id: string): Promise<void> {
  return apiFetch(`/api/v1/orgs/${orgId}/process-groups/${id}`, { method: 'DELETE' })
}

export function assignProcessGroup(orgId: string, definitionId: string, processGroupId: string): Promise<void> {
  return apiFetch(`/api/v1/orgs/${orgId}/deployments/${definitionId}/process-group`, {
    method: 'PUT',
    body: JSON.stringify({ process_group_id: processGroupId }),
  })
}
