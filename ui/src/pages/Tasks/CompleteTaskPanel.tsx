import { useRef, useState } from 'react'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { completeTask, type Task } from '../../api/tasks'
import SidePanel from '../../components/forms/SidePanel'
import VariableForm, { type VariableFormHandle } from '../../components/forms/VariableForm'

interface Props {
  task: Task
  onClose: () => void
}

export default function CompleteTaskPanel({ task, onClose }: Props) {
  const qc = useQueryClient()
  const formRef = useRef<VariableFormHandle>(null)
  const [error, setError] = useState<string | null>(null)

  // Tasks can carry an output schema embedded in the element. The element's
  // schema isn't currently surfaced through the task API, so the form falls
  // back to free-form JSON — we still get type coercion (number/boolean)
  // when the form parses the JSON.
  const schema: string | undefined = undefined

  const mut = useMutation({
    mutationFn: (variables: Array<{ name: string; value_type: string; value: unknown }>) =>
      completeTask(task.id, variables),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['tasks'] })
      qc.invalidateQueries({ queryKey: ['instance', task.instance_id] })
      qc.invalidateQueries({ queryKey: ['instance-events', task.instance_id] })
      onClose()
    },
    onError: (e: Error) => setError(e.message),
  })

  const onSubmit = () => {
    setError(null)
    const vars = formRef.current?.collect()
    if (vars === null) return
    mut.mutate(vars ?? [])
  }

  return (
    <SidePanel
      title="Complete task"
      subtitle={
        <>
          {task.name || task.element_id}
          {task.assignee && <span style={{ marginLeft: 8 }}>· @{task.assignee}</span>}
        </>
      }
      onClose={onClose}
      footer={
        <>
          <button className="btn-ghost" onClick={onClose} disabled={mut.isPending}>
            Cancel
          </button>
          <button className="btn-primary" disabled={mut.isPending} onClick={onSubmit}>
            {mut.isPending ? 'Completing…' : 'Complete'}
          </button>
        </>
      }
    >
      <VariableForm ref={formRef} schema={schema} />
      {error && <div className="error-banner" style={{ marginTop: 16 }}>{error}</div>}
    </SidePanel>
  )
}
