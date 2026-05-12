import { useRef, useState, useEffect, useCallback } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import BpmnEditor, { BpmnEditorHandle } from '../components/bpmn/BpmnEditor'
import { defaultBpmnXml } from '../components/bpmn/defaultBpmn'
import { fetchDeployment, deployProcess, saveDraft, createDraft, promoteDraft, fetchLayout, saveLayout, type LayoutData } from '../api/deployments'
import { structuralFingerprint } from '../components/bpmn/bpmnXml'
import { useOrg } from '../App'

function formatTime(iso: string): string {
  const d = new Date(iso)
  if (Number.isNaN(d.getTime())) return ''
  return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
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
      const stub = `process-${Math.random().toString(36).slice(2, 8)}`
      return createDraft(orgId, {
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
  const [baselineFingerprint, setBaselineFingerprint] = useState<string | null>(null)
  const [savePrompt, setSavePrompt] = useState<{ name: string; key: string } | null>(null)
  const [lastSavedAt, setLastSavedAt] = useState<Date | null>(null)

  const { data: existing } = useQuery({
    queryKey: ['deployment', org?.id, defId],
    queryFn: () => fetchDeployment(org!.id, defId),
    enabled: !!org,
  })

  const { data: savedLayout } = useQuery({
    queryKey: ['process_layout', existing?.org_id, existing?.process_key],
    queryFn: () => fetchLayout(existing!.org_id, existing!.process_key),
    enabled: !!existing?.org_id && !!existing?.process_key,
    staleTime: Infinity,
  })

  useEffect(() => {
    if (existing) {
      setKey(existing.process_key ?? '')
      setName(existing.name ?? '')
      if (existing.status === 'draft') {
        setDraftId(existing.id)
      }
      try {
        setBaselineFingerprint(structuralFingerprint(existing.bpmn_xml))
      } catch { /* ignore parse errors */ }
    }
  }, [existing])

  useEffect(() => {
    const label = name || 'Process'
    document.title = key ? `${label} (${key}) · Conduit` : `${label} · Conduit`
    return () => { document.title = 'Conduit' }
  }, [name, key])

  const process_group_id = existing?.process_group_id ?? null

  const saveMut = useMutation({
    mutationFn: async (overrides: { name: string; key: string } | void) => {
      if (!org) throw new Error('No organisation selected')
      if (!process_group_id) throw new Error('Process is not assigned to a process group')
      const bpmn_xml = await modRef.current!.getXml()
      return saveDraft(org.id, { process_group_id, key: overrides?.key ?? key, name: overrides?.name ?? name, bpmn_xml })
    },
    onSuccess: (result) => {
      qc.invalidateQueries({ queryKey: ['deployments'] })
      setLastSavedAt(new Date())
      setDraftId(result.id)
      try {
        setBaselineFingerprint(structuralFingerprint(result.bpmn_xml))
      } catch { /* ignore */ }
    },
    onError: (e: Error) => setError(e.message),
  })

  const deployMut = useMutation({
    mutationFn: async () => {
      if (!org) throw new Error('No organisation selected')
      if (!process_group_id) throw new Error('Process is not assigned to a process group')
      const bpmn_xml = await modRef.current!.getXml()

      const existingDraftId = draftId ?? (existing?.status === 'draft' ? existing.id : null)
      if (existingDraftId) {
        await saveDraft(org.id, { process_group_id, key, name, bpmn_xml })
        return promoteDraft(org.id, existingDraftId)
      }
      return deployProcess(org.id, { process_group_id, key, name, bpmn_xml })
    },
    onSuccess: (data) => {
      qc.invalidateQueries({ queryKey: ['deployments'] })
      navigate(`/definitions/${data.id}`)
    },
    onError: (e: Error) => setError(e.message),
  })

  const handleSave = async () => {
    setError('')
    if (modRef.current && baselineFingerprint !== null) {
      try {
        const xml = await modRef.current.getXml()
        if (structuralFingerprint(xml) === baselineFingerprint) {
          return // layout-only change — nothing to persist
        }
      } catch { /* fingerprint check failed, fall through to save */ }
    }
    if (name === 'Untitled process') {
      setSavePrompt({ name, key })
      return
    }
    saveMut.mutate()
  }

  const handleDeploy = () => {
    setError('')
    deployMut.mutate()
  }

  const handleLayoutChange = useCallback((layout: LayoutData) => {
    if (!existing?.org_id || !existing?.process_key) return
    const orgId = existing.org_id
    const processKey = existing.process_key
    saveLayout(orgId, processKey, layout)
      .then(() => qc.setQueryData(['process_layout', orgId, processKey], layout))
      .catch(() => {})
  }, [existing?.org_id, existing?.process_key, qc])

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
            {(lastSavedAt ?? existing?.deployed_at) && (
              <span title={(lastSavedAt ?? new Date(existing!.deployed_at)).toLocaleString()}>
                Last saved {formatTime((lastSavedAt ?? new Date(existing!.deployed_at)).toISOString())}
              </span>
            )}
            {saveMut.isSuccess && !saveMut.isPending && (
              <span style={{ color: '#16a34a' }}>· Saved</span>
            )}
          </div>
        </div>
        <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
          <div style={{ width: 1, height: 20, background: 'var(--color-border)', margin: '0 2px' }} />
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

      {error && (
        <div className="error-banner" style={{ marginBottom: 8, flexShrink: 0 }}>{error}</div>
      )}

      <div style={{
        flex: 1,
        border: '1px solid var(--color-border)',
        borderRadius: 'var(--radius)',
        overflow: 'hidden',
        background: 'var(--color-surface-2)',
      }}>
        <BpmnEditor ref={modRef} xml={existing?.bpmn_xml} initialLayout={savedLayout} onLayoutChange={handleLayoutChange} onProcessNameChange={setName} groupId={existing?.process_group_id} />
      </div>

      {savePrompt && (
        <div className="modal-overlay" onClick={() => setSavePrompt(null)}>
          <div className="modal" onClick={e => e.stopPropagation()}>
            <h3>Name your process</h3>
            <div className="field">
              <label>Name</label>
              <input
                autoFocus
                value={savePrompt.name === 'Untitled process' ? '' : savePrompt.name}
                placeholder="e.g. Order Approval"
                onChange={e => setSavePrompt(p => p && { ...p, name: e.target.value })}
              />
            </div>
            <div className="field">
              <label>Key</label>
              <input
                value={savePrompt.key}
                placeholder="e.g. order-approval"
                onChange={e => setSavePrompt(p => p && { ...p, key: e.target.value.replace(/\s+/g, '-').toLowerCase() })}
              />
            </div>
            <div className="modal-actions">
              <button className="btn-ghost" onClick={() => setSavePrompt(null)}>Cancel</button>
              <button
                className="btn-primary"
                disabled={!savePrompt.name || !savePrompt.key || saveMut.isPending}
                onClick={() => {
                  setName(savePrompt.name)
                  setKey(savePrompt.key)
                  saveMut.mutate({ name: savePrompt.name, key: savePrompt.key })
                  setSavePrompt(null)
                }}
              >
                {saveMut.isPending ? 'Saving…' : 'Save'}
              </button>
            </div>
          </div>
        </div>
      )}

    </div>
  )
}
