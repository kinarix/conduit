import { Routes, Route, Navigate, useParams } from 'react-router-dom'
import { useQuery } from '@tanstack/react-query'
import { createContext, useContext, useState } from 'react'
import Layout from './components/Layout'
import Login from './pages/Login'
import Welcome from './pages/Welcome'
import OrgDashboard from './pages/OrgDashboard'
import ProcessGroupDashboard from './pages/ProcessGroupDashboard'
import Modeller from './pages/Modeller'
import ProcessDashboard from './pages/Process/ProcessDashboard'
import InstancesList from './pages/InstancesList'
import InstanceDetail from './pages/Instance/InstanceDetail'
import TaskList from './pages/TaskList'
import Secrets from './pages/Secrets'
import Decisions from './pages/Decisions'
import DecisionTableEditor from './pages/DecisionTableEditor'
import { fetchDeployment } from './api/deployments'
import { useAuth } from './context/AuthContext'
import AdminShell from './pages/admin/AdminShell'
import AdminUsers from './pages/admin/AdminUsers'
import AdminRoles from './pages/admin/AdminRoles'
import AdminAuth from './pages/admin/AdminAuth'
import AdminSettings from './pages/admin/AdminSettings'

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

function DefinitionRedirect() {
  const { id = '' } = useParams<{ id: string }>()
  const { org } = useOrg()
  const { data, isLoading, error } = useQuery({
    queryKey: ['deployment', org?.id, id],
    queryFn: () => fetchDeployment(org!.id, id),
    enabled: !!id && !!org,
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

function RequireAuth({ children }: { children: React.ReactNode }) {
  const { isAuthenticated, isLoading } = useAuth()
  if (isLoading) {
    return (
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100vh' }}>
        <div className="spinner" />
      </div>
    )
  }
  if (!isAuthenticated) return <Navigate to="/login" replace />
  return <>{children}</>
}

function RequirePerm({ anyOf, children }: { anyOf: string[]; children: React.ReactNode }) {
  const { user } = useAuth()
  // Global admins bypass UI permission gates — server-side authorisation
  // still applies on every request.
  if (user?.is_global_admin) return <>{children}</>
  const perms = new Set(user?.global_permissions ?? [])
  const ok = anyOf.some(p => perms.has(p))
  if (!ok) return <Navigate to="/" replace />
  return <>{children}</>
}

export default function App() {
  const [org, setOrg] = useState<Org | null>(null)

  return (
    <OrgContext.Provider value={{ org, setOrg }}>
      <Routes>
        <Route path="/login" element={<Login />} />
        <Route
          path="/"
          element={
            <RequireAuth>
              <Layout />
            </RequireAuth>
          }
        >
          <Route index element={<OrgDashboard />} />
          <Route path="definitions" element={<Navigate to="/" replace />} />
          <Route path="definitions/:id" element={<DefinitionRedirect />} />
          <Route path="definitions/:id/edit" element={<Modeller />} />
          <Route path="process-groups/:groupId" element={<ProcessGroupDashboard />} />
          <Route path="process-groups/:groupId/definitions/new" element={<Modeller />} />
          <Route
            path="groups/:groupId/processes/:processKey"
            element={<ProcessDashboard />}
          />
          <Route path="instances" element={<InstancesList />} />
          <Route path="instances/:instanceId" element={<InstanceDetail />} />
          <Route path="tasks" element={<TaskList />} />
          <Route
            path="secrets"
            element={<RequirePerm anyOf={['secret.create', 'secret.read_metadata', 'secret.update', 'secret.delete']}><Secrets /></RequirePerm>}
          />
          <Route path="decisions" element={<Decisions />} />
          <Route path="decisions/new" element={<DecisionTableEditor />} />
          <Route path="decisions/:key/edit" element={<DecisionTableEditor />} />
          <Route path="process-groups/:groupId/decisions" element={<Decisions />} />
          <Route path="process-groups/:groupId/decisions/new" element={<DecisionTableEditor />} />
          <Route path="process-groups/:groupId/decisions/:key/edit" element={<DecisionTableEditor />} />
          <Route path="welcome" element={<Welcome />} />
          <Route
            path="admin"
            element={
              <RequirePerm anyOf={[
                'org.read', 'org.update',
                'user.read', 'user.create',
                'role.read', 'role.create',
                'role_assignment.read', 'role_assignment.create',
                'auth_config.read', 'auth_config.update',
              ]}>
                <AdminShell />
              </RequirePerm>
            }
          >
            <Route index element={<Navigate to="users" replace />} />
            <Route
              path="users"
              element={<RequirePerm anyOf={['user.read', 'user.create', 'role_assignment.read']}><AdminUsers /></RequirePerm>}
            />
            <Route
              path="roles"
              element={<RequirePerm anyOf={['role.read', 'role.create']}><AdminRoles /></RequirePerm>}
            />
            <Route
              path="auth"
              element={<RequirePerm anyOf={['auth_config.read', 'auth_config.update']}><AdminAuth /></RequirePerm>}
            />
            <Route
              path="settings"
              element={<RequirePerm anyOf={['org.read', 'org.update']}><AdminSettings /></RequirePerm>}
            />
          </Route>
        </Route>
      </Routes>
    </OrgContext.Provider>
  )
}
