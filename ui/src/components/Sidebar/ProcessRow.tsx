import { useState } from 'react'
import { useNavigate, useLocation } from 'react-router-dom'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { type LogicalProcess, type ProcessDefinition, renameProcess } from '../../api/deployments'
import { useOrg, type Org } from '../../App'
import { ProcessIcon, TrashIcon, DownloadIcon, PencilIcon, ConflictIcon } from './SidebarIcons'
import InlineNameInput from './InlineNameInput'
import styles from './Sidebar.module.css'

interface Props {
  proc: LogicalProcess
  org: Org
  onConfirmDelete: (proc: LogicalProcess) => void
}

const DRAG_MIME = 'application/x-conduit-process-def-id'

export default function ProcessRow({ proc, org, onConfirmDelete }: Props) {
  const navigate = useNavigate()
  const location = useLocation()
  const { setOrg } = useOrg()
  const qc = useQueryClient()
  const [editing, setEditing] = useState(false)
  const [editInitial, setEditInitial] = useState(proc.displayName)
  const [editValue, setEditValue] = useState(proc.displayName)
  const [apiError, setApiError] = useState<string | undefined>()

  const editingThis = proc.versions.some(
    v =>
      location.pathname === `/definitions/${v.id}/edit` ||
      location.pathname === `/definitions/${v.id}`,
  )
  const dashboardPath = `/groups/${proc.groupId}/processes/${encodeURIComponent(proc.key)}`
  const onDashboard = location.pathname === dashboardPath
  const active = editingThis || onDashboard

  const validateName = (value: string): string | undefined => {
    const trimmed = value.trim()
    if (!trimmed) return 'Name cannot be empty'
    const cached = qc.getQueryData<ProcessDefinition[]>(['deployments', org.id]) ?? []
    const conflict = cached.some(
      d => d.process_group_id === proc.groupId && d.name === trimmed && d.process_key !== proc.key,
    )
    if (conflict) return 'Name already in use in this group'
    return undefined
  }

  // Local validation takes precedence; apiError is a fallback for server-side conflicts
  const validationMsg = validateName(editValue) ?? apiError

  const renameMut = useMutation({
    mutationFn: (name: string) =>
      renameProcess(org.id, {
        process_group_id: proc.groupId,
        process_key: proc.key,
        name,
      }),
    onMutate: async (name) => {
      await qc.cancelQueries({ queryKey: ['deployments', org.id] })
      const prev = qc.getQueryData<ProcessDefinition[]>(['deployments', org.id])
      if (prev) {
        qc.setQueryData<ProcessDefinition[]>(
          ['deployments', org.id],
          prev.map(d => d.process_key === proc.key ? { ...d, name } : d),
        )
      }
      return { prev }
    },
    onError: (err, name, ctx) => {
      if (ctx?.prev) qc.setQueryData(['deployments', org.id], ctx.prev)
      setApiError(err instanceof Error ? err.message.replace(/^\[[^\]]+\]\s*/, '') : 'Rename failed')
      setEditInitial(name)
      setEditValue(name)
      setEditing(true)
    },
    onSuccess: () => {
      proc.versions.forEach(v => qc.invalidateQueries({ queryKey: ['deployment', v.id] }))
      qc.invalidateQueries({ queryKey: ['deployments', org.id] })
    },
  })

  const handleExport = (e: React.MouseEvent) => {
    e.stopPropagation()
    const xml = proc.latest.bpmn_xml
    const blob = new Blob([xml], { type: 'application/xml' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = `${proc.key}.bpmn`
    a.click()
    URL.revokeObjectURL(url)
  }

  const handleClick = () => {
    setOrg(org)
    navigate(`/groups/${proc.groupId}/processes/${encodeURIComponent(proc.key)}`)
  }

  const openEdit = (e: React.MouseEvent) => {
    e.stopPropagation()
    setApiError(undefined)
    setEditInitial(proc.displayName)
    setEditValue(proc.displayName)
    setEditing(true)
  }

  const closeEdit = () => {
    setEditing(false)
    setApiError(undefined)
  }

  return (
    <div
      className={`${styles.row} ${styles.indent2} ${active ? styles.selected : ''}`}
      draggable={!editing}
      onDragStart={e => {
        e.dataTransfer.effectAllowed = 'move'
        e.dataTransfer.setData(DRAG_MIME, proc.latest.id)
        e.dataTransfer.setData('text/plain', proc.latest.id)
      }}
      onClick={() => { if (!editing) handleClick() }}
      title={proc.displayName}
    >
      <span className={styles.toggle} />
      <span className={styles.icon}>
        <ProcessIcon size={11} />
      </span>
      {editing ? (
        <InlineNameInput
          initial={editInitial}
          onSubmit={name => { setEditing(false); setApiError(undefined); renameMut.mutate(name) }}
          onCancel={closeEdit}
          onValueChange={v => { setEditValue(v); setApiError(undefined) }}
          isInvalid={!!validationMsg}
        />
      ) : (
        <span className={styles.label}>{proc.displayName}</span>
      )}
      {proc.hasDraft && !editing && <span className={styles.draftDot} title="Has draft" />}
      {proc.versions.length > 1 && !editing && (
        <span className={styles.versionBadge} title={`${proc.versions.length} versions`}>
          ×{proc.versions.length}
        </span>
      )}
      <span
        className={styles.actions}
        style={(editing && validationMsg) ? { visibility: 'visible' } : undefined}
      >
        {editing && validationMsg ? (
          <span className={`${styles.actionBtn} ${styles.conflict}`} title={validationMsg}>
            <ConflictIcon size={13} />
          </span>
        ) : !editing ? (
          <>
            <button
              type="button"
              className={styles.actionBtn}
              title="Rename"
              onClick={openEdit}
            >
              <PencilIcon size={13} />
            </button>
            <button
              type="button"
              className={styles.actionBtn}
              title="Export BPMN"
              onClick={handleExport}
            >
              <DownloadIcon size={13} />
            </button>
            <button
              type="button"
              className={`${styles.actionBtn} ${styles.delete}`}
              title="Delete process"
              onClick={e => {
                e.stopPropagation()
                onConfirmDelete(proc)
              }}
            >
              <TrashIcon size={13} />
            </button>
          </>
        ) : null}
      </span>
    </div>
  )
}

export const PROCESS_DRAG_MIME = DRAG_MIME
