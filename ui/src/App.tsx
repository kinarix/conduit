import { Routes, Route, Navigate } from 'react-router-dom'
import { createContext, useContext, useState } from 'react'
import Layout from './components/Layout'
import DefinitionsList from './pages/DefinitionsList'
import Modeller from './pages/Modeller'
import InstancesList from './pages/InstancesList'
import TaskList from './pages/TaskList'

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

export default function App() {
  const [org, setOrgState] = useState<Org | null>(() => {
    try {
      const saved = localStorage.getItem('conduit_org')
      return saved ? JSON.parse(saved) : null
    } catch {
      return null
    }
  })

  const setOrg = (o: Org | null) => {
    setOrgState(o)
    if (o) localStorage.setItem('conduit_org', JSON.stringify(o))
    else localStorage.removeItem('conduit_org')
  }

  return (
    <OrgContext.Provider value={{ org, setOrg }}>
      <Routes>
        <Route path="/" element={<Layout />}>
          <Route index element={<Navigate to="/definitions" replace />} />
          <Route path="definitions" element={<DefinitionsList />} />
          <Route path="definitions/new" element={<Modeller />} />
          <Route path="definitions/:id" element={<Modeller />} />
          <Route path="instances" element={<InstancesList />} />
          <Route path="tasks" element={<TaskList />} />
        </Route>
      </Routes>
    </OrgContext.Provider>
  )
}
