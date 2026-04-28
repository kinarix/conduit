import { apiFetch } from './client'

export interface Task {
  id: string
  instance_id: string
  execution_id: string
  element_id: string
  name: string | null
  task_type: string
  assignee: string | null
  state: string
  created_at: string
  completed_at: string | null
}

export interface TaskListResponse {
  items: Task[]
}

export const fetchTasks = () =>
  apiFetch<TaskListResponse>('/api/v1/tasks').then(r => r.items)

/** Convert a flat JSON object {"key": value} to VariableInput[] */
export function toVariableInputs(
  obj: Record<string, unknown>,
): Array<{ name: string; value_type: string; value: unknown }> {
  return Object.entries(obj).map(([name, value]) => ({
    name,
    value_type: typeof value === 'number' ? 'number' : typeof value === 'boolean' ? 'boolean' : 'string',
    value,
  }))
}

export const completeTask = (
  id: string,
  variables?: Array<{ name: string; value_type: string; value: unknown }>,
) =>
  apiFetch<void>(`/api/v1/tasks/${id}/complete`, {
    method: 'POST',
    body: JSON.stringify({ variables: variables ?? [] }),
  })
