import { useEffect, useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import {
  fetchAdminOrg, patchAdminOrg, fetchAuthConfig, patchAuthConfig,
  createOrgUser, listBuiltinRoles, grantOrgRole,
} from '../api/admin'
import { useAuth } from '../context/AuthContext'
import { TOKEN_KEY } from '../api/client'

// ─── Helpers ──────────────────────────────────────────────────────────────────

type StepNum = 1 | 2 | 3

const MILESTONES: { num: StepNum; label: string }[] = [
  { num: 1, label: 'Org' },
  { num: 2, label: 'Auth' },
  { num: 3, label: 'Team' },
]

/**
 * One-line summaries of the built-in roles, shown in the wizard's
 * role-picker help panel. Keep these in sync with the catalog comments
 * in migrations/031_permission_catalog.sql.
 */
const ROLE_DESCRIPTIONS: Record<string, string> = {
  OrgOwner: 'Full control of the organisation — every permission, including delete.',
  OrgAdmin: 'Manage users, roles, and auth config. No process or instance authoring.',
  Developer: 'Create, edit, deploy, and run processes and decisions. Day-to-day builder role.',
  Modeller: 'Design BPMN and DMN drafts. Cannot promote to live versions.',
  Operator: 'Start and monitor instances; complete tasks. No design or deploy.',
  Reader: 'Read-only access to everything in the org.',
}

// ─── Main component ───────────────────────────────────────────────────────────

export default function Welcome() {
  const qc = useQueryClient()
  const { refreshUser, user } = useAuth()
  const currentOrgId = user?.orgs?.[0]?.id ?? null

  const [step, setStep] = useState<StepNum>(1)

  // Step 1 is read-only — the platform admin already set the org name when
  // provisioning. The Org Admin can rename later via /admin/settings.

  // Step 2 — auth config
  const [provider, setProvider] = useState<'internal' | 'oidc'>('internal')
  const [oidcIssuer, setOidcIssuer] = useState('')
  const [oidcClientId, setOidcClientId] = useState('')
  const [oidcClientSecret, setOidcClientSecret] = useState('')
  const [oidcRedirectUri, setOidcRedirectUri] = useState('')

  // Step 3 — invite a teammate (optional). One inline form; users can
  // add more later from Admin → Users.
  const [inviteEmail, setInviteEmail] = useState('')
  const [inviteName, setInviteName] = useState('')
  const [invitePhone, setInvitePhone] = useState('')
  const [inviteAuth, setInviteAuth] = useState<'internal' | 'external'>('internal')
  const [invitePassword, setInvitePassword] = useState('')
  const [inviteExternalId, setInviteExternalId] = useState('')
  const [inviteRoleId, setInviteRoleId] = useState('')
  const [invitedCount, setInvitedCount] = useState(0)
  const [roleHelpOpen, setRoleHelpOpen] = useState(false)

  const [error, setError] = useState('')
  const [completing, setCompleting] = useState(false)

  const orgQ = useQuery({
    queryKey: ['admin-org', currentOrgId],
    queryFn: () => fetchAdminOrg(currentOrgId!),
    enabled: !!currentOrgId,
  })
  const authConfigQ = useQuery({
    queryKey: ['admin-auth-config', currentOrgId],
    queryFn: () => fetchAuthConfig(currentOrgId!),
    enabled: !!currentOrgId,
  })

  useEffect(() => {
    if (!authConfigQ.data) return
    setProvider(authConfigQ.data.provider)
    setOidcIssuer(authConfigQ.data.oidc_issuer ?? '')
    setOidcClientId(authConfigQ.data.oidc_client_id ?? '')
    setOidcRedirectUri(
      authConfigQ.data.oidc_redirect_uri ?? `${window.location.origin}/auth/callback`
    )
  }, [authConfigQ.data]) // eslint-disable-line react-hooks/exhaustive-deps

  const patchAuthMut = useMutation({
    mutationFn: (body: Parameters<typeof patchAuthConfig>[1]) => patchAuthConfig(currentOrgId!, body),
  })

  // Builtin roles for the invite step's role picker. The org admin can
  // see every builtin via /api/v1/roles regardless of their own grants.
  const rolesQ = useQuery({
    queryKey: ['builtin-roles'],
    queryFn: listBuiltinRoles,
  })

  useEffect(() => {
    if (inviteRoleId || !rolesQ.data) return
    const developer = rolesQ.data.find(r => r.name === 'Developer' && r.org_id === null)
    if (developer) setInviteRoleId(developer.id)
  }, [rolesQ.data, inviteRoleId])

  const inviteMut = useMutation({
    mutationFn: async () => {
      const trimmedName = inviteName.trim()
      const trimmedPhone = invitePhone.trim()
      const u = await createOrgUser(currentOrgId!, {
        email: inviteEmail.trim(),
        auth_provider: inviteAuth,
        password: inviteAuth === 'internal' ? invitePassword : undefined,
        external_id: inviteAuth === 'external' ? inviteExternalId.trim() : undefined,
        name: trimmedName || undefined,
        phone: trimmedPhone || undefined,
      })
      if (inviteRoleId) {
        await grantOrgRole(currentOrgId!, u.id, inviteRoleId)
      }
      return u
    },
    onSuccess: () => {
      setInviteEmail('')
      setInviteName('')
      setInvitePhone('')
      setInvitePassword('')
      setInviteExternalId('')
      setInvitedCount(n => n + 1)
      setError('')
    },
    onError: (e: Error) => setError(e.message),
  })

  const completeSetup = async () => {
    setCompleting(true)
    try {
      if (currentOrgId) await patchAdminOrg(currentOrgId, { setup_completed: true })
    } catch { /* best-effort */ }
    if (localStorage.getItem(TOKEN_KEY)) {
      await refreshUser()
    }
    qc.invalidateQueries({ queryKey: ['orgs'] })
    // refreshUser() updated setup_completed → Layout shows <Outlet />
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

  const handleFinish = async () => {
    setError('')
    try {
      await completeSetup()
    } catch (e) { setError((e as Error).message) }
  }

  const isDataLoading = orgQ.isLoading || authConfigQ.isLoading
  const isPending = patchAuthMut.isPending || inviteMut.isPending || completing

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

        {/* Step 3 — Invite teammates (optional) */}
        {step === 3 && (
          <>
            <p style={{ fontSize: 13, color: 'var(--color-text-muted)', marginBottom: 16 }}>
              Invite people who'll work in this organisation. Grant them
              <strong> Developer </strong>or<strong> Modeller </strong>so they can create
              process groups and processes. You can add more later from
              <strong> Admin → Users</strong>.
            </p>

            <div style={{ display: 'flex', flexDirection: 'column', gap: 10, marginBottom: 12 }}>
              <div className="field">
                <label>Email</label>
                <input
                  autoFocus
                  type="email"
                  value={inviteEmail}
                  placeholder="teammate@example.com"
                  onChange={e => setInviteEmail(e.target.value)}
                />
              </div>

              <div style={{ display: 'flex', gap: 10 }}>
                <div className="field" style={{ flex: 1 }}>
                  <label>Name <span style={{ fontWeight: 400, color: 'var(--color-text-muted)', textTransform: 'none', letterSpacing: 0 }}>(optional)</span></label>
                  <input
                    type="text"
                    value={inviteName}
                    placeholder="Jane Doe"
                    onChange={e => setInviteName(e.target.value)}
                  />
                </div>
                <div className="field" style={{ flex: 1 }}>
                  <label>Phone <span style={{ fontWeight: 400, color: 'var(--color-text-muted)', textTransform: 'none', letterSpacing: 0 }}>(optional)</span></label>
                  <input
                    type="tel"
                    value={invitePhone}
                    placeholder="+1 555 123 4567"
                    onChange={e => setInvitePhone(e.target.value)}
                  />
                </div>
              </div>

              <div>
                <label style={{
                  display: 'block', fontSize: 11, fontWeight: 600,
                  color: 'var(--color-text-muted)', textTransform: 'uppercase',
                  letterSpacing: '0.04em', marginBottom: 6,
                }}>Auth provider</label>
                <div style={{ display: 'flex', gap: 8 }}>
                  {(['internal', 'external'] as const).map(p => (
                    <label
                      key={p}
                      style={{
                        flex: 1, display: 'flex', alignItems: 'center', gap: 6,
                        padding: '6px 10px',
                        border: `1px solid ${inviteAuth === p ? 'var(--color-primary)' : 'var(--color-border)'}`,
                        borderRadius: 5, cursor: 'pointer', fontSize: 12,
                        background: inviteAuth === p
                          ? 'var(--color-primary-soft, color-mix(in srgb, var(--color-primary) 8%, transparent))'
                          : 'transparent',
                      }}
                    >
                      <input type="radio" name="invite-auth" checked={inviteAuth === p}
                        onChange={() => setInviteAuth(p)} />
                      {p === 'internal' ? 'Internal (password)' : 'External (OIDC)'}
                    </label>
                  ))}
                </div>
              </div>

              {inviteAuth === 'internal' ? (
                <div className="field">
                  <label>Initial password</label>
                  <input
                    type="password"
                    value={invitePassword}
                    placeholder="At least 8 characters"
                    onChange={e => setInvitePassword(e.target.value)}
                  />
                </div>
              ) : (
                <div className="field">
                  <label>External ID</label>
                  <input
                    type="text"
                    value={inviteExternalId}
                    placeholder="Subject identifier from your IdP"
                    onChange={e => setInviteExternalId(e.target.value)}
                  />
                </div>
              )}

              <div className="field">
                <label style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                  <span>Role</span>
                  <button
                    type="button"
                    aria-label={roleHelpOpen ? 'Hide role help' : 'Show role help'}
                    aria-expanded={roleHelpOpen}
                    onClick={() => setRoleHelpOpen(v => !v)}
                    style={{
                      width: 16, height: 16, borderRadius: '50%',
                      border: '1px solid var(--color-border)',
                      background: roleHelpOpen ? 'var(--color-primary)' : 'transparent',
                      color: roleHelpOpen ? '#fff' : 'var(--color-text-muted)',
                      fontSize: 10, fontWeight: 700, lineHeight: 1,
                      display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
                      cursor: 'pointer', padding: 0,
                    }}
                  >
                    ?
                  </button>
                </label>
                <select
                  value={inviteRoleId}
                  onChange={e => setInviteRoleId(e.target.value)}
                >
                  <option value="">(no role)</option>
                  {(rolesQ.data ?? [])
                    .filter(r => r.org_id === null && r.name !== 'PlatformAdmin')
                    .map(r => (
                      <option key={r.id} value={r.id}>{r.name}</option>
                    ))}
                </select>
                {roleHelpOpen && (
                  <div
                    role="region"
                    aria-label="Built-in role descriptions"
                    style={{
                      marginTop: 8,
                      padding: 10,
                      border: '1px solid var(--color-border)',
                      borderRadius: 5,
                      background: 'var(--color-surface-2)',
                      fontSize: 12,
                      lineHeight: 1.5,
                    }}
                  >
                    {(rolesQ.data ?? [])
                      .filter(r => r.org_id === null && r.name !== 'PlatformAdmin')
                      .map((r, i, arr) => (
                        <div key={r.id} style={{
                          marginBottom: i < arr.length - 1 ? 6 : 0,
                          paddingBottom: i < arr.length - 1 ? 6 : 0,
                          borderBottom: i < arr.length - 1 ? '1px solid var(--color-border)' : 'none',
                        }}>
                          <span style={{
                            fontWeight: 600,
                            color: r.id === inviteRoleId ? 'var(--color-primary)' : 'var(--color-text)',
                          }}>{r.name}</span>
                          {' — '}
                          <span style={{ color: 'var(--color-text-muted)' }}>
                            {ROLE_DESCRIPTIONS[r.name] ?? 'Custom role.'}
                          </span>
                        </div>
                      ))}
                  </div>
                )}
              </div>
            </div>

            {invitedCount > 0 && (
              <div style={{ fontSize: 12, color: 'var(--status-success, #2a8f3e)', marginBottom: 12 }}>
                Invited {invitedCount} teammate{invitedCount === 1 ? '' : 's'}. Add more or finish below.
              </div>
            )}
            {error && <div className="error-banner" style={{ marginBottom: 12 }}>{error}</div>}

            <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
              <button className="btn-ghost" disabled={isPending} onClick={() => { setStep(2); setError('') }}>
                ← Back
              </button>
              <button
                className="btn-ghost"
                disabled={isPending}
                onClick={handleFinish}
              >
                {completing
                  ? 'Finishing…'
                  : invitedCount > 0 ? 'Finish' : 'Skip & finish'}
              </button>
              <button
                className="btn-primary"
                disabled={
                  isPending ||
                  inviteEmail.trim().length === 0 ||
                  (inviteAuth === 'internal'
                    ? invitePassword.length < 8
                    : inviteExternalId.trim().length === 0)
                }
                onClick={() => inviteMut.mutate()}
              >
                {inviteMut.isPending ? 'Inviting…' : 'Invite teammate'}
              </button>
            </div>
          </>
        )}
      </div>

    </div>
  )
}
