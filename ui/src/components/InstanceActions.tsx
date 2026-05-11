import { useState } from 'react'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import {
  pauseInstance,
  resumeInstance,
  cancelInstance,
  deleteInstance,
  type ProcessInstance,
} from '../api/instances'

type Variant = 'icons' | 'buttons'

interface Props {
  instance: ProcessInstance
  variant?: Variant
  onDeleted?: () => void
}

export function InstanceActions({ instance, variant = 'icons', onDeleted }: Props) {
  const qc = useQueryClient()
  const [confirmDelete, setConfirmDelete] = useState(false)
  const [confirmCancel, setConfirmCancel] = useState(false)

  const invalidate = () => {
    qc.invalidateQueries({ queryKey: ['instance', instance.id] })
    qc.invalidateQueries({ queryKey: ['instances'] })
  }

  const pauseMut = useMutation({ mutationFn: () => pauseInstance(instance.org_id, instance.id), onSuccess: invalidate })
  const resumeMut = useMutation({ mutationFn: () => resumeInstance(instance.org_id, instance.id), onSuccess: invalidate })
  const cancelMut = useMutation({
    mutationFn: () => cancelInstance(instance.org_id, instance.id),
    onSuccess: () => { invalidate(); setConfirmCancel(false) },
  })
  const deleteMut = useMutation({
    mutationFn: () => deleteInstance(instance.org_id, instance.id),
    onSuccess: () => { invalidate(); setConfirmDelete(false); onDeleted?.() },
  })

  const isRunning = instance.state === 'running'
  const isSuspended = instance.state === 'suspended'
  const isTerminal = instance.state === 'completed' || instance.state === 'cancelled'
  const canPause = isRunning
  const canResume = isSuspended
  const canCancel = !isTerminal

  const stop = (e: React.MouseEvent) => e.stopPropagation()

  if (variant === 'icons') {
    return (
      <span style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }} onClick={stop}>
        {canPause && (
          <IconBtn title="Pause" color="#f59e0b" disabled={pauseMut.isPending}
            onClick={() => pauseMut.mutate()}>⏸</IconBtn>
        )}
        {canResume && (
          <IconBtn title="Resume" color="#16a34a" disabled={resumeMut.isPending}
            onClick={() => resumeMut.mutate()}>▶</IconBtn>
        )}
        {canCancel && (
          <IconBtn title="Stop" color="#dc2626" disabled={cancelMut.isPending}
            onClick={() => setConfirmCancel(true)}>■</IconBtn>
        )}
        <IconBtn title="Delete" color="#94a3b8" disabled={deleteMut.isPending}
          onClick={() => setConfirmDelete(true)}>🗑</IconBtn>
        {(confirmCancel || confirmDelete) && (
          <ConfirmModal
            kind={confirmCancel ? 'cancel' : 'delete'}
            instance={instance}
            pending={confirmCancel ? cancelMut.isPending : deleteMut.isPending}
            error={confirmCancel ? cancelMut.error : deleteMut.error}
            onClose={() => { setConfirmCancel(false); setConfirmDelete(false) }}
            onConfirm={() => (confirmCancel ? cancelMut.mutate() : deleteMut.mutate())}
          />
        )}
      </span>
    )
  }

  return (
    <div style={{ display: 'flex', gap: 8, alignItems: 'center' }} onClick={stop}>
      {canPause && (
        <button className="btn-ghost" disabled={pauseMut.isPending} onClick={() => pauseMut.mutate()}>
          ⏸ Pause
        </button>
      )}
      {canResume && (
        <button className="btn-primary" disabled={resumeMut.isPending} onClick={() => resumeMut.mutate()}>
          ▶ Resume
        </button>
      )}
      {canCancel && (
        <button className="btn-ghost" disabled={cancelMut.isPending} onClick={() => setConfirmCancel(true)}>
          ■ Stop
        </button>
      )}
      <button className="btn-danger" disabled={deleteMut.isPending} onClick={() => setConfirmDelete(true)}>
        🗑 Delete
      </button>
      {(confirmCancel || confirmDelete) && (
        <ConfirmModal
          kind={confirmCancel ? 'cancel' : 'delete'}
          instance={instance}
          pending={confirmCancel ? cancelMut.isPending : deleteMut.isPending}
          error={confirmCancel ? cancelMut.error : deleteMut.error}
          onClose={() => { setConfirmCancel(false); setConfirmDelete(false) }}
          onConfirm={() => (confirmCancel ? cancelMut.mutate() : deleteMut.mutate())}
        />
      )}
    </div>
  )
}

function IconBtn({
  title, color, disabled, onClick, children,
}: {
  title: string
  color: string
  disabled?: boolean
  onClick: () => void
  children: React.ReactNode
}) {
  return (
    <span
      title={title}
      role="button"
      onClick={(e) => { e.stopPropagation(); if (!disabled) onClick() }}
      style={{
        display: 'inline-flex',
        alignItems: 'center',
        justifyContent: 'center',
        width: 18,
        height: 18,
        cursor: disabled ? 'wait' : 'pointer',
        color,
        fontSize: 12,
        lineHeight: 1,
        opacity: disabled ? 0.5 : 1,
        userSelect: 'none',
      }}
    >
      {children}
    </span>
  )
}

function ConfirmModal({
  kind, instance, pending, error, onClose, onConfirm,
}: {
  kind: 'cancel' | 'delete'
  instance: ProcessInstance
  pending: boolean
  error: unknown
  onClose: () => void
  onConfirm: () => void
}) {
  const isDelete = kind === 'delete'
  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h3>{isDelete ? 'Delete instance' : 'Stop instance'}</h3>
        <p style={{ fontSize: 13, color: 'var(--color-text-muted)' }}>
          {isDelete
            ? 'This permanently removes the instance and all its tasks, jobs, variables, and history. This cannot be undone.'
            : 'This will cancel the instance and tear down its open tasks and jobs. The row remains for audit.'}
        </p>
        <p style={{ fontSize: 12, fontFamily: 'monospace', color: 'var(--color-text-muted)' }}>
          {instance.id}
        </p>
        {error ? <div className="error-banner">{String(error)}</div> : null}
        <div className="modal-actions">
          <button className="btn-ghost" onClick={onClose} disabled={pending}>Cancel</button>
          <button
            className={isDelete ? 'btn-danger' : 'btn-primary'}
            onClick={onConfirm}
            disabled={pending}
          >
            {pending ? 'Working…' : isDelete ? 'Delete' : 'Stop'}
          </button>
        </div>
      </div>
    </div>
  )
}
