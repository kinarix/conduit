import { useEffect, useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import {
  fetchNotificationConfig,
  patchNotificationConfig,
  type NotificationConfig,
} from '../../../api/admin'
import { useOrg } from '../../../App'

type Provider = NotificationConfig['provider']

export default function NotificationsSection() {
  const qc = useQueryClient()
  const { org } = useOrg()
  const orgId = org?.id

  const configQ = useQuery({
    queryKey: ['admin-notification-config', orgId],
    queryFn: () => fetchNotificationConfig(orgId!),
    enabled: !!orgId,
  })

  const [provider, setProvider] = useState<Provider>('disabled')
  const [fromEmail, setFromEmail] = useState('')
  const [fromName, setFromName] = useState('')
  const [sendgridApiKey, setSendgridApiKey] = useState('')
  const [smtpHost, setSmtpHost] = useState('')
  const [smtpPort, setSmtpPort] = useState<string>('')
  const [smtpUsername, setSmtpUsername] = useState('')
  const [smtpPassword, setSmtpPassword] = useState('')
  const [smtpUseTls, setSmtpUseTls] = useState(true)

  // Sync local edit state from the server snapshot. Secrets stay blank —
  // a blank field on save means "preserve the stored value".
  useEffect(() => {
    if (!configQ.data) return
    const d = configQ.data
    setProvider(d.provider)
    setFromEmail(d.from_email ?? '')
    setFromName(d.from_name ?? '')
    setSmtpHost(d.smtp_host ?? '')
    setSmtpPort(d.smtp_port != null ? String(d.smtp_port) : '')
    setSmtpUsername(d.smtp_username ?? '')
    setSmtpUseTls(d.smtp_use_tls)
  }, [configQ.data])

  const saveMut = useMutation({
    mutationFn: () => patchNotificationConfig(orgId!, {
      provider,
      from_email: provider === 'disabled' ? null : (fromEmail.trim() || null),
      from_name:  provider === 'disabled' ? null : (fromName.trim()  || null),
      sendgrid_api_key: provider === 'sendgrid' && sendgridApiKey ? sendgridApiKey : undefined,
      smtp_host:     provider === 'smtp' ? (smtpHost.trim() || null) : null,
      smtp_port:     provider === 'smtp' && smtpPort ? Number(smtpPort) : null,
      smtp_username: provider === 'smtp' ? (smtpUsername.trim() || null) : null,
      smtp_password: provider === 'smtp' && smtpPassword ? smtpPassword : undefined,
      smtp_use_tls:  smtpUseTls,
    }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['admin-notification-config', orgId] })
      // Clear plaintext secrets from local state after a successful save.
      setSendgridApiKey('')
      setSmtpPassword('')
    },
  })

  if (!orgId) return <div style={{ padding: 8, fontSize: 13 }}>Select an organisation.</div>
  if (configQ.isLoading) return <div style={{ padding: 8 }}><div className="spinner" /></div>
  if (configQ.isError) return <div style={{ color: 'var(--status-error)', fontSize: 13 }}>Failed to load notification config.</div>

  const apiKeySet      = configQ.data?.sendgrid_api_key_set ?? false
  const smtpPasswordSet = configQ.data?.smtp_password_set    ?? false

  return (
    <div style={{ maxWidth: 480 }}>
      <h2 style={{ fontSize: 15, fontWeight: 600, margin: '0 0 6px' }}>Email notifications</h2>
      <p style={{ fontSize: 12, color: 'var(--color-text-muted)', margin: '0 0 20px' }}>
        Configure outbound email used by notification tasks. Actually sending
        mail requires a deployed BPMN flow that emits a notification — this
        page only persists the provider credentials.
      </p>

      <div style={{ display: 'flex', flexDirection: 'column', gap: 10, marginBottom: 24 }}>
        {(['disabled', 'sendgrid', 'smtp'] as const).map(opt => (
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
              name="notif-provider"
              value={opt}
              checked={provider === opt}
              onChange={() => setProvider(opt)}
              style={{ marginTop: 2 }}
            />
            <div>
              <div style={{ fontSize: 13, fontWeight: 500 }}>
                {opt === 'disabled' && 'Disabled'}
                {opt === 'sendgrid' && 'SendGrid'}
                {opt === 'smtp'     && 'SMTP'}
              </div>
              <div style={{ fontSize: 12, color: 'var(--color-text-muted)', marginTop: 2 }}>
                {opt === 'disabled' && 'No outbound email — notification tasks will fail at runtime.'}
                {opt === 'sendgrid' && 'Send via SendGrid using an API key.'}
                {opt === 'smtp'     && 'Send via a generic SMTP relay (host + credentials).'}
              </div>
            </div>
          </label>
        ))}
      </div>

      {provider !== 'disabled' && (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 14, marginBottom: 24 }}>
          <div>
            <label style={labelStyle}>From email</label>
            <input
              type="email"
              placeholder="notifications@example.com"
              value={fromEmail}
              onChange={e => setFromEmail(e.target.value)}
              style={inputStyle}
            />
          </div>
          <div>
            <label style={labelStyle}>From name (optional)</label>
            <input
              type="text"
              placeholder="Conduit"
              value={fromName}
              onChange={e => setFromName(e.target.value)}
              style={inputStyle}
            />
          </div>
        </div>
      )}

      {provider === 'sendgrid' && (
        <div style={{ marginBottom: 24 }}>
          <label style={labelStyle}>
            SendGrid API key
            {apiKeySet && (
              <span style={mutedHintStyle}>(already set — leave blank to keep)</span>
            )}
          </label>
          <input
            type="password"
            placeholder={apiKeySet ? '••••••••' : 'SG....'}
            value={sendgridApiKey}
            onChange={e => setSendgridApiKey(e.target.value)}
            style={inputStyle}
          />
        </div>
      )}

      {provider === 'smtp' && (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 14, marginBottom: 24 }}>
          <div style={{ display: 'flex', gap: 8 }}>
            <div style={{ flex: 2 }}>
              <label style={labelStyle}>SMTP host</label>
              <input
                type="text"
                placeholder="smtp.example.com"
                value={smtpHost}
                onChange={e => setSmtpHost(e.target.value)}
                style={inputStyle}
              />
            </div>
            <div style={{ flex: 1 }}>
              <label style={labelStyle}>Port</label>
              <input
                type="number"
                placeholder="587"
                value={smtpPort}
                onChange={e => setSmtpPort(e.target.value)}
                style={inputStyle}
              />
            </div>
          </div>
          <div>
            <label style={labelStyle}>Username</label>
            <input
              type="text"
              value={smtpUsername}
              onChange={e => setSmtpUsername(e.target.value)}
              style={inputStyle}
            />
          </div>
          <div>
            <label style={labelStyle}>
              Password
              {smtpPasswordSet && (
                <span style={mutedHintStyle}>(already set — leave blank to keep)</span>
              )}
            </label>
            <input
              type="password"
              placeholder={smtpPasswordSet ? '••••••••' : ''}
              value={smtpPassword}
              onChange={e => setSmtpPassword(e.target.value)}
              style={inputStyle}
            />
          </div>
          <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 13 }}>
            <input
              type="checkbox"
              checked={smtpUseTls}
              onChange={e => setSmtpUseTls(e.target.checked)}
            />
            Use STARTTLS
          </label>
        </div>
      )}

      {saveMut.isError && (
        <div style={{ fontSize: 12, color: 'var(--status-error)', marginBottom: 12 }}>
          {(saveMut.error as Error).message}
        </div>
      )}
      {saveMut.isSuccess && (
        <div style={{ fontSize: 12, color: 'var(--status-success)', marginBottom: 12 }}>
          Notification configuration saved.
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

const mutedHintStyle: React.CSSProperties = {
  fontSize: 11,
  color: 'var(--color-text-muted)',
  fontWeight: 400,
  marginLeft: 6,
}
