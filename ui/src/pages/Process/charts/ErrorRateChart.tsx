import { ResponsiveContainer, AreaChart, Area, XAxis, YAxis, Tooltip, CartesianGrid, Legend } from 'recharts'
import type { ThroughputPoint } from './chartUtils'

interface Props {
  data: ThroughputPoint[]
}

export default function ErrorRateChart({ data }: Props) {
  return (
    <ResponsiveContainer width="100%" height={220}>
      <AreaChart data={data} margin={{ top: 8, right: 16, bottom: 0, left: -16 }}>
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
        <Area
          type="monotone"
          dataKey="completed"
          stackId="state"
          name="Completed"
          stroke="var(--status-ok)"
          fill="var(--status-ok-soft)"
        />
        <Area
          type="monotone"
          dataKey="errored"
          stackId="state"
          name="Errored"
          stroke="var(--status-error)"
          fill="var(--status-error-soft)"
        />
        <Area
          type="monotone"
          dataKey="cancelled"
          stackId="state"
          name="Cancelled"
          stroke="var(--text-tertiary)"
          fill="var(--bg-tertiary)"
        />
      </AreaChart>
    </ResponsiveContainer>
  )
}
