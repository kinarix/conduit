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
import type { RuntimeStatus } from '../../components/bpmn/bpmnTypes'

interface Props {
  instanceId: string
}

/**
 * Derives a runtime-status map for every BPMN element this instance has touched
 * (or is currently sitting on), based on:
 *
 *   - execution_history rows  → entered / left
 *   - active tasks            → element is currently waiting on the user
 *   - active jobs             → element is currently waiting on a worker
 *   - process_events errors   → mark as 'error'
 */
export default function InstanceDiagram({ instanceId }: Props) {
  const instanceQ = useQuery({
    queryKey: ['instance', instanceId],
    queryFn: () => fetchInstance(instanceId),
    refetchInterval: 5_000,
  })

  const defQ = useQuery({
    queryKey: ['deployment', instanceQ.data?.definition_id],
    queryFn: () => fetchDeployment(instanceQ.data!.definition_id),
    enabled: !!instanceQ.data?.definition_id,
  })

  const historyQ = useQuery({
    queryKey: ['instance-history', instanceId],
    queryFn: () => fetchInstanceHistory(instanceId),
    refetchInterval: 5_000,
  })

  const tasksQ = useQuery({
    queryKey: ['tasks'],
    queryFn: fetchTasks,
    refetchInterval: 5_000,
  })

  const jobsQ = useQuery({
    queryKey: ['instance-jobs', instanceId],
    queryFn: () => fetchInstanceJobs(instanceId),
    refetchInterval: 5_000,
  })

  const eventsQ = useQuery({
    queryKey: ['instance-events', instanceId],
    queryFn: () => fetchInstanceEvents(instanceId),
    refetchInterval: 5_000,
  })

  const elementStates = useMemo(() => {
    const m = new Map<string, RuntimeStatus>()

    // 1. From history: entered+left → completed; entered+open → active.
    for (const row of historyQ.data ?? []) {
      const prev = m.get(row.element_id)
      if (row.left_at) {
        // Already completed at least once. Don't downgrade an active row.
        if (prev !== 'active') m.set(row.element_id, 'completed')
      } else {
        m.set(row.element_id, 'active')
      }
    }

    // 2. Active tasks for this instance — explicit "active" override.
    for (const t of tasksQ.data ?? []) {
      if (t.instance_id === instanceId && t.state === 'active') {
        m.set(t.element_id, 'active')
      }
    }

    // 3. Locked / pending jobs → active. Failed jobs → error on element.
    for (const j of jobsQ.data ?? []) {
      // Jobs don't directly carry element_id in the API — but their execution_id
      // matches an execution_history row. Cross-reference via history.
      const histRow = (historyQ.data ?? []).find(h => h.execution_id === j.execution_id)
      if (!histRow) continue
      if (j.state === 'pending' || j.state === 'locked') {
        m.set(histRow.element_id, 'active')
      } else if (j.state === 'failed' || j.error_message) {
        m.set(histRow.element_id, 'error')
      }
    }

    // 4. Errors raised/caught from the event log.
    for (const ev of eventsQ.data ?? []) {
      if (ev.event_type === 'error_raised' || ev.event_type === 'error_caught') {
        if (ev.element_id) m.set(ev.element_id, 'error')
      }
    }

    return m
  }, [historyQ.data, tasksQ.data, jobsQ.data, eventsQ.data, instanceId])

  if (defQ.isLoading || instanceQ.isLoading) {
    return <div style={{ padding: 32, textAlign: 'center', color: 'var(--text-tertiary)' }}>Loading diagram…</div>
  }

  const def = defQ.data
  if (!def?.bpmn_xml) {
    return (
      <div style={{ padding: 32, textAlign: 'center', color: 'var(--text-tertiary)' }}>
        No BPMN XML available for this instance's definition.
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
      <BpmnViewer xml={def.bpmn_xml} elementStates={elementStates} />
    </div>
  )
}
