import { useState } from 'react'
import { useNavigate, useLocation } from 'react-router-dom'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { type DecisionSummary, renameDecision } from '../../api/decisions'
import { TableNavIcon, TrashIcon, PencilIcon, ConflictIcon } from './SidebarIcons'
import InlineNameInput from './InlineNameInput'
import styles from './Sidebar.module.css'

interface Props {
  decision: DecisionSummary
  orgId: string
  editBase: string
  indentClass?: string
  onSelect?: () => void
  onConfirmDelete: () => void
}

export default function DecisionRow({ decision, orgId, editBase, indentClass = styles.indent2, onSelect, onConfirmDelete }: Props) {
  const navigate = useNavigate()
  const location = useLocation()
  const qc = useQueryClient()
  const [editing, setEditing] = useState(false)
  const [editInitial, setEditInitial] = useState(decision.name ?? decision.decision_key)
  const [editValue, setEditValue] = useState(decision.name ?? decision.decision_key)
  const [apiError, setApiError] = useState<string | undefined>()

  const editPath = `${editBase}/${decision.decision_key}/edit`
  const active = location.pathname === editPath

  const displayName = decision.name ?? decision.decision_key

  const cacheKey = decision.process_group_id
    ? ['decisions', orgId, decision.process_group_id]
    : ['decisions', orgId]

  const validateName = (value: string): string | undefined => {
    const trimmed = value.trim()
    if (!trimmed) return 'Name cannot be empty'
    const cached = qc.getQueryData<DecisionSummary[]>(cacheKey) ?? []
    const conflict = cached.some(
      d => d.name === trimmed && d.decision_key !== decision.decision_key,
    )
    if (conflict) return 'Name already in use in this scope'
    return undefined
  }

  const validationMsg = validateName(editValue) ?? apiError

  const renameMut = useMutation({
    mutationFn: (name: string) => renameDecision(orgId, decision.decision_key, name),
    onMutate: async (name) => {
      await qc.cancelQueries({ queryKey: cacheKey })
      const prev = qc.getQueryData<DecisionSummary[]>(cacheKey)
      if (prev) {
        qc.setQueryData<DecisionSummary[]>(
          cacheKey,
          prev.map(d => d.decision_key === decision.decision_key ? { ...d, name } : d),
        )
      }
      return { prev }
    },
    onError: (err, name, ctx) => {
      if (ctx?.prev) qc.setQueryData(cacheKey, ctx.prev)
      setApiError(err instanceof Error ? err.message.replace(/^\[[^\]]+\]\s*/, '') : 'Rename failed')
      setEditInitial(name)
      setEditValue(name)
      setEditing(true)
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: cacheKey })
    },
  })

  const openEdit = (e: React.MouseEvent) => {
    e.stopPropagation()
    setApiError(undefined)
    setEditInitial(displayName)
    setEditValue(displayName)
    setEditing(true)
  }

  const closeEdit = () => {
    setEditing(false)
    setApiError(undefined)
  }

  return (
    <div
      className={`${styles.row} ${indentClass} ${active ? styles.selected : ''}`}
      onClick={() => { if (!editing) { onSelect?.(); navigate(editPath) } }}
      title={displayName}
    >
      <span className={styles.toggle} />
      <span className={styles.icon}>
        <TableNavIcon size={11} />
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
        <span className={styles.label}>{displayName}</span>
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
              className={`${styles.actionBtn} ${styles.delete}`}
              title="Delete decision"
              onClick={e => { e.stopPropagation(); onConfirmDelete() }}
            >
              <TrashIcon size={11} />
            </button>
          </>
        ) : null}
      </span>
    </div>
  )
}
