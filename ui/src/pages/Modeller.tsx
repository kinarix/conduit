import { useRef, useState, useEffect } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import { useQuery, useMutation } from '@tanstack/react-query'
import BpmnEditor, { BpmnEditorHandle } from '../components/bpmn/BpmnEditor'
import { fetchDeployment, deployProcess } from '../api/deployments'
import { useOrg } from '../App'

export default function Modeller() {
  const { id } = useParams<{ id: string }>()
  const navigate = useNavigate()
  const { org } = useOrg()
  const modRef = useRef<BpmnEditorHandle>(null)
  const [key, setKey] = useState('')
  const [name, setName] = useState('')
  const [error, setError] = useState('')
  const [showDeploy, setShowDeploy] = useState(false)

  const { data: existing } = useQuery({
    queryKey: ['deployment', id],
    queryFn: () => fetchDeployment(id!),
    enabled: !!id,
  })

  useEffect(() => {
    if (existing) {
      setKey(existing.key)
      setName(existing.name)
    }
  }, [existing])

  const deployMut = useMutation({
    mutationFn: async () => {
      const bpmn_xml = await modRef.current!.getXml()
      return deployProcess({ org_id: org!.id, key, name, bpmn_xml })
    },
    onSuccess: () => navigate('/definitions'),
    onError: (e: Error) => setError(e.message),
  })

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: 'calc(100vh - 48px)' }}>
      <div style={{
        display: 'flex',
        justifyContent: 'space-between',
        alignItems: 'center',
        marginBottom: 16,
        flexShrink: 0,
      }}>
        <h1 style={{ fontSize: 18, fontWeight: 600 }}>
          {id ? 'Edit Process' : 'New Process'}
        </h1>
        <div style={{ display: 'flex', gap: 8 }}>
          <button className="btn-ghost" onClick={() => navigate('/definitions')}>
            Cancel
          </button>
          <button className="btn-primary" onClick={() => { setError(''); setShowDeploy(true) }}>
            Deploy
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
        <BpmnEditor ref={modRef} xml={existing?.bpmn_xml} />
      </div>

      {showDeploy && (
        <div className="modal-overlay" onClick={() => setShowDeploy(false)}>
          <div className="modal" onClick={e => e.stopPropagation()}>
            <h3>Deploy process</h3>
            <div className="field">
              <label>Process key (unique identifier)</label>
              <input
                value={key}
                autoFocus
                onChange={e => setKey(e.target.value)}
                placeholder="my-process"
              />
            </div>
            <div className="field">
              <label>Display name</label>
              <input
                value={name}
                onChange={e => setName(e.target.value)}
                placeholder="My Process"
              />
            </div>
            {error && <div className="error-banner">{error}</div>}
            <div className="modal-actions">
              <button className="btn-ghost" onClick={() => setShowDeploy(false)}>Cancel</button>
              <button
                className="btn-primary"
                disabled={!key || !name || deployMut.isPending}
                onClick={() => deployMut.mutate()}
              >
                {deployMut.isPending ? 'Deploying…' : 'Deploy'}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
