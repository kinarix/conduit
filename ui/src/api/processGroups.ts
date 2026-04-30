import { apiFetch } from './client'

export interface ProcessGroup {
  id: string
  org_id: string
  name: string
  created_at: string
}

export function fetchProcessGroups(orgId: string): Promise<ProcessGroup[]> {
  return apiFetch(`/api/v1/process-groups?org_id=${orgId}`)
}

export function createProcessGroup(orgId: string, name: string): Promise<ProcessGroup> {
  return apiFetch('/api/v1/process-groups', {
    method: 'POST',
    body: JSON.stringify({ org_id: orgId, name }),
  })
}

export function renameProcessGroup(id: string, name: string): Promise<ProcessGroup> {
  return apiFetch(`/api/v1/process-groups/${id}`, {
    method: 'PUT',
    body: JSON.stringify({ name }),
  })
}

export function deleteProcessGroup(id: string): Promise<void> {
  return apiFetch(`/api/v1/process-groups/${id}`, { method: 'DELETE' })
}

export function assignProcessGroup(definitionId: string, processGroupId: string): Promise<void> {
  return apiFetch(`/api/v1/deployments/${definitionId}/process-group`, {
    method: 'PUT',
    body: JSON.stringify({ process_group_id: processGroupId }),
  })
}
