import { useMemo, useRef, useState } from 'react'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { startInstance } from '../../api/instances'
import type { ProcessDefinition } from '../../api/deployments'
import { fromXml } from '../../components/bpmn/bpmnXml'
import SidePanel from '../../components/forms/SidePanel'
import VariableForm, { type VariableFormHandle } from '../../components/forms/VariableForm'

interface Props {
  org: string
  version: ProcessDefinition
  onClose: () => void
  onStarted?: (instanceId: string) => void
}

export default function StartInstancePanel({ org, version, onClose, onStarted }: Props) {
  const qc = useQueryClient()
  const formRef = useRef<VariableFormHandle>(null)
  const [error, setError] = useState<string | null>(null)

  const schema = useMemo(() => {
    try {
      return fromXml(version.bpmn_xml).inputSchema
    } catch {
      return undefined
    }
  }, [version.bpmn_xml])

  const startMut = useMutation({
    mutationFn: (variables: Array<{ name: string; value_type: string; value: unknown }>) =>
      startInstance({ org_id: org, definition_id: version.id, variables }),
    onSuccess: created => {
      qc.invalidateQueries({ queryKey: ['instances', org] })
      onStarted?.(created.id)
      onClose()
    },
    onError: (e: Error) => setError(e.message),
  })

  const onSubmit = () => {
    setError(null)
    const vars = formRef.current?.collect()
    if (vars === null) return // validation errors already shown
    startMut.mutate(vars ?? [])
  }

  const canStart = version.status === 'deployed'

  return (
    <SidePanel
      title={`Start instance — v${version.version}`}
      subtitle={
        <>
          {version.name || version.process_key}{' '}
          {version.status === 'draft' && (
            <span style={{ color: 'var(--status-warn)', marginLeft: 8 }}>(draft — deploy first)</span>
          )}
        </>
      }
      onClose={onClose}
      footer={
        <>
          <button className="btn-ghost" onClick={onClose} disabled={startMut.isPending}>
            Cancel
          </button>
          <button
            className="btn-primary"
            disabled={!canStart || startMut.isPending}
            onClick={onSubmit}
          >
            {startMut.isPending ? 'Starting…' : 'Start'}
          </button>
        </>
      }
    >
      <VariableForm ref={formRef} schema={schema} />
      {error && <div className="error-banner" style={{ marginTop: 16 }}>{error}</div>}
    </SidePanel>
  )
}
