import { useState } from 'react'
import { useNavigate, useLocation } from 'react-router-dom'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { renameProcessGroup, assignProcessGroup, type ProcessGroup } from '../../api/processGroups'
import { groupByProcessKey, type ProcessDefinition, type LogicalProcess } from '../../api/deployments'
import { useOrg, type Org } from '../../App'
import { ChevronIcon, GroupIcon, PencilIcon, PlusIcon, TrashIcon } from './SidebarIcons'
import ProcessRow, { PROCESS_DRAG_MIME } from './ProcessRow'
import InlineNameInput from './InlineNameInput'
import styles from './Sidebar.module.css'

interface Props {
  group: ProcessGroup
  org: Org
  defs: ProcessDefinition[]
  expanded: boolean
  onToggle: () => void
  onConfirmDeleteGroup: (group: ProcessGroup) => void
  onConfirmDeleteProcess: (proc: LogicalProcess) => void
}

export default function GroupRow({
  group,
  org,
  defs,
  expanded,
  onToggle,
  onConfirmDeleteGroup,
  onConfirmDeleteProcess,
}: Props) {
  const navigate = useNavigate()
  const location = useLocation()
  const qc = useQueryClient()
  const { setOrg } = useOrg()
  const [editing, setEditing] = useState(false)
  const [dropping, setDropping] = useState(false)
  const orgId = org.id

  const renameMut = useMutation({
    mutationFn: (name: string) => renameProcessGroup(group.id, name),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['process-groups', orgId] })
      setEditing(false)
    },
  })

  const assignMut = useMutation({
    mutationFn: (defId: string) => assignProcessGroup(defId, group.id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['deployments', orgId] }),
  })

  const groupActive = location.pathname.startsWith(`/groups/${group.id}`) ||
                      location.pathname.startsWith(`/process-groups/${group.id}`)

  const processes = groupByProcessKey(defs)

  return (
    <>
      <div
        className={`${styles.row} ${styles.indent1} ${groupActive ? styles.selected : ''} ${dropping ? styles.dropTarget : ''}`}
        onClick={() => {
          if (!editing) {
            setOrg(org)
            onToggle()
            navigate(`/process-groups/${group.id}`)
          }
        }}
        onDragOver={e => {
          if (e.dataTransfer.types.includes(PROCESS_DRAG_MIME)) {
            e.preventDefault()
            e.dataTransfer.dropEffect = 'move'
            setDropping(true)
          }
        }}
        onDragLeave={() => setDropping(false)}
        onDrop={e => {
          e.preventDefault()
          setDropping(false)
          const id =
            e.dataTransfer.getData(PROCESS_DRAG_MIME) ||
            e.dataTransfer.getData('text/plain')
          if (id) assignMut.mutate(id)
        }}
        title={group.name}
      >
        <span className={styles.toggle} onClick={e => { e.stopPropagation(); onToggle() }}>
          <ChevronIcon size={10} expanded={expanded} />
        </span>
        <span className={styles.icon}>
          <GroupIcon size={13} />
        </span>
        {editing ? (
          <InlineNameInput
            initial={group.name}
            onSubmit={name => renameMut.mutate(name)}
            onCancel={() => setEditing(false)}
          />
        ) : (
          <span className={styles.label}>{group.name}</span>
        )}
        <span className={styles.actions}>
          <button
            type="button"
            className={`${styles.actionBtn} ${styles.add}`}
            title="New process"
            onClick={e => {
              e.stopPropagation()
              navigate(`/process-groups/${group.id}/definitions/new`)
            }}
          >
            <PlusIcon size={11} />
          </button>
          <button
            type="button"
            className={styles.actionBtn}
            title="Rename"
            onClick={e => { e.stopPropagation(); setEditing(true) }}
          >
            <PencilIcon size={11} />
          </button>
          <button
            type="button"
            className={`${styles.actionBtn} ${styles.delete}`}
            title="Delete process group"
            onClick={e => { e.stopPropagation(); onConfirmDeleteGroup(group) }}
          >
            <TrashIcon size={11} />
          </button>
        </span>
      </div>

      {expanded && (
        processes.length === 0 ? (
          <div className={styles.empty} style={{ paddingLeft: 'calc(var(--space-3) + 32px)' }}>
            No processes yet
          </div>
        ) : (
          processes.map(proc => (
            <ProcessRow
              key={`${proc.groupId}::${proc.key}`}
              proc={proc}
              org={org}
              onConfirmDelete={onConfirmDeleteProcess}
            />
          ))
        )
      )}
    </>
  )
}
