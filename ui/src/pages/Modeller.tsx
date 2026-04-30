import { useRef, useState, useEffect } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import BpmnEditor, { BpmnEditorHandle } from '../components/bpmn/BpmnEditor'
import { defaultBpmnXml } from '../components/bpmn/defaultBpmn'
import { fetchDeployment, deployProcess, saveDraft, createDraft, promoteDraft } from '../api/deployments'
import { useOrg } from '../App'

function formatRelative(iso: string): string {
  const then = new Date(iso).getTime()
  if (Number.isNaN(then)) return ''
  const secs = Math.round((Date.now() - then) / 1000)
  if (secs < 5) return 'just now'
  if (secs < 60) return `${secs}s ago`
  const mins = Math.round(secs / 60)
  if (mins < 60) return `${mins}m ago`
  const hrs = Math.round(mins / 60)
  if (hrs < 24) return `${hrs}h ago`
  const days = Math.round(hrs / 24)
  if (days < 7) return `${days}d ago`
  return new Date(iso).toLocaleDateString()
}

export default function Modeller() {
  const { id, groupId } = useParams<{ id?: string; groupId?: string }>()
  const navigate = useNavigate()
  const { org } = useOrg()

  // Two distinct modes:
  //  1. New flow  (no `id`, requires `groupId`): show only the create modal,
  //     no canvas. On submit, POST a stub draft and redirect to the edit URL.
  //  2. Edit flow (`id` present): full canvas + save/deploy controls.
  if (!id) {
    if (!groupId || !org) {
      // Defensive: a "new" route without a process group context shouldn't happen.
      return (
        <div className="empty-state">
          <p>Select a process group before creating a process.</p>
          <button className="btn-ghost" onClick={() => navigate(-1)}>Back</button>
        </div>
      )
    }
    return <ModellerCreate groupId={groupId} orgId={org.id} />
  }

  return <ModellerEdit defId={id} />
}

// ─── Create flow: skip the modal, create a stub draft immediately ───────────

