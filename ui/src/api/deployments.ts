import { apiFetch } from './client'

export interface ProcessDefinition {
  id: string
  org_id: string
  key: string
  name: string
  version: number
  bpmn_xml: string
  deployed_at: string
}

export const fetchDeployments = (org_id: string) =>
  apiFetch<ProcessDefinition[]>(`/api/v1/deployments?org_id=${org_id}`)

export const fetchDeployment = (id: string) =>
  apiFetch<ProcessDefinition>(`/api/v1/deployments/${id}`)

export const deployProcess = (body: {
  org_id: string
  key: string
  name: string
  bpmn_xml: string
}) => apiFetch<ProcessDefinition>('/api/v1/deployments', { method: 'POST', body: JSON.stringify(body) })
