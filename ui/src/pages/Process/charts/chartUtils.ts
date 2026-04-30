import type { ProcessInstance } from '../../../api/instances'

export type Bucket = 'hour' | 'day'

/** Truncate `date` to the start of its bucket. */
export function bucketStart(d: Date, bucket: Bucket): Date {
  const out = new Date(d)
  out.setMinutes(0, 0, 0)
  if (bucket === 'day') out.setHours(0)
  return out
}

export function bucketStep(bucket: Bucket): number {
  return bucket === 'hour' ? 60 * 60 * 1000 : 24 * 60 * 60 * 1000
}

/**
 * Bucket instances by `started_at`. Returns dense series (zero-filled gaps)
 * across the [from, to] range.
 */
export interface ThroughputPoint {
  timestamp: number // ms
  bucketLabel: string
  total: number
  completed: number
  errored: number
  cancelled: number
}

export function bucketThroughput(
  instances: ProcessInstance[],
  bucket: Bucket,
  windowMs: number,
): ThroughputPoint[] {
  const now = Date.now()
  const from = bucketStart(new Date(now - windowMs), bucket)
  const step = bucketStep(bucket)

  const buckets = new Map<number, ThroughputPoint>()
  for (let t = from.getTime(); t <= now; t += step) {
    buckets.set(t, {
      timestamp: t,
      bucketLabel: formatBucketLabel(t, bucket),
      total: 0,
      completed: 0,
      errored: 0,
      cancelled: 0,
    })
  }

  for (const inst of instances) {
    const started = new Date(inst.started_at).getTime()
    if (started < from.getTime() || started > now) continue
    const key = bucketStart(new Date(started), bucket).getTime()
    const point = buckets.get(key)
    if (!point) continue
    point.total += 1
    if (inst.state === 'completed') point.completed += 1
    else if (inst.state === 'error' || (inst.state as string) === 'failed') point.errored += 1
    else if (inst.state === 'cancelled') point.cancelled += 1
  }

  return [...buckets.values()].sort((a, b) => a.timestamp - b.timestamp)
}

function formatBucketLabel(ts: number, bucket: Bucket): string {
  const d = new Date(ts)
  if (bucket === 'hour') {
    return `${d.getMonth() + 1}/${d.getDate()} ${String(d.getHours()).padStart(2, '0')}:00`
  }
  return `${d.getMonth() + 1}/${d.getDate()}`
}

/** Elapsed-time percentiles per bucket. */
export interface ElapsedPoint {
  timestamp: number
  bucketLabel: string
  p50: number | null
  p95: number | null
  p99: number | null
  count: number
}

export function bucketElapsed(
  instances: ProcessInstance[],
  bucket: Bucket,
  windowMs: number,
): ElapsedPoint[] {
  const now = Date.now()
  const from = bucketStart(new Date(now - windowMs), bucket)
  const step = bucketStep(bucket)

  const lists = new Map<number, number[]>()
  for (let t = from.getTime(); t <= now; t += step) lists.set(t, [])

  for (const inst of instances) {
    if (!inst.ended_at) continue
    const started = new Date(inst.started_at).getTime()
    const ended = new Date(inst.ended_at).getTime()
    if (started < from.getTime()) continue
    const key = bucketStart(new Date(started), bucket).getTime()
    const arr = lists.get(key)
    if (arr) arr.push((ended - started) / 1000) // seconds
  }

  return [...lists.entries()]
    .sort(([a], [b]) => a - b)
    .map(([ts, arr]) => ({
      timestamp: ts,
      bucketLabel: formatBucketLabel(ts, bucket),
      p50: percentile(arr, 0.5),
      p95: percentile(arr, 0.95),
      p99: percentile(arr, 0.99),
      count: arr.length,
    }))
}

function percentile(values: number[], q: number): number | null {
  if (values.length === 0) return null
  const sorted = [...values].sort((a, b) => a - b)
  const idx = Math.min(sorted.length - 1, Math.max(0, Math.round(q * (sorted.length - 1))))
  return sorted[idx]
}

export function formatDurationSec(s: number | null): string {
  if (s == null) return '—'
  if (s < 1) return `${Math.round(s * 1000)}ms`
  if (s < 60) return `${s.toFixed(1)}s`
  const m = Math.floor(s / 60)
  const r = Math.round(s - m * 60)
  return r ? `${m}m ${r}s` : `${m}m`
}

/** Pick a sensible bucket given a window length. */
export function chooseBucket(windowMs: number): Bucket {
  return windowMs <= 36 * 60 * 60 * 1000 ? 'hour' : 'day'
}