function ModellerCreate({ groupId, orgId }: { groupId: string; orgId: string }) {
  const navigate = useNavigate()
  const qc = useQueryClient()
  const [error, setError] = useState('')
  const triedRef = useRef(false)

  const createMut = useMutation({
    mutationFn: () => {
      // Generate a unique-ish key client-side. The user can rename + change
      // the key from inside the modeller.
      const stub = `process-${Math.random().toString(36).slice(2, 8)}`
      return createDraft({
        org_id: orgId,
        process_group_id: groupId,
        key: stub,
        name: 'Untitled process',
        bpmn_xml: defaultBpmnXml(stub, 'Untitled process'),
      })
    },
    onSuccess: (created) => {
      qc.invalidateQueries({ queryKey: ['deployments', orgId] })
      navigate(`/definitions/${created.id}/edit`, { replace: true })
    },
    onError: (e: Error) => setError(e.message),
  })

  // Fire once on mount.
  useEffect(() => {
    if (triedRef.current) return
    triedRef.current = true
    createMut.mutate()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  return (
    <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100%' }}>
      <div style={{ textAlign: 'center', color: 'var(--text-secondary)' }}>
        {error ? (
          <>
            <div className="error-banner" style={{ marginBottom: 12 }}>{error}</div>
            <button className="btn-ghost" onClick={() => navigate(-1)}>Back</button>
          </>
        ) : (
          <>
            <div className="spinner" style={{ margin: '0 auto 12px' }} />
            <div>Creating new process…</div>
          </>
        )}
      </div>
    </div>
  )
}

// ─── Edit flow: full canvas ──────────────────────────────────────────────────

function ModellerEdit({ defId }: { defId: string }) {
  const navigate = useNavigate()
  const { org } = useOrg()
  const qc = useQueryClient()
  const modRef = useRef<BpmnEditorHandle>(null)
  const [key, setKey] = useState('')
  const [name, setName] = useState('')
  const [error, setError] = useState('')
  const [draftId, setDraftId] = useState<string | null>(null)
  const [editingName, setEditingName] = useState(false)
  const [editingKey, setEditingKey] = useState(false)

  const { data: existing } = useQuery({
    queryKey: ['deployment', defId],
    queryFn: () => fetchDeployment(defId),
  })

  useEffect(() => {
    if (existing) {
      setKey(existing.process_key ?? '')
      setName(existing.name ?? '')
      if (existing.status === 'draft') {
        setDraftId(existing.id)
      }
    }
  }, [existing])

  useEffect(() => {
    const label = name || 'Process'
    document.title = key ? `${label} (${key}) · Conduit` : `${label} · Conduit`
    return () => { document.title = 'Conduit' }
  }, [name, key])

  const process_group_id = existing?.process_group_id ?? null

  const saveMut = useMutation({
    mutationFn: async () => {
      if (!process_group_id) throw new Error('Process is not assigned to a process group')
      const bpmn_xml = await modRef.current!.getXml()
      return saveDraft({ org_id: org!.id, process_group_id, key, name, bpmn_xml })
    },
    onSuccess: (result) => {
      qc.invalidateQueries({ queryKey: ['deployments'] })
      setDraftId(result.id)
    },
    onError: (e: Error) => setError(e.message),
  })

  const deployMut = useMutation({
    mutationFn: async () => {
      if (!process_group_id) throw new Error('Process is not assigned to a process group')
      const bpmn_xml = await modRef.current!.getXml()

      const existingDraftId = draftId ?? (existing?.status === 'draft' ? existing.id : null)
      if (existingDraftId) {
        await saveDraft({ org_id: org!.id, process_group_id, key, name, bpmn_xml })
        return promoteDraft(existingDraftId)
      }
      return deployProcess({ org_id: org!.id, process_group_id, key, name, bpmn_xml })
    },
    onSuccess: (data) => {
      qc.invalidateQueries({ queryKey: ['deployments'] })
      navigate(`/definitions/${data.id}`)
    },
    onError: (e: Error) => setError(e.message),
  })

  const handleSave = () => {
    setError('')
    saveMut.mutate()
  }

  const handleDeploy = () => {
    setError('')
    deployMut.mutate()
  }

  const isExistingDraft = existing?.status === 'draft' || !!draftId
  const isBusy = saveMut.isPending || deployMut.isPending

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100vh', padding: 'var(--space-5)', boxSizing: 'border-box' }}>
      <div style={{
        display: 'flex',
        justifyContent: 'space-between',
        alignItems: 'center',
        marginBottom: 16,
        flexShrink: 0,
      }}>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 2, minWidth: 0 }}>
          <div style={{ display: 'flex', alignItems: 'baseline', gap: 10, minWidth: 0 }}>
            {editingName ? (
              <input
                autoFocus
                type="text"
                value={name}
                placeholder="Untitled process"
                onChange={(e) => setName(e.target.value)}
                onBlur={() => { setEditingName(false); if (isExistingDraft) saveMut.mutate() }}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') { e.currentTarget.blur() }
                  else if (e.key === 'Escape') { setEditingName(false) }
                }}
                style={{
                  fontSize: 18,
                  fontWeight: 600,
                  border: '1px solid var(--color-border)',
                  borderRadius: 4,
                  padding: '2px 6px',
                  background: 'var(--color-surface)',
                  color: 'var(--color-text)',
                  minWidth: 200,
                }}
              />
            ) : (
              <h1
                onClick={() => setEditingName(true)}
                title="Click to rename"
                style={{
                  fontSize: 18,
                  fontWeight: 600,
                  margin: 0,
                  overflow: 'hidden',
                  textOverflow: 'ellipsis',
                  whiteSpace: 'nowrap',
                  cursor: 'text',
                  padding: '2px 6px',
                  borderRadius: 4,
                  border: '1px solid transparent',
                }}
              >
                {name || (isExistingDraft ? 'Untitled draft' : 'Untitled process')}
              </h1>
            )}
            {editingKey ? (
              <input
                autoFocus
                type="text"
                value={key}
                placeholder="process-key"
                onChange={(e) => setKey(e.target.value.replace(/\s+/g, '-').toLowerCase())}
                onBlur={() => { setEditingKey(false); if (isExistingDraft) saveMut.mutate() }}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') { e.currentTarget.blur() }
                  else if (e.key === 'Escape') { setEditingKey(false) }
                }}
                style={{
                  fontSize: 12,
                  fontFamily: 'monospace',
                  border: '1px solid var(--color-border)',
                  borderRadius: 3,
                  padding: '1px 5px',
                  background: 'var(--color-surface)',
                  color: 'var(--color-text)',
                  minWidth: 140,
                }}
              />
            ) : (
              <span
                onClick={() => setEditingKey(true)}
                title="Click to edit key"
                style={{
                  fontSize: 12,
                  color: 'var(--color-text-muted)',
                  fontFamily: 'monospace',
                  flexShrink: 0,
                  cursor: 'text',
                  padding: '1px 5px',
                  borderRadius: 3,
                  border: '1px solid transparent',
                }}
              >
                {key || '(no key)'}
              </span>
            )}
          </div>
          <div style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 11, color: 'var(--color-text-muted)' }}>
            <span style={{
              padding: '1px 6px',
              borderRadius: 3,
              fontWeight: 600,
              background: isExistingDraft ? 'var(--color-surface-2)' : 'rgba(34,197,94,0.12)',
              color: isExistingDraft ? 'var(--color-text-muted)' : '#16a34a',
              textTransform: 'uppercase',
              letterSpacing: '0.04em',
              fontSize: 10,
            }}>
              {isExistingDraft ? 'Draft' : 'Deployed'}
            </span>
            {existing?.version != null && <span>v{existing.version}</span>}
            {existing?.deployed_at && (
              <span title={new Date(existing.deployed_at).toLocaleString()}>
                Last saved {formatRelative(existing.deployed_at)}
              </span>
            )}
            {saveMut.isSuccess && !saveMut.isPending && (
              <span style={{ color: '#16a34a' }}>· Saved</span>
            )}
          </div>
        </div>
        <div style={{ display: 'flex', gap: 8 }}>
          <button className="btn-ghost" onClick={() => navigate(`/definitions/${defId}`)} disabled={isBusy}>
            Cancel
          </button>
          <button className="btn-ghost" onClick={handleSave} disabled={isBusy}>
            {saveMut.isPending ? 'Saving…' : 'Save'}
          </button>
          <button className="btn-primary" onClick={handleDeploy} disabled={isBusy}>
            {deployMut.isPending ? 'Deploying…' : 'Deploy'}
          </button>
        </div>
      </div>

      <div style={{
        flex: 1,
        border: '1px solid var(--color-border)',
        borderRadius: 'var(--radius)',
        overflow: 'hidden',
        background: 'var(--color-surface-2)',
      }}>
        <BpmnEditor ref={modRef} xml={existing?.bpmn_xml} onProcessNameChange={setName} />
      </div>

    </div>
  )
}
