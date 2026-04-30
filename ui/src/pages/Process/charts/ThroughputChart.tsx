import { ResponsiveContainer, LineChart, Line, XAxis, YAxis, Tooltip, CartesianGrid, Legend } from 'recharts'
import type { ThroughputPoint } from './chartUtils'

interface Props {
  data: ThroughputPoint[]
}

export default function ThroughputChart({ data }: Props) {
  return (
    <ResponsiveContainer width="100%" height={220}>
      <LineChart data={data} margin={{ top: 8, right: 16, bottom: 0, left: -16 }}>
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
          allowDecimals={false}
        />
        <Tooltip
          contentStyle={{
            background: 'var(--bg-secondary)',
            border: '1px solid var(--border-primary)',
            borderRadius: 'var(--radius-sm)',
            fontSize: 12,
          }}
        />
        <Legend wrapperStyle={{ fontSize: 11 }} />
        <Line type="monotone" dataKey="total" name="Started" stroke="var(--accent)" strokeWidth={2} dot={false} />
        <Line type="monotone" dataKey="completed" name="Completed" stroke="var(--status-ok)" strokeWidth={1.5} dot={false} />
        <Line type="monotone" dataKey="errored" name="Errored" stroke="var(--status-error)" strokeWidth={1.5} dot={false} />
      </LineChart>
    </ResponsiveContainer>
  )
}
