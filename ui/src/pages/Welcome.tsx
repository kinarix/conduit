import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { createOrg, type Org } from '../api/orgs'
import { createProcessGroup, type ProcessGroup } from '../api/processGroups'
import { createDraft } from '../api/deployments'
import { defaultBpmnXml } from '../components/bpmn/defaultBpmn'
import { useOrg } from '../App'

// ─── Concept card SVGs ────────────────────────────────────────────────────────

const STROKE = 'var(--color-text-muted)'
const ACCENT = 'var(--color-primary)'

const CONCEPTS = [
  {
    title: 'Organization',
    blurb: 'A workspace that owns its process groups, processes, and people. Everything is scoped under an org.',
    svg: (
      <svg viewBox="0 0 120 80" width="120" height="80" fill="none">
        <rect x="46" y="6" width="28" height="18" rx="3" stroke={ACCENT} strokeWidth="1.5" />
        <rect x="14" y="50" width="28" height="18" rx="3" stroke={STROKE} strokeWidth="1.5" />
        <rect x="46" y="50" width="28" height="18" rx="3" stroke={STROKE} strokeWidth="1.5" />
        <rect x="78" y="50" width="28" height="18" rx="3" stroke={STROKE} strokeWidth="1.5" />
        <path d="M60 24 L60 38 M28 38 L92 38 M28 38 L28 50 M60 38 L60 50 M92 38 L92 50" stroke={STROKE} strokeWidth="1.2" />
      </svg>
    ),
  },
  {
    title: 'Process Group',
    blurb: 'Org units inside an organization. Group related processes by team, domain, or business unit.',
    svg: (
      <svg viewBox="0 0 120 80" width="120" height="80" fill="none">
        <path d="M14 22 L14 64 Q14 68 18 68 L102 68 Q106 68 106 64 L106 30 Q106 26 102 26 L56 26 L48 18 L18 18 Q14 18 14 22 Z"
          stroke={ACCENT} strokeWidth="1.5" />
        <line x1="28" y1="42" x2="92" y2="42" stroke={STROKE} strokeWidth="1" opacity="0.5" />
        <line x1="28" y1="50" x2="80" y2="50" stroke={STROKE} strokeWidth="1" opacity="0.5" />
        <line x1="28" y1="58" x2="70" y2="58" stroke={STROKE} strokeWidth="1" opacity="0.5" />
      </svg>
    ),
  },
  {
    title: 'Process Definition',
    blurb: 'A BPMN blueprint — the steps, gateways, and events that describe how work flows.',
    svg: (
      <svg viewBox="0 0 120 80" width="120" height="80" fill="none">
        <circle cx="16" cy="40" r="6" stroke={ACCENT} strokeWidth="1.5" />
        <rect x="34" y="32" width="22" height="16" rx="2" stroke={STROKE} strokeWidth="1.5" />
        <path d="M68 32 L80 40 L68 48 L56 40 Z" stroke={STROKE} strokeWidth="1.5" />
        <rect x="86" y="32" width="22" height="16" rx="2" stroke={STROKE} strokeWidth="1.5" />
        <line x1="22" y1="40" x2="34" y2="40" stroke={STROKE} strokeWidth="1.2" />
        <line x1="56" y1="40" x2="56" y2="40" stroke={STROKE} strokeWidth="1.2" />
        <line x1="80" y1="40" x2="86" y2="40" stroke={STROKE} strokeWidth="1.2" />
      </svg>
    ),
  },
  {
    title: 'Process Instance',
    blurb: 'A live execution of a definition. Each instance carries its own variables and history.',
    svg: (
      <svg viewBox="0 0 120 80" width="120" height="80" fill="none">
        <rect x="14" y="14" width="92" height="20" rx="3" stroke={STROKE} strokeWidth="1.2" opacity="0.45" />
        <rect x="14" y="38" width="92" height="20" rx="3" stroke={ACCENT} strokeWidth="1.5" />
        <circle cx="26" cy="48" r="3" fill={ACCENT} />
        <line x1="34" y1="48" x2="94" y2="48" stroke={ACCENT} strokeWidth="1.2" strokeDasharray="2 3" />
        <rect x="14" y="62" width="92" height="10" rx="3" stroke={STROKE} strokeWidth="1.2" opacity="0.3" />
      </svg>
    ),
  },
  {
    title: 'Task',
    blurb: 'A unit of work waiting on a person or external worker. Completing it advances the instance.',
    svg: (
      <svg viewBox="0 0 120 80" width="120" height="80" fill="none">
        <rect x="22" y="16" width="76" height="48" rx="4" stroke={ACCENT} strokeWidth="1.5" />
        <line x1="32" y1="30" x2="56" y2="30" stroke={STROKE} strokeWidth="1.2" />
        <line x1="32" y1="40" x2="78" y2="40" stroke={STROKE} strokeWidth="1.2" opacity="0.6" />
        <line x1="32" y1="50" x2="68" y2="50" stroke={STROKE} strokeWidth="1.2" opacity="0.6" />
        <path d="M76 28 L82 34 L92 22" stroke={ACCENT} strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
      </svg>
    ),
  },
]

