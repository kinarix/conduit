import { useCallback, useEffect, useState } from 'react'

/**
 * `Set<string>` of expanded ids, persisted to localStorage so the tree state
 * survives reloads. Each consumer (orgs, groups, …) uses its own key.
 */
export function useExpansion(storageKey: string) {
  const [expanded, setExpanded] = useState<Set<string>>(() => {
    try {
      const raw = localStorage.getItem(storageKey)
      if (!raw) return new Set()
      const arr = JSON.parse(raw)
      if (!Array.isArray(arr)) return new Set()
      return new Set(arr.filter((v): v is string => typeof v === 'string'))
    } catch {
      return new Set()
    }
  })

  useEffect(() => {
    try {
      localStorage.setItem(storageKey, JSON.stringify([...expanded]))
    } catch {
      // ignore quota / disabled storage
    }
  }, [storageKey, expanded])

  useEffect(() => {
    const handler = (e: Event) => {
      const { detail } = e as CustomEvent<string>
      if (detail !== storageKey) return
      try {
        const raw = localStorage.getItem(storageKey)
        if (!raw) return
        const arr = JSON.parse(raw)
        if (!Array.isArray(arr)) return
        setExpanded(new Set(arr.filter((v): v is string => typeof v === 'string')))
      } catch {}
    }
    window.addEventListener('sidebar:expansion-sync', handler)
    return () => window.removeEventListener('sidebar:expansion-sync', handler)
  }, [storageKey])

  const toggle = useCallback((id: string) => {
    setExpanded(prev => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }, [])

  const expand = useCallback((id: string) => {
    setExpanded(prev => (prev.has(id) ? prev : new Set([...prev, id])))
  }, [])

  const collapse = useCallback((id: string) => {
    setExpanded(prev => {
      if (!prev.has(id)) return prev
      const next = new Set(prev)
      next.delete(id)
      return next
    })
  }, [])

  return { expanded, toggle, expand, collapse }
}
