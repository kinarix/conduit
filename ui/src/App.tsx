import { Routes, Route, Navigate, useParams } from 'react-router-dom'
import { useQuery } from '@tanstack/react-query'
import { createContext, useContext, useState } from 'react'
import Layout from './components/Layout'
import Welcome from './pages/Welcome'
import DefinitionsList from './pages/DefinitionsList'
import Modeller from './pages/Modeller'
import ProcessDashboard from './pages/Process/ProcessDashboard'
import InstancesList from './pages/InstancesList'
import InstanceDetail from './pages/Instance/InstanceDetail'
import TaskList from './pages/TaskList'
import Secrets from './pages/Secrets'
import Decisions from './pages/Decisions'
import DecisionTableEditor from './pages/DecisionTableEditor'
import { fetchDeployment } from './api/deployments'

export interface Org {
  id: string
  name: string
  slug: string
  created_at: string
}

interface OrgContextValue {
  org: Org | null
  setOrg: (org: Org | null) => void
}

export const OrgContext = createContext<OrgContextValue>({ org: null, setOrg: () => {} })
export const useOrg = () => useContext(OrgContext)

function HomeIndex() {
  return (
    <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100%', color: 'var(--color-text-muted)', fontSize: 14 }}>
      Select a process from the sidebar to get started.
    </div>
  )
}

function DefinitionRedirect() {
  const { id = '' } = useParams<{ id: string }>()
  const { data, isLoading, error } = useQuery({
    queryKey: ['deployment', id],
    queryFn: () => fetchDeployment(id),
    enabled: !!id,
  })
  if (isLoading) return <div style={{ padding: 24 }}>Loading…</div>
  if (error || !data) return <Navigate to="/" replace />
  return (
    <Navigate
      to={`/groups/${data.process_group_id}/processes/${encodeURIComponent(data.process_key)}`}
      replace
    />
  )
}

export default function App() {
  const [org, setOrg] = useState<Org | null>(null)

  return (
    <OrgContext.Provider value={{ org, setOrg }}>
      <Routes>
        <Route path="/" element={<Layout />}>
          <Route index element={<HomeIndex />} />
          <Route path="definitions" element={<Navigate to="/" replace />} />
          {/* Legacy /definitions/:id resolves to the new Process Dashboard. */}
          <Route path="definitions/:id" element={<DefinitionRedirect />} />
          <Route path="definitions/:id/edit" element={<Modeller />} />
          <Route path="process-groups/:groupId" element={<DefinitionsList />} />
          <Route path="process-groups/:groupId/definitions/new" element={<Modeller />} />
          <Route
            path="groups/:groupId/processes/:processKey"
            element={<ProcessDashboard />}
          />
          <Route path="instances" element={<InstancesList />} />
          <Route path="instances/:instanceId" element={<InstanceDetail />} />
          <Route path="tasks" element={<TaskList />} />
          <Route path="secrets" element={<Secrets />} />
          <Route path="decisions" element={<Decisions />} />
          <Route path="decisions/new" element={<DecisionTableEditor />} />
          <Route path="decisions/:key/edit" element={<DecisionTableEditor />} />
          <Route path="process-groups/:groupId/decisions" element={<Decisions />} />
          <Route path="process-groups/:groupId/decisions/new" element={<DecisionTableEditor />} />
          <Route path="process-groups/:groupId/decisions/:key/edit" element={<DecisionTableEditor />} />
        </Route>
      </Routes>
    </OrgContext.Provider>
  )
}
