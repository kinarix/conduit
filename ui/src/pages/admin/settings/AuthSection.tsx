import { useEffect, useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { fetchAuthConfig, patchAuthConfig } from '../../../api/admin'
import { useOrg } from '../../../App'

export default function AuthSection() {
  const qc = useQueryClient()
  const { org } = useOrg()
  const orgId = org?.id
  const configQ = useQuery({
    queryKey: ['admin-auth-config', orgId],
    queryFn: () => fetchAuthConfig(orgId!),
    enabled: !!orgId,
  })

  const [provider, setProvider] = useState<'internal' | 'oidc'>('internal')
  const [issuer, setIssuer] = useState('')
  const [clientId, setClientId] = useState('')
  const [clientSecret, setClientSecret] = useState('')
  const [redirectUri, setRedirectUri] = useState('')

  useEffect(() => {
    if (!configQ.data) return
    setProvider(configQ.data.provider)
    setIssuer(configQ.data.oidc_issuer ?? '')
    setClientId(configQ.data.oidc_client_id ?? '')
    setRedirectUri(configQ.data.oidc_redirect_uri ?? '')
  }, [configQ.data])

  const saveMut = useMutation({
    mutationFn: () => patchAuthConfig(orgId!, {
      provider,
      oidc_issuer: provider === 'oidc' ? issuer || null : null,
      oidc_client_id: provider === 'oidc' ? clientId || null : null,
      oidc_client_secret: provider === 'oidc' && clientSecret ? clientSecret : undefined,
      oidc_redirect_uri: provider === 'oidc' ? redirectUri || null : null,
    }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['admin-auth-config', orgId] })
      setClientSecret('')
    },
  })

  if (!orgId) return <div style={{ padding: 8, fontSize: 13 }}>Select an organisation.</div>
  if (configQ.isLoading) return <div style={{ padding: 8 }}><div className="spinner" /></div>
  if (configQ.isError) return <div style={{ color: 'var(--status-error)', fontSize: 13 }}>Failed to load auth config.</div>

  return (
    <div style={{ maxWidth: 480 }}>
      <h2 style={{ fontSize: 15, fontWeight: 600, margin: '0 0 20px' }}>Authentication provider</h2>

      <div style={{ display: 'flex', flexDirection: 'column', gap: 10, marginBottom: 24 }}>
        {(['internal', 'oidc'] as const).map(opt => (
          <label
            key={opt}
            style={{
              display: 'flex',
              alignItems: 'flex-start',
              gap: 10,
              padding: '12px 14px',
              border: `1px solid ${provider === opt ? 'var(--color-primary)' : 'var(--color-border)'}`,
              borderRadius: 6,
              cursor: 'pointer',
              background: provider === opt
                ? 'var(--color-primary-soft, color-mix(in srgb, var(--color-primary) 8%, transparent))'
                : 'var(--bg-secondary)',
            }}
          >
            <input
              type="radio"
              name="provider"
              value={opt}
              checked={provider === opt}
              onChange={() => setProvider(opt)}
              style={{ marginTop: 2 }}
            />
            <div>
              <div style={{ fontSize: 13, fontWeight: 500 }}>
                {opt === 'internal' ? 'Internal (username + password)' : 'External OIDC provider'}
              </div>
              <div style={{ fontSize: 12, color: 'var(--color-text-muted)', marginTop: 2 }}>
                {opt === 'internal'
                  ? 'Users authenticate with email and password stored in Conduit'
                  : 'Delegate authentication to an external provider (Okta, Auth0, Google Workspace, etc.)'}
              </div>
            </div>
          </label>
        ))}
      </div>

      {provider === 'oidc' && (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 14, marginBottom: 24 }}>
          <div>
            <label style={labelStyle}>Issuer URL</label>
            <input
              type="url"
              placeholder="https://accounts.example.com"
              value={issuer}
              onChange={e => setIssuer(e.target.value)}
              style={inputStyle}
            />
          </div>
          <div>
            <label style={labelStyle}>Client ID</label>
            <input
              type="text"
              placeholder="client_id"
              value={clientId}
              onChange={e => setClientId(e.target.value)}
              style={inputStyle}
            />
          </div>
          <div>
            <label style={labelStyle}>
              Client secret
              {configQ.data?.oidc_client_secret_set && (
                <span style={{ fontSize: 11, color: 'var(--color-text-muted)', fontWeight: 400, marginLeft: 6 }}>
                  (already set — leave blank to keep)
                </span>
              )}
            </label>
            <input
              type="password"
              placeholder={configQ.data?.oidc_client_secret_set ? '••••••••' : 'client_secret'}
              value={clientSecret}
              onChange={e => setClientSecret(e.target.value)}
              style={inputStyle}
            />
          </div>
          <div>
            <label style={labelStyle}>Redirect URI</label>
            <input
              type="url"
              placeholder="https://conduit.example.com/auth/callback"
              value={redirectUri}
              onChange={e => setRedirectUri(e.target.value)}
              style={inputStyle}
            />
          </div>
        </div>
      )}

      {saveMut.isError && (
        <div style={{ fontSize: 12, color: 'var(--status-error)', marginBottom: 12 }}>
          {(saveMut.error as Error).message}
        </div>
      )}
      {saveMut.isSuccess && (
        <div style={{ fontSize: 12, color: 'var(--status-success)', marginBottom: 12 }}>
          Auth configuration saved.
        </div>
      )}

      <button
        className="btn-primary"
        disabled={saveMut.isPending}
        onClick={() => saveMut.mutate()}
      >
        {saveMut.isPending ? 'Saving…' : 'Save'}
      </button>
    </div>
  )
}

const labelStyle: React.CSSProperties = {
  display: 'block',
  fontSize: 12,
  fontWeight: 500,
  marginBottom: 5,
  color: 'var(--color-text)',
}

const inputStyle: React.CSSProperties = {
  width: '100%',
  padding: '7px 10px',
  fontSize: 13,
  border: '1px solid var(--color-border)',
  borderRadius: 5,
  background: 'var(--bg-primary)',
  color: 'var(--color-text)',
  boxSizing: 'border-box',
}
