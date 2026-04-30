import { ResponsiveContainer, LineChart, Line, XAxis, YAxis, Tooltip, CartesianGrid, Legend } from 'recharts'
import { formatDurationSec, type ElapsedPoint } from './chartUtils'

interface Props {
  data: ElapsedPoint[]
}

export default function ElapsedTimeChart({ data }: Props) {
  return (
    <ResponsiveContainer width="100%" height={220}>
      <LineChart data={data} margin={{ top: 8, right: 16, bottom: 0, left: 0 }}>
        <CartesianGrid stroke="var(--border-primary)" strokeDasharray="3 3" />
        <XAxis
          dataKey="bucketLabel"
          stroke="var(--text-tertiary)"
          fontSize={10}
          tickLine={false}
          interval="preserveStartEnd"
        />
        <YAxis
          stroke="var(--text-tertiary)"
          fontSize={10}
          tickLine={false}
          tickFormatter={(v: number) => formatDurationSec(v)}
        />
        <Tooltip
          contentStyle={{
            background: 'var(--bg-secondary)',
            border: '1px solid var(--border-primary)',
            borderRadius: 'var(--radius-sm)',
            fontSize: 12,
          }}
          formatter={(v: unknown) =>
            typeof v === 'number' ? formatDurationSec(v) : String(v ?? '—')
          }
        />
        <Legend wrapperStyle={{ fontSize: 11 }} />
        <Line type="monotone" dataKey="p50" name="P50" stroke="var(--accent)" strokeWidth={2} dot={false} connectNulls />
        <Line type="monotone" dataKey="p95" name="P95" stroke="var(--status-warn)" strokeWidth={1.5} dot={false} connectNulls />
        <Line type="monotone" dataKey="p99" name="P99" stroke="var(--status-error)" strokeWidth={1.5} dot={false} connectNulls />
      </LineChart>
    </ResponsiveContainer>
  )
}
