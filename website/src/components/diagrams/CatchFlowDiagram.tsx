type Variant = 'timer' | 'message' | 'signal';

const configs: Record<Variant, {
  color: string;
  apiLabel: string;
  icon: (cx: number, cy: number) => JSX.Element;
  height: number;
  viewBox: string;
  eventY: number;
  startY: number;
}> = {
  timer: {
    color: '#ea580c',
    apiLabel: '',
    icon: (cx, cy) => (
      <>
        <circle cx={cx} cy={cy} r={8} fill="none" stroke="#ea580c" strokeWidth={0.9}/>
        <path d={`M${cx} ${cy-3}v3l2 1.2`} stroke="#ea580c" strokeWidth={0.9} strokeLinecap="round"/>
      </>
    ),
    height: 56,
    viewBox: '0 0 210 56',
    eventY: 24,
    startY: 24,
  },
  message: {
    color: '#0284c7',
    apiLabel: 'POST /api/v1/messages',
    icon: (cx, cy) => (
      <>
        <circle cx={cx} cy={cy} r={8} fill="none" stroke="#0284c7" strokeWidth={0.9}/>
        <rect x={cx-5} y={cy-3} width={10} height={7} rx={1} fill="#f0f9ff" stroke="#0284c7" strokeWidth={0.8}/>
        <path d={`M${cx-5} ${cy-2}l5 3 5-3`} stroke="#0284c7" strokeWidth={0.7} fill="none" strokeLinecap="round"/>
      </>
    ),
    height: 72,
    viewBox: '0 0 210 72',
    eventY: 42,
    startY: 42,
  },
  signal: {
    color: '#7c3aed',
    apiLabel: 'POST /api/v1/signals/broadcast',
    icon: (cx, cy) => (
      <>
        <circle cx={cx} cy={cy} r={8} fill="none" stroke="#7c3aed" strokeWidth={0.9}/>
        <polygon points={`${cx},${cy-5} ${cx+5},${cy+3} ${cx-5},${cy+3}`} fill="#7c3aed" opacity={0.7}/>
      </>
    ),
    height: 72,
    viewBox: '0 0 210 72',
    eventY: 42,
    startY: 42,
  },
};

const eventFill: Record<Variant, string> = {
  timer: '#fff7ed',
  message: '#f0f9ff',
  signal: '#faf5ff',
};

const eventStroke: Record<Variant, string> = {
  timer: '#ea580c',
  message: '#0284c7',
  signal: '#7c3aed',
};

export default function CatchFlowDiagram({ variant }: { variant: Variant }) {
  const cfg = configs[variant];
  const ey = cfg.eventY;
  const sy = cfg.startY;

  return (
    <div style={{ margin: '1rem 0', borderRadius: 8, background: '#f8fafc', padding: '10px 12px', border: '1px solid #e2e8f0' }}>
      <svg width={210} height={cfg.height} viewBox={cfg.viewBox} style={{ display: 'block', overflow: 'visible' }}>
        {cfg.apiLabel && (
          <>
            <text x={84} y={9} textAnchor="middle" fontSize={7} fill={cfg.color}>{cfg.apiLabel}</text>
            <line x1={84} y1={11} x2={84} y2={20} stroke={cfg.color} strokeWidth={1} strokeDasharray="2 1.5"/>
            <polygon points="81,20 87,20 84,24" fill={cfg.color}/>
          </>
        )}
        <circle cx={16} cy={sy} r={10} fill="#f0fdf4" stroke="#16a34a" strokeWidth={1.4}/>
        <line x1={26} y1={ey} x2={66} y2={ey} stroke="#64748b" strokeWidth={1.2}/>
        <polygon points={`66,${ey-3} 66,${ey+3} 72,${ey}`} fill="#64748b"/>
        <circle cx={84} cy={ey} r={12} fill={eventFill[variant]} stroke={eventStroke[variant]} strokeWidth={1.4}/>
        <circle cx={84} cy={ey} r={7.5} fill="none" stroke={eventStroke[variant]} strokeWidth={0.9}/>
        {cfg.icon(84, ey)}
        <text x={84} y={cfg.height - 4} textAnchor="middle" fontSize={7} fill="#94a3b8">token waits here</text>
        <line x1={96} y1={ey} x2={140} y2={ey} stroke="#64748b" strokeWidth={1.2}/>
        <polygon points={`140,${ey-3} 140,${ey+3} 146,${ey}`} fill="#64748b"/>
        <rect x={146} y={ey - 9} width={56} height={18} rx={3} fill="#ede9fe" stroke="#6366f1" strokeWidth={1.4}/>
        <text x={174} y={ey + 4} textAnchor="middle" fontSize={9} fill="#3730a3">Continue</text>
      </svg>
    </div>
  );
}
