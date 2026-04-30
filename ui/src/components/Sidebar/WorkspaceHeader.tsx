import { useState } from 'react'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { createOrg } from '../../api/orgs'
import { useOrg } from '../../App'
import { PlusIcon } from './SidebarIcons'
import styles from './Sidebar.module.css'

export default function WorkspaceHeader() {
  const qc = useQueryClient()
  const { setOrg } = useOrg()
  const [open, setOpen] = useState(false)
  const [name, setName] = useState('')
  const [slug, setSlug] = useState('')

  const createMut = useMutation({
    mutationFn: createOrg,
    onSuccess: created => {
      qc.invalidateQueries({ queryKey: ['orgs'] })
      setOrg(created)
      setOpen(false)
      setName('')
      setSlug('')
    },
  })

  const slugify = (v: string) =>
    v.toLowerCase().replace(/\s+/g, '-').replace(/[^a-z0-9-]/g, '')

  return (
    <>
      <header className={styles.header}>
        <div className={styles.brand}>Conduit</div>
        <button
          type="button"
          className={styles.iconBtn}
          title="New organisation"
          onClick={() => setOpen(true)}
        >
          <PlusIcon size={13} />
        </button>
      </header>

      {open && (
        <div className="modal-overlay" onClick={() => setOpen(false)}>
          <div className="modal" onClick={e => e.stopPropagation()}>
            <h3>Create organisation</h3>
            <div className="field">
              <label>Name</label>
              <input
                autoFocus
                value={name}
                onChange={e => {
                  setName(e.target.value)
                  setSlug(slugify(e.target.value))
                }}
              />
            </div>
            <div className="field">
              <label>Slug</label>
              <input value={slug} onChange={e => setSlug(slugify(e.target.value))} />
            </div>
            {createMut.error && <div className="error-banner">{String(createMut.error)}</div>}
            <div className="modal-actions">
              <button className="btn-ghost" onClick={() => setOpen(false)}>Cancel</button>
              <button
                className="btn-primary"
                disabled={!name || !slug || createMut.isPending}
                onClick={() => createMut.mutate({ name, slug })}
              >
                {createMut.isPending ? 'Creating…' : 'Create'}
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  )
}
