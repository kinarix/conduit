import { Outlet } from 'react-router-dom'
import { useQuery } from '@tanstack/react-query'
import { fetchOrgs } from '../api/orgs'
import Sidebar from './Sidebar/Sidebar'

export default function Layout() {
  const { data: orgs = [], isLoading } = useQuery({ queryKey: ['orgs'], queryFn: fetchOrgs })

  return (
    <div style={{ display: 'flex', height: '100vh', overflow: 'hidden' }}>
      <Sidebar />

      <main style={{ flex: 1, overflow: 'auto' }}>
        {isLoading ? (
          <CenterPanel><div className="spinner" /></CenterPanel>
        ) : orgs.length === 0 ? (
          <CenterPanel>
            <div style={{ textAlign: 'center', maxWidth: 360, color: 'var(--text-secondary)' }}>
              <div style={{ fontSize: 32, opacity: 0.25, marginBottom: 12 }}>⬡</div>
              <h2 style={{ fontSize: 17, fontWeight: 600, color: 'var(--text-primary)', margin: 0 }}>
                Add an organisation to start
              </h2>
              <p style={{ fontSize: 13, lineHeight: 1.6, marginTop: 8 }}>
                Organisations are the top-level containers for your processes. Create one from
                the sidebar, then add process groups and deploy your first BPMN process.
              </p>
            </div>
          </CenterPanel>
        ) : (
          <Outlet />
        )}
      </main>
    </div>
  )
}

function CenterPanel({ children }: { children: React.ReactNode }) {
  return (
    <div style={{
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      height: '100%',
      padding: 24,
    }}>
      {children}
    </div>
  )
}
