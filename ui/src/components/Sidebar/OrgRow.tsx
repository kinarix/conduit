import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import {
  fetchProcessGroups,
  createProcessGroup,
  type ProcessGroup,
} from '../../api/processGroups'
import { fetchDeployments } from '../../api/deployments'
import type { LogicalProcess } from '../../api/deployments'
import { ChevronIcon, OrgIcon, PlusIcon, TrashIcon, GroupIcon } from './SidebarIcons'
import GroupRow from './GroupRow'
import { useExpansion } from './useExpansion'
import { useOrg, type Org } from '../../App'
import styles from './Sidebar.module.css'

interface Props {
  org: Org
  expanded: boolean
  onToggle: () => void
  onConfirmDeleteOrg: (org: Org) => void
  onConfirmDeleteGroup: (group: ProcessGroup) => void
  onConfirmDeleteProcess: (proc: LogicalProcess) => void
}

export default function OrgRow({
  org,
  expanded,
  onToggle,
  onConfirmDeleteOrg,
  onConfirmDeleteGroup,
  onConfirmDeleteProcess,
}: Props) {
  const qc = useQueryClient()
  const { setOrg } = useOrg()
  const groupsExp = useExpansion(`sidebar.groups.${org.id}`)

  const { data: groups = [] } = useQuery({
    queryKey: ['process-groups', org.id],
    queryFn: () => fetchProcessGroups(org.id),
    enabled: expanded,
  })

  const { data: defs = [] } = useQuery({
    queryKey: ['deployments', org.id],
    queryFn: () => fetchDeployments(org.id),
    enabled: expanded,
  })

  const [newGroupId, setNewGroupId] = useState<string | null>(null)

  const createGroupMut = useMutation({
    mutationFn: () => createProcessGroup(org.id, 'New Process Group'),
    onSuccess: created => {
      qc.invalidateQueries({ queryKey: ['process-groups', org.id] })
      groupsExp.expand(created.id)
      setNewGroupId(created.id)
    },
  })

  return (
    <div>
      <div
        className={`${styles.row} ${styles.orgRow}`}
        onClick={() => {
          onToggle()
          setOrg(org)
        }}
        title={org.name}
      >
        <span className={styles.toggle} onClick={e => { e.stopPropagation(); onToggle() }}>
          <ChevronIcon size={10} expanded={expanded} />
        </span>
        <span className={styles.icon} style={{ color: 'var(--accent)' }}>
          <OrgIcon size={13} />
        </span>
        <span className={styles.label}>{org.name}</span>
        <span className={styles.actions}>
          <button
            type="button"
            className={`${styles.actionBtn} ${styles.add}`}
            title="New process group"
            onClick={e => {
              e.stopPropagation()
              setOrg(org)
              if (!expanded) onToggle()
              createGroupMut.mutate()
            }}
          >
            <PlusIcon size={13} />
          </button>
          <button
            type="button"
            className={`${styles.actionBtn} ${styles.delete}`}
            title="Delete org"
            onClick={e => { e.stopPropagation(); onConfirmDeleteOrg(org) }}
          >
            <TrashIcon size={13} />
          </button>
        </span>
      </div>

      {expanded && (
        groups.length === 0 ? (
          <div className={styles.empty} style={{ paddingLeft: 'calc(var(--space-3) + 16px)' }}>
            <span style={{ display: 'inline-flex', alignItems: 'center', gap: 6 }}>
              <GroupIcon size={11} />
              No process groups yet
            </span>
          </div>
        ) : (
          groups.map(group => (
            <GroupRow
              key={group.id}
              group={group}
              org={org}
              defs={defs.filter(d => d.process_group_id === group.id)}
              expanded={groupsExp.expanded.has(group.id)}
              onToggle={() => groupsExp.toggle(group.id)}
              onConfirmDeleteGroup={onConfirmDeleteGroup}
              onConfirmDeleteProcess={onConfirmDeleteProcess}
              autoEdit={group.id === newGroupId}
              onEditDone={() => setNewGroupId(null)}
            />
          ))
        )
      )}
    </div>
  )
}
