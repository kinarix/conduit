import { useMemo, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { useQuery } from '@tanstack/react-query'
import { fetchInstances, ProcessInstance } from '../api/instances'
import { fetchDeployments, ProcessDefinition } from '../api/deployments'
import { useOrg } from '../App'

interface StateCounts {
  running: number
  completed: number
  errored: number
  total: number
}

const ERROR_STATES = new Set(['error', 'failed'])

function countStates(instances: ProcessInstance[]): StateCounts {
  const counts: StateCounts = { running: 0, completed: 0, errored: 0, total: instances.length }
  for (const inst of instances) {
    if (inst.state === 'running') counts.running++
    else if (inst.state === 'completed') counts.completed++
    else if (ERROR_STATES.has(inst.state)) counts.errored++
  }
  return counts
}

interface PillProps {
  count: number
  label: string
  className: string
  onClick: (e: React.MouseEvent) => void
}

function Pill({ count, label, className, onClick }: PillProps) {
  if (count === 0) return null
  return (
    <span
      className={`badge ${className}`}
      style={{ cursor: 'pointer', fontSize: 11 }}
      onClick={onClick}
      title={`Show ${label} instances`}
    >
      {count} {label}
    </span>
  )
}

interface GroupRowProps {
  def: ProcessDefinition
  instances: ProcessInstance[]
}

function GroupRow({ def, instances }: GroupRowProps) {
  const navigate = useNavigate()
  const [hovered, setHovered] = useState(false)
  const counts = useMemo(() => countStates(instances), [instances])

  const goToDef = (state?: string) => {
    const url = state ? `/definitions/${def.id}?state=${state}` : `/definitions/${def.id}`
    navigate(url)
  }

  return (
    <tr
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      onClick={() => goToDef()}
      style={{ cursor: 'pointer' }}
    >
      <td>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <span style={{ fontWeight: 500 }}>{def.name || def.process_key}</span>
          <span style={{ fontSize: 11, color: 'var(--color-text-muted)' }}>v{def.version}</span>
        </div>
      </td>
      <td style={{ width: 320 }}>
        <div
          style={{
            display: 'flex',
            gap: 6,
            justifyContent: 'flex-end',
            opacity: hovered ? 1 : 0,
            transition: 'opacity 120ms',
            pointerEvents: hovered ? 'auto' : 'none',
          }}
        >
          <Pill
            count={counts.running}
            label="running"
            className="badge-running"
            onClick={(e) => { e.stopPropagation(); goToDef('running') }}
          />
          <Pill
            count={counts.completed}
            label="completed"
            className="badge-completed"
            onClick={(e) => { e.stopPropagation(); goToDef('completed') }}
          />
          <Pill
            count={counts.errored}
            label="errored"
            className="badge-error"
            onClick={(e) => { e.stopPropagation(); goToDef('error') }}
          />
        </div>
      </td>
      <td style={{ width: 60, textAlign: 'right', color: 'var(--color-text-muted)', fontSize: 12 }}>
        {counts.total}
      </td>
    </tr>
  )
}

export default function InstancesList() {
  const { org } = useOrg()

  const { data: instances, isLoading: instancesLoading } = useQuery({
    queryKey: ['instances', org?.id],
    queryFn: () => fetchInstances(org!.id),
    enabled: !!org,
    refetchInterval: 5_000,
  })

  const { data: definitions, isLoading: defsLoading } = useQuery({
    queryKey: ['deployments', org?.id],
    queryFn: () => fetchDeployments(org!.id),
    enabled: !!org,
  })

  const groups = useMemo(() => {
    if (!definitions) return []
    const byDef = new Map<string, ProcessInstance[]>()
    for (const inst of instances ?? []) {
      const list = byDef.get(inst.definition_id) ?? []
      list.push(inst)
      byDef.set(inst.definition_id, list)
    }
    return definitions
      .map(def => ({ def, list: byDef.get(def.id) ?? [] }))
      .filter(g => g.list.length > 0)
      .sort((a, b) => (a.def.name || a.def.process_key).localeCompare(b.def.name || b.def.process_key))
  }, [definitions, instances])

  if (instancesLoading || defsLoading) {
    return <div className="empty-state"><div className="spinner" /></div>
  }

  return (
    <div>
      <h1 style={{ fontSize: 18, fontWeight: 600, marginBottom: 20 }}>Process Instances</h1>

      {groups.length === 0 ? (
        <div className="empty-state">
          <p>No instances yet. Start one from a process definition.</p>
        </div>
      ) : (
        <table>
          <thead>
            <tr>
              <th>Process Definition</th>
              <th />
              <th style={{ textAlign: 'right' }}>Instances</th>
            </tr>
          </thead>
          <tbody>
            {groups.map(g => (
              <GroupRow key={g.def.id} def={g.def} instances={g.list} />
            ))}
          </tbody>
        </table>
      )}
    </div>
  )
}
