import { useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { createOrg } from '../../api/orgs'
import { createAdminUser, listAdminRoles } from '../../api/admin'

type Step = 1 | 2

const MILESTONES: { num: Step; label: string }[] = [
  { num: 1, label: 'Organisation' },
  { num: 2, label: 'Org Admin' },
]

function slugify(v: string) {
  return v.toLowerCase().replace(/\s+/g, '-').replace(/[^a-z0-9-]/g, '')
}

interface Props {
  onComplete: () => void
  onCancel: () => void
}

export default function InstanceSetup({ onComplete, onCancel }: Props) {
  const qc = useQueryClient()
  const [step, setStep] = useState<Step>(1)

  // Step 1
  const [orgName, setOrgName] = useState('')
  const [orgSlug, setOrgSlug] = useState('')
  const [slugTouched, setSlugTouched] = useState(false)

  // Step 2
  const [adminEmail, setAdminEmail] = useState('')
  const [adminPassword, setAdminPassword] = useState('')

  // Created state — persists across step 2 errors so the org is not duplicated.
  const [createdOrgId, setCreatedOrgId] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  const rolesQ = useQuery({ queryKey: ['admin-roles'], queryFn: listAdminRoles })

  const createOrgMut = useMutation({
    mutationFn: () => createOrg({ name: orgName.trim(), slug: orgSlug.trim() }),
  })

  const createUserMut = useMutation({
    mutationFn: (body: {
      org_id: string
      email: string
      password: string
      role_ids: string[]
    }) =>
      createAdminUser({
        org_id: body.org_id,
        email: body.email,
        auth_provider: 'internal',
        password: body.password,
        role_ids: body.role_ids,
      }),
  })

  const handleStep1 = async () => {
    setError(null)
    if (!orgName.trim()) { setError('Organisation name is required.'); return }
    if (!orgSlug.trim()) { setError('Slug is required.'); return }
    if (orgSlug.toLowerCase() === 'conduit') {
      setError("The slug 'conduit' is reserved.")
      return
    }
    try {
      const org = await createOrgMut.mutateAsync()
      setCreatedOrgId(org.id)
      qc.invalidateQueries({ queryKey: ['orgs'] })
      setStep(2)
    } catch (e) {
      setError((e as Error).message)
    }
  }

  const handleStep2 = async () => {
    setError(null)
    if (!createdOrgId) { setError('Org was not created — please retry step 1.'); return }
    if (!adminEmail.trim() || !adminPassword) {
      setError('Email and password are required.')
      return
    }
    const orgAdminRole = (rolesQ.data ?? []).find(
      r => r.name === 'Org Admin' && r.org_id === null
    )
    if (!orgAdminRole) {
      setError('Built-in "Org Admin" role missing — migration 025 not applied?')
      return
    }
    try {
      await createUserMut.mutateAsync({
        org_id: createdOrgId,
        email: adminEmail.trim(),
        password: adminPassword,
        role_ids: [orgAdminRole.id],
      })
      onComplete()
    } catch (e) {
      setError((e as Error).message)
    }
  }

  const pending = createOrgMut.isPending || createUserMut.isPending

  return (
    <div style={{ maxWidth: 560, margin: '0 auto', padding: '32px 24px' }}>
      <div style={{ marginBottom: 28 }}>
        <h1 style={{ fontSize: 20, fontWeight: 700, marginBottom: 6 }}>Create an organisation</h1>
        <p style={{ fontSize: 13, color: 'var(--color-text-muted)', lineHeight: 1.5 }}>
          Provision a new tenant and seed its first Org Admin. The admin will complete the
          process-group and first-process setup on their next sign-in.
        </p>
      </div>

      {/* Milestone bar */}
      <div style={{ display: 'flex', alignItems: 'flex-start', marginBottom: 28 }}>
        {MILESTONES.map((m, i) => (
          <div key={m.num} style={{ display: 'flex', alignItems: 'flex-start', flex: i < MILESTONES.length - 1 ? 1 : 'none' }}>
            <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 6, flexShrink: 0 }}>
              <div style={{
                width: 26, height: 26, borderRadius: '50%',
                display: 'flex', alignItems: 'center', justifyContent: 'center',
                fontSize: 12, fontWeight: 700,
                background: step === m.num ? 'var(--color-primary)' : step > m.num ? 'transparent' : 'var(--color-surface-2)',
                color: step === m.num ? '#fff' : step > m.num ? 'var(--color-primary)' : 'var(--color-text-muted)',
                border: step >= m.num ? '2px solid var(--color-primary)' : '2px solid var(--color-border)',
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
                flex: 1, height: 2, marginTop: 12, marginLeft: 8, marginRight: 8,
                background: step > m.num ? 'var(--color-primary)' : 'var(--color-border)',
                transition: 'background 0.2s',
              }} />
            )}
          </div>
        ))}
      </div>

      <div style={{ maxWidth: 420 }}>
        {step === 1 && (
          <>
            <div className="field">
              <label>Organisation name</label>
              <input
                autoFocus
                value={orgName}
                placeholder="e.g. Acme Corp"
                onChange={e => {
                  setOrgName(e.target.value)
                  if (!slugTouched) setOrgSlug(slugify(e.target.value))
                }}
                onKeyDown={e => e.key === 'Enter' && handleStep1()}
              />
            </div>
            <div className="field">
              <label>Slug</label>
              <input
                value={orgSlug}
                placeholder="acme"
                onChange={e => { setOrgSlug(slugify(e.target.value)); setSlugTouched(true) }}
                onKeyDown={e => e.key === 'Enter' && handleStep1()}
              />
              <div style={{ fontSize: 11, color: 'var(--color-text-muted)', marginTop: 4 }}>
                Used at login. Lowercase letters, digits, and dashes only.
              </div>
            </div>

            {error && <div className="error-banner" style={{ marginBottom: 12 }}>{error}</div>}

            <div style={{ display: 'flex', gap: 8 }}>
              <button className="btn-ghost" onClick={onCancel} disabled={pending}>Cancel</button>
              <button
                className="btn-primary"
                disabled={!orgName.trim() || !orgSlug.trim() || pending}
                onClick={handleStep1}
              >
                {createOrgMut.isPending ? 'Creating…' : 'Continue →'}
              </button>
            </div>
          </>
        )}

        {step === 2 && (
          <>
            <div style={{ marginBottom: 16, fontSize: 13, color: 'var(--color-text-muted)', lineHeight: 1.5 }}>
              This user becomes the first Org Admin for <strong style={{ color: 'var(--color-text)' }}>{orgName}</strong>.
              They can manage users, roles, and processes inside the org.
            </div>

            <div className="field">
              <label>Admin email</label>
              <input
                autoFocus
                type="email"
                value={adminEmail}
                placeholder="admin@example.com"
                onChange={e => setAdminEmail(e.target.value)}
                onKeyDown={e => e.key === 'Enter' && handleStep2()}
              />
            </div>
            <div className="field">
              <label>Initial password</label>
              <input
                type="password"
                value={adminPassword}
                placeholder="Set an initial password"
                onChange={e => setAdminPassword(e.target.value)}
                onKeyDown={e => e.key === 'Enter' && handleStep2()}
              />
              <div style={{ fontSize: 11, color: 'var(--color-text-muted)', marginTop: 4 }}>
                Share this securely. The admin can change it after first sign-in.
              </div>
            </div>

            {error && <div className="error-banner" style={{ marginBottom: 12 }}>{error}</div>}

            <div style={{ display: 'flex', gap: 8 }}>
              <button className="btn-ghost" onClick={onComplete} disabled={pending}>Done — skip admin</button>
              <button
                className="btn-primary"
                disabled={!adminEmail.trim() || !adminPassword || pending || rolesQ.isLoading}
                onClick={handleStep2}
              >
                {createUserMut.isPending ? 'Creating…' : 'Create admin'}
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  )
}
