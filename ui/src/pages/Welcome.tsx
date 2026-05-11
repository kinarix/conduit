import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { fetchAdminOrg, patchAdminOrg, fetchAuthConfig, patchAuthConfig } from '../api/admin'
import { createProcessGroup, type ProcessGroup } from '../api/processGroups'
import { createDraft } from '../api/deployments'
import { defaultBpmnXml } from '../components/bpmn/defaultBpmn'
import { useOrg } from '../App'
import { useAuth } from '../context/AuthContext'
import { TOKEN_KEY } from '../api/client'

// ─── Concept card SVGs ────────────────────────────────────────────────────────

const STROKE = 'var(--color-text-muted)'
const ACCENT = 'var(--color-primary)'

const CONCEPTS = [
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

function expandInStorage(key: string, id: string) {
  try {
    const raw = localStorage.getItem(key)
    const arr: string[] = Array.isArray(JSON.parse(raw ?? 'null')) ? JSON.parse(raw!) : []
    if (!arr.includes(id)) arr.push(id)
    localStorage.setItem(key, JSON.stringify(arr))
    window.dispatchEvent(new CustomEvent('sidebar:expansion-sync', { detail: key }))
  } catch { /* ignore quota / disabled storage */ }
}

type StepNum = 1 | 2 | 3 | 4

const MILESTONES: { num: StepNum; label: string }[] = [
  { num: 1, label: 'Org' },
  { num: 2, label: 'Auth' },
  { num: 3, label: 'Process Group' },
  { num: 4, label: 'Process' },
]

// ─── Main component ───────────────────────────────────────────────────────────

export default function Welcome() {
  const navigate = useNavigate()
  const qc = useQueryClient()
  const { setOrg } = useOrg()
  const { refreshUser } = useAuth()

  const [step, setStep] = useState<StepNum>(1)
  const [createdGroup, setCreatedGroup] = useState<ProcessGroup | null>(null)

  // Step 1 is read-only — the platform admin already set the org name when
  // provisioning. The Org Admin can rename later via /admin/settings.

  // Step 2
  const [provider, setProvider] = useState<'internal' | 'oidc'>('internal')
  const [oidcIssuer, setOidcIssuer] = useState('')
  const [oidcClientId, setOidcClientId] = useState('')
  const [oidcClientSecret, setOidcClientSecret] = useState('')
  const [oidcRedirectUri, setOidcRedirectUri] = useState('')

  // Step 3
  const [groupName, setGroupName] = useState('')

  // Step 4
  const [processName, setProcessName] = useState('')
  const [processKey, setProcessKey] = useState('')

  const [error, setError] = useState('')
  const [completing, setCompleting] = useState(false)

  const orgQ = useQuery({ queryKey: ['admin-org'], queryFn: fetchAdminOrg })
  const authConfigQ = useQuery({ queryKey: ['admin-auth-config'], queryFn: fetchAuthConfig })

  useEffect(() => {
    if (!authConfigQ.data) return
    setProvider(authConfigQ.data.provider)
    setOidcIssuer(authConfigQ.data.oidc_issuer ?? '')
    setOidcClientId(authConfigQ.data.oidc_client_id ?? '')
    setOidcRedirectUri(
      authConfigQ.data.oidc_redirect_uri ?? `${window.location.origin}/auth/callback`
    )
  }, [authConfigQ.data]) // eslint-disable-line react-hooks/exhaustive-deps

  const patchAuthMut = useMutation({ mutationFn: patchAuthConfig })

  const createGroupMut = useMutation({
    mutationFn: () => createProcessGroup(orgQ.data!.id, groupName),
    onSuccess: g => { setCreatedGroup(g); setError(''); setStep(4) },
    onError: (e: Error) => setError(e.message),
  })

  const createProcessMut = useMutation({
    mutationFn: () => createDraft({
      org_id: orgQ.data!.id,
      process_group_id: createdGroup!.id,
      key: processKey,
      name: processName,
      bpmn_xml: defaultBpmnXml(processKey, processName),
    }),
  })

  const completeSetup = async (defId?: string) => {
    setCompleting(true)
    try {
      await patchAdminOrg({ setup_completed: true })
    } catch { /* best-effort */ }
    if (localStorage.getItem(TOKEN_KEY)) {
      await refreshUser()
    }

    if (defId && orgQ.data && createdGroup) {
      expandInStorage('sidebar.orgs', orgQ.data.id)
      expandInStorage(`sidebar.groups.${orgQ.data.id}`, createdGroup.id)
      setOrg(orgQ.data)
      qc.invalidateQueries({ queryKey: ['orgs'] })
      qc.invalidateQueries({ queryKey: ['process-groups', orgQ.data.id] })
      navigate(`/definitions/${defId}/edit`)
    }
    // If no defId, refreshUser() updated setup_completed → Layout shows <Outlet />
  }

  const handleStep2 = async () => {
    setError('')
    try {
      await patchAuthMut.mutateAsync(
        provider === 'oidc'
          ? {
              provider: 'oidc',
              oidc_issuer: oidcIssuer || null,
              oidc_client_id: oidcClientId || null,
              oidc_client_secret: oidcClientSecret || undefined,
              oidc_redirect_uri: oidcRedirectUri || null,
            }
          : { provider: 'internal', oidc_issuer: null, oidc_client_id: null, oidc_redirect_uri: null }
      )
      setStep(3)
    } catch (e) { setError((e as Error).message) }
  }

  const handleOpenInEditor = async () => {
    setError('')
    try {
      const def = await createProcessMut.mutateAsync()
      await completeSetup(def.id)
    } catch (e) { setError((e as Error).message) }
  }

  const handleSkipProcess = async () => {
    setError('')
    try {
      await completeSetup()
    } catch (e) { setError((e as Error).message) }
  }

  const isDataLoading = orgQ.isLoading || authConfigQ.isLoading
  const isPending = patchAuthMut.isPending ||
    createGroupMut.isPending || createProcessMut.isPending || completing

  if (isDataLoading) {
    return (
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100%' }}>
        <div className="spinner" />
      </div>
    )
  }

  return (
    <div style={{ maxWidth: 900, margin: '0 auto', padding: '40px 24px' }}>

      {/* ── Page header ── */}
      <div style={{ marginBottom: 36 }}>
        <h1 style={{ fontSize: 22, fontWeight: 700, marginBottom: 6 }}>Welcome to Conduit</h1>
        <p style={{ fontSize: 14, color: 'var(--color-text-muted)', lineHeight: 1.5 }}>
          Configure your workspace and create your first process to get started.
        </p>
      </div>

      {/* ── Milestone progress line ── */}
      <div style={{ display: 'flex', alignItems: 'flex-start', marginBottom: 32 }}>
        {MILESTONES.map((m, i) => (
          <div key={m.num} style={{ display: 'flex', alignItems: 'flex-start', flex: i < MILESTONES.length - 1 ? 1 : 'none' }}>
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
                border: step >= m.num
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

        {/* Step 1 — Welcome / org confirmation (read-only) */}
        {step === 1 && (
          <>
            <h2 style={{ fontSize: 18, fontWeight: 700, marginBottom: 6 }}>
              Welcome to {orgQ.data?.name}
            </h2>
            <p style={{ fontSize: 13, color: 'var(--color-text-muted)', lineHeight: 1.5, marginBottom: 14 }}>
              Your platform administrator created this organisation for you.
              You can rename it later in <strong>Admin → Settings</strong>.
            </p>
            <div style={{ fontSize: 12, color: 'var(--color-text-muted)', marginBottom: 20 }}>
              Slug: <code style={{ fontFamily: 'monospace' }}>{orgQ.data?.slug}</code>
            </div>
            {error && <div className="error-banner" style={{ marginBottom: 12 }}>{error}</div>}
            <button
              className="btn-primary"
              disabled={isPending}
              onClick={() => setStep(2)}
              autoFocus
            >
              Get started →
            </button>
          </>
        )}

        {/* Step 2 — Auth Provider */}
        {step === 2 && (
          <>
            <div style={{ marginBottom: 16 }}>
              <p style={{ fontSize: 13, color: 'var(--color-text-muted)', marginBottom: 12 }}>
                Choose how users authenticate. You can change this later in Admin → Auth.
              </p>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
                {(['internal', 'oidc'] as const).map(p => (
                  <label
                    key={p}
                    style={{
                      display: 'flex',
                      alignItems: 'flex-start',
                      gap: 10,
                      padding: '10px 12px',
                      border: `1px solid ${provider === p ? 'var(--color-primary)' : 'var(--color-border)'}`,
                      borderRadius: 6,
                      cursor: 'pointer',
                      background: provider === p ? 'var(--color-primary-soft, color-mix(in srgb, var(--color-primary) 8%, transparent))' : 'var(--color-surface)',
                      transition: 'border-color 0.15s, background 0.15s',
                    }}
                  >
                    <input
                      type="radio"
                      name="provider"
                      value={p}
                      checked={provider === p}
                      onChange={() => setProvider(p)}
                      style={{ marginTop: 2 }}
                    />
                    <div>
                      <div style={{ fontSize: 13, fontWeight: 600 }}>
                        {p === 'internal' ? 'Internal (password)' : 'External OIDC'}
                      </div>
                      <div style={{ fontSize: 12, color: 'var(--color-text-muted)', marginTop: 2 }}>
                        {p === 'internal'
                          ? 'Users log in with email and password stored in Conduit.'
                          : 'Delegate authentication to an external identity provider (e.g. Okta, Auth0, Keycloak).'}
                      </div>
                    </div>
                  </label>
                ))}
              </div>
            </div>

            {provider === 'oidc' && (
              <div style={{ display: 'flex', flexDirection: 'column', gap: 10, marginBottom: 16 }}>
                <div className="field">
                  <label>Issuer URL</label>
                  <input
                    value={oidcIssuer}
                    placeholder="https://your-idp.example.com"
                    onChange={e => setOidcIssuer(e.target.value)}
                  />
                </div>
                <div className="field">
                  <label>Client ID</label>
                  <input
                    value={oidcClientId}
                    placeholder="your-client-id"
                    onChange={e => setOidcClientId(e.target.value)}
                  />
                </div>
                <div className="field">
                  <label>Client secret</label>
                  <input
                    type="password"
                    value={oidcClientSecret}
                    placeholder={authConfigQ.data?.oidc_client_secret_set ? 'Already set — enter to update' : ''}
                    onChange={e => setOidcClientSecret(e.target.value)}
                  />
                </div>
                <div className="field">
                  <label>Redirect URI</label>
                  <input
                    value={oidcRedirectUri}
                    onChange={e => setOidcRedirectUri(e.target.value)}
                  />
                </div>
              </div>
            )}

            {error && <div className="error-banner" style={{ marginBottom: 12 }}>{error}</div>}
            <div style={{ display: 'flex', gap: 8 }}>
              <button className="btn-ghost" disabled={isPending} onClick={() => { setStep(1); setError('') }}>← Back</button>
              <button className="btn-ghost" disabled={isPending} onClick={() => { setError(''); setStep(3) }}>
                Skip for now
              </button>
              <button className="btn-primary" disabled={isPending} onClick={handleStep2}>
                {isPending ? 'Saving…' : 'Save & continue →'}
              </button>
            </div>
          </>
        )}

        {/* Step 3 — Process Group */}
        {step === 3 && (
          <>
            <p style={{ fontSize: 13, color: 'var(--color-text-muted)', marginBottom: 16 }}>
              Process groups organise related processes by team or domain.
            </p>
            <div className="field">
              <label>Process group name</label>
              <input
                autoFocus
                value={groupName}
                placeholder="e.g. Order Management"
                onChange={e => setGroupName(e.target.value)}
                onKeyDown={e => e.key === 'Enter' && groupName.trim() && createGroupMut.mutate()}
              />
            </div>
            {error && <div className="error-banner" style={{ marginBottom: 12 }}>{error}</div>}
            <div style={{ display: 'flex', gap: 8 }}>
              <button className="btn-ghost" disabled={isPending} onClick={() => { setStep(2); setError('') }}>← Back</button>
              <button
                className="btn-primary"
                disabled={!groupName.trim() || isPending}
                onClick={() => createGroupMut.mutate()}
              >
                {isPending ? 'Creating…' : 'Continue →'}
              </button>
            </div>
          </>
        )}

        {/* Step 4 — First Process */}
        {step === 4 && (
          <>
            <p style={{ fontSize: 13, color: 'var(--color-text-muted)', marginBottom: 16 }}>
              Create your first process definition and open it in the modeller. You can skip this and create one later.
            </p>
            <div className="field">
              <label>Process name</label>
              <input
                autoFocus
                value={processName}
                placeholder="e.g. Order Approval"
                onChange={e => { setProcessName(e.target.value); setProcessKey(slugify(e.target.value)) }}
                onKeyDown={e => e.key === 'Enter' && processName.trim() && processKey.trim() && handleOpenInEditor()}
              />
            </div>
            <div className="field">
              <label>Key</label>
              <input
                value={processKey}
                placeholder="e.g. order-approval"
                onChange={e => setProcessKey(slugify(e.target.value))}
                onKeyDown={e => e.key === 'Enter' && processName.trim() && processKey.trim() && handleOpenInEditor()}
              />
            </div>
            {error && <div className="error-banner" style={{ marginBottom: 12 }}>{error}</div>}
            <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
              <button className="btn-ghost" disabled={isPending} onClick={handleSkipProcess}>
                {completing ? 'Finishing…' : 'Skip for now'}
              </button>
              <button
                className="btn-primary"
                disabled={!processName.trim() || !processKey.trim() || isPending}
                onClick={handleOpenInEditor}
              >
                {isPending && !completing ? 'Opening editor…' : 'Open in editor →'}
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