// ─── Helpers ──────────────────────────────────────────────────────────────────

function slugify(v: string) {
  return v.toLowerCase().replace(/\s+/g, '-').replace(/[^a-z0-9-]/g, '')
}

const MILESTONES = [
  { num: 1 as const, label: 'Organization' },
  { num: 2 as const, label: 'Process Group' },
  { num: 3 as const, label: 'Process' },
]

// ─── Main component ───────────────────────────────────────────────────────────

export default function Welcome() {
  const navigate = useNavigate()
  const qc = useQueryClient()
  const { setOrg } = useOrg()

  const [step, setStep] = useState<1 | 2 | 3>(1)
  const [createdOrg, setCreatedOrg] = useState<Org | null>(null)
  const [createdGroup, setCreatedGroup] = useState<ProcessGroup | null>(null)

  const [orgName, setOrgName] = useState('')
  const [orgSlug, setOrgSlug] = useState('')
  const [groupName, setGroupName] = useState('')
  const [processName, setProcessName] = useState('')
  const [processKey, setProcessKey] = useState('')
  const [error, setError] = useState('')

  // Don't invalidate ['orgs'] until the wizard completes — otherwise Layout.tsx
  // unmounts <Welcome/> mid-wizard when it sees orgs.length > 0.
  const createOrgMut = useMutation({
    mutationFn: () => createOrg({ name: orgName, slug: orgSlug }),
    onSuccess: org => { setCreatedOrg(org); setError(''); setStep(2) },
    onError: (e: Error) => setError(e.message),
  })

  const createGroupMut = useMutation({
    mutationFn: () => createProcessGroup(createdOrg!.id, groupName),
    onSuccess: g => { setCreatedGroup(g); setError(''); setStep(3) },
    onError: (e: Error) => setError(e.message),
  })

  const createProcessMut = useMutation({
    mutationFn: () => createDraft({
      org_id: createdOrg!.id,
      process_group_id: createdGroup!.id,
      key: processKey,
      name: processName,
      bpmn_xml: defaultBpmnXml(processKey, processName),
    }),
  })

  const handleOpenInEditor = async () => {
    setError('')
    let def: Awaited<ReturnType<typeof createDraft>>
    try {
      def = await createProcessMut.mutateAsync()
    } catch (e) {
      setError((e as Error).message)
      return
    }

    // Pre-expand the sidebar tree in localStorage so the org and process group
    // are visible when the editor opens.
    const expandInStorage = (key: string, id: string) => {
      try {
        const raw = localStorage.getItem(key)
        const arr: string[] = Array.isArray(JSON.parse(raw ?? 'null')) ? JSON.parse(raw!) : []
        if (!arr.includes(id)) arr.push(id)
        localStorage.setItem(key, JSON.stringify(arr))
        window.dispatchEvent(new CustomEvent('sidebar:expansion-sync', { detail: key }))
      } catch { /* ignore quota / disabled storage */ }
    }
    expandInStorage('sidebar.orgs', createdOrg!.id)
    expandInStorage(`sidebar.groups.${createdOrg!.id}`, createdGroup!.id)

    setOrg(createdOrg!)
    // Immediately seed the orgs cache so Layout renders <Outlet /> before navigate fires.
    qc.setQueryData<Org[]>(['orgs'], (old = []) =>
      old.some(o => o.id === createdOrg!.id) ? old : [...old, createdOrg!]
    )
    qc.invalidateQueries({ queryKey: ['orgs'] })
    qc.invalidateQueries({ queryKey: ['process-groups', createdOrg!.id] })
    qc.invalidateQueries({ queryKey: ['deployments', createdOrg!.id] })
    navigate(`/definitions/${def.id}/edit`)
  }

  const isPending = createOrgMut.isPending || createGroupMut.isPending || createProcessMut.isPending

  return (
    <div style={{ maxWidth: 900, margin: '0 auto', padding: '40px 24px' }}>

      {/* ── Page header ── */}
      <div style={{ marginBottom: 36 }}>
        <h1 style={{ fontSize: 22, fontWeight: 700, marginBottom: 6 }}>Welcome to Conduit</h1>
        <p style={{ fontSize: 14, color: 'var(--color-text-muted)', lineHeight: 1.5 }}>
          Create your organization, a process group, and your first process to get started.
        </p>
      </div>

      {/* ── Milestone progress line ── */}
      <div style={{ display: 'flex', alignItems: 'flex-start', marginBottom: 32 }}>
        {MILESTONES.map((m, i) => (
          <div key={m.num} style={{ display: 'flex', alignItems: 'flex-start', flex: i < MILESTONES.length - 1 ? 1 : 'none' }}>
            {/* Milestone node */}
            <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 6, flexShrink: 0 }}>
              <div style={{
                width: 28,
                height: 28,
                borderRadius: '50%',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                fontSize: 12,
                fontWeight: 700,
                background: step === m.num
                  ? 'var(--color-primary)'
                  : step > m.num
                    ? 'transparent'
                    : 'var(--color-surface-2)',
                color: step === m.num
                  ? '#fff'
                  : step > m.num
                    ? 'var(--color-primary)'
                    : 'var(--color-text-muted)',
                border: step > m.num
                  ? '2px solid var(--color-primary)'
                  : step === m.num
                    ? '2px solid var(--color-primary)'
                    : '2px solid var(--color-border)',
                transition: 'all 0.2s',
              }}>
                {step > m.num ? '✓' : m.num}
              </div>
              <span style={{
                fontSize: 11,
                fontWeight: step === m.num ? 600 : 400,
                color: step === m.num ? 'var(--color-text)' : 'var(--color-text-muted)',
                whiteSpace: 'nowrap',
              }}>
                {m.label}
              </span>
            </div>

            {/* Connector line */}
            {i < MILESTONES.length - 1 && (
              <div style={{
                flex: 1,
                height: 2,
                marginTop: 13,
                marginLeft: 8,
                marginRight: 8,
                background: step > m.num ? 'var(--color-primary)' : 'var(--color-border)',
                transition: 'background 0.2s',
              }} />
            )}
          </div>
        ))}
      </div>

      {/* ── Active step form ── */}
      <div style={{ maxWidth: 420, marginBottom: 48 }}>

        {step === 1 && (
          <>
            <div className="field">
              <label>Organization name</label>
              <input
                autoFocus
                value={orgName}
                placeholder="e.g. Acme Corp"
                onChange={e => { setOrgName(e.target.value); setOrgSlug(slugify(e.target.value)) }}
                onKeyDown={e => e.key === 'Enter' && orgName && orgSlug && createOrgMut.mutate()}
              />
            </div>
            <div className="field">
              <label>Slug</label>
              <input
                value={orgSlug}
                placeholder="e.g. acme-corp"
                onChange={e => setOrgSlug(slugify(e.target.value))}
                onKeyDown={e => e.key === 'Enter' && orgName && orgSlug && createOrgMut.mutate()}
              />
            </div>
            {error && <div className="error-banner" style={{ marginBottom: 12 }}>{error}</div>}
            <button
              className="btn-primary"
              disabled={!orgName || !orgSlug || isPending}
              onClick={() => createOrgMut.mutate()}
            >
              {isPending ? 'Creating…' : 'Continue →'}
            </button>
          </>
        )}

        {step === 2 && (
          <>
            <div className="field">
              <label>Process group name</label>
              <input
                autoFocus
                value={groupName}
                placeholder="e.g. Order Management"
                onChange={e => setGroupName(e.target.value)}
                onKeyDown={e => e.key === 'Enter' && groupName && createGroupMut.mutate()}
              />
            </div>
            {error && <div className="error-banner" style={{ marginBottom: 12 }}>{error}</div>}
            <div style={{ display: 'flex', gap: 8 }}>
              <button className="btn-ghost" onClick={() => { setStep(1); setError('') }}>← Back</button>
              <button
                className="btn-primary"
                disabled={!groupName || isPending}
                onClick={() => createGroupMut.mutate()}
              >
                {isPending ? 'Creating…' : 'Continue →'}
              </button>
            </div>
          </>
        )}

        {step === 3 && (
          <>
            <div className="field">
              <label>Process name</label>
              <input
                autoFocus
                value={processName}
                placeholder="e.g. Order Approval"
                onChange={e => { setProcessName(e.target.value); setProcessKey(slugify(e.target.value)) }}
                onKeyDown={e => e.key === 'Enter' && processName && processKey && handleOpenInEditor()}
              />
            </div>
            <div className="field">
              <label>Key</label>
              <input
                value={processKey}
                placeholder="e.g. order-approval"
                onChange={e => setProcessKey(slugify(e.target.value))}
                onKeyDown={e => e.key === 'Enter' && processName && processKey && handleOpenInEditor()}
              />
            </div>
            {error && <div className="error-banner" style={{ marginBottom: 12 }}>{error}</div>}
            <div style={{ display: 'flex', gap: 8 }}>
              <button className="btn-ghost" onClick={() => { setStep(2); setError('') }}>← Back</button>
              <button
                className="btn-primary"
                disabled={!processName || !processKey || isPending}
                onClick={handleOpenInEditor}
              >
                {isPending ? 'Opening editor…' : 'Open in editor →'}
              </button>
            </div>
          </>
        )}
      </div>

      {/* ── Divider ── */}
      <div style={{ borderTop: '1px solid var(--color-border)', marginBottom: 32 }} />

      {/* ── Concept cards ── */}
      <h2 style={{ fontSize: 14, fontWeight: 600, color: 'var(--color-text-muted)', textTransform: 'uppercase', letterSpacing: '0.06em', marginBottom: 16 }}>
        Core concepts
      </h2>
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(240px, 1fr))', gap: 16 }}>
        {CONCEPTS.map(c => (
          <div
            key={c.title}
            style={{
              border: '1px solid var(--color-border)',
              borderRadius: 6,
              padding: 16,
              background: 'var(--color-surface)',
            }}
          >
            <div style={{
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              background: 'var(--color-surface-2)',
              borderRadius: 4,
              marginBottom: 12,
              padding: 8,
              minHeight: 96,
            }}>
              {c.svg}
            </div>
            <div style={{ fontSize: 14, fontWeight: 600, marginBottom: 4 }}>{c.title}</div>
            <div style={{ fontSize: 12, color: 'var(--color-text-muted)', lineHeight: 1.5 }}>{c.blurb}</div>
          </div>
        ))}
      </div>
    </div>
  )
}
