import { useMemo } from 'react'
import { useQuery } from '@tanstack/react-query'
import {
  fetchInstance,
  fetchInstanceHistory,
  fetchInstanceJobs,
} from '../../api/instances'
import { fetchDeployment } from '../../api/deployments'
import { fetchTasks } from '../../api/tasks'
import { fetchInstanceEvents } from '../../api/events'
import BpmnViewer from '../../components/bpmn/BpmnViewer'
import { useOrg } from '../../App'
import type { RuntimeStatus } from '../../components/bpmn/bpmnTypes'

interface Props {
  instanceId: string
}

export default function InstanceDiagram({ instanceId }: Props) {
  const { org } = useOrg()
  const orgId = org?.id

  const instanceQ = useQuery({
    queryKey: ['instance', orgId, instanceId],
    queryFn: () => fetchInstance(orgId!, instanceId),
    enabled: !!orgId,
    refetchInterval: 5_000,
  })

  const defQ = useQuery({
    queryKey: ['deployment', orgId, instanceQ.data?.definition_id],
    queryFn: () => fetchDeployment(orgId!, instanceQ.data!.definition_id),
    enabled: !!instanceQ.data?.definition_id && !!orgId,
  })

  const historyQ = useQuery({
    queryKey: ['instance-history', orgId, instanceId],
    queryFn: () => fetchInstanceHistory(orgId!, instanceId),
    enabled: !!orgId,
    refetchInterval: 5_000,
  })

  const tasksQ = useQuery({
    queryKey: ['tasks', orgId],
    queryFn: () => fetchTasks(orgId!),
    enabled: !!orgId,
    refetchInterval: 5_000,
  })

  const jobsQ = useQuery({
    queryKey: ['instance-jobs', orgId, instanceId],
    queryFn: () => fetchInstanceJobs(orgId!, instanceId),
    enabled: !!orgId,
    refetchInterval: 5_000,
  })

  const eventsQ = useQuery({
    queryKey: ['instance-events', orgId, instanceId],
    queryFn: () => fetchInstanceEvents(orgId!, instanceId),
    enabled: !!orgId,
    refetchInterval: 5_000,
  })

  const elementStates = useMemo(() => {
    const m = new Map<string, RuntimeStatus>()

    // Index open executions (left_at=null) by execution_id for O(1) job lookup.
    // Jobs only carry execution_id, not element_id directly.
    const openExecToElement = new Map<string, string>()

    for (const row of historyQ.data ?? []) {
      if (!row.left_at) {
        openExecToElement.set(row.execution_id, row.element_id)
        m.set(row.element_id, 'active')
      } else {
        // Don't downgrade 'active' (a later open row for the same element takes priority)
        if (m.get(row.element_id) !== 'active') {
          m.set(row.element_id, 'completed')
        }
      }
    }

    // Active human tasks override with 'active'
    for (const t of tasksQ.data ?? []) {
      if (t.instance_id === instanceId && t.state === 'active') {
        m.set(t.element_id, 'active')
      }
    }

    // Jobs: failed → 'error'; pending/locked → 'active'
    for (const j of jobsQ.data ?? []) {
      const elementId = openExecToElement.get(j.execution_id)
      if (!elementId) continue
      if (j.state === 'failed' || j.error_message) {
        m.set(elementId, 'error')
      } else if (j.state === 'pending' || j.state === 'locked') {
        m.set(elementId, 'active')
      }
    }

    // Error events take highest priority (override any other status)
    for (const ev of eventsQ.data ?? []) {
      if (
        (ev.event_type === 'error_raised' || ev.event_type === 'error_caught') &&
        ev.element_id
      ) {
        m.set(ev.element_id, 'error')
      }
    }

    return m
  }, [historyQ.data, tasksQ.data, jobsQ.data, eventsQ.data, instanceId])

  if (instanceQ.isLoading || defQ.isLoading) {
    return (
      <div style={{ padding: 32, textAlign: 'center', color: 'var(--text-tertiary)' }}>
        Loading diagram…
      </div>
    )
  }

  const bpmnXml = defQ.data?.bpmn_xml
  if (!bpmnXml) {
    return (
      <div style={{ padding: 32, textAlign: 'center', color: 'var(--text-tertiary)' }}>
        No BPMN diagram available for this process.
      </div>
    )
  }

  return (
    <div
      style={{
        height: 540,
        border: '1px solid var(--border-primary)',
        borderRadius: 'var(--radius-md)',
        overflow: 'hidden',
        background: 'var(--bg-tertiary)',
      }}
    >
      <BpmnViewer xml={bpmnXml} elementStates={elementStates} />
    </div>
  )
}
