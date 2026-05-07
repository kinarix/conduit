type StartVariant = 'plain' | 'message' | 'timer';

export default function StartEventDiagram({ variant = 'plain' }: { variant?: StartVariant }) {
  const color = variant === 'timer' ? '#ea580c' : variant === 'message' ? '#0284c7' : '#16a34a';
  const bg = variant === 'timer' ? '#fff7ed' : variant === 'message' ? '#f0f9ff' : '#f0fdf4';
  const apiNote = variant === 'message' ? 'POST /api/v1/messages' : variant === 'timer' ? 'timer fires' : 'POST /api/v1/instances';

  return (
    <div style={{ margin: '1rem 0', borderRadius: 8, background: '#f8fafc', padding: '10px 12px', border: '1px solid #e2e8f0' }}>
      <svg width={280} height={70} viewBox="0 0 280 70" style={{ display: 'block', overflow: 'visible' }}>
        <text x={52} y={9} textAnchor="middle" fontSize={7.5} fill={color}>{apiNote}</text>
        <line x1={52} y1={11} x2={52} y2={22} stroke={color} strokeWidth={1} strokeDasharray="2 1.5"/>
        <polygon points="49,22 55,22 52,26" fill={color}/>
        <circle cx={52} cy={38} r={14} fill={bg} stroke={color} strokeWidth={1.8}/>
        {variant === 'message' && (
          <>
            <rect x={45} y={33} width={14} height={10} rx={1} fill={bg} stroke={color} strokeWidth={1}/>
            <path d="M45 34 l7 4 7-4" stroke={color} strokeWidth={0.8} fill="none" strokeLinecap="round"/>
          </>
        )}
        {variant === 'timer' && (
          <>
            <circle cx={52} cy={38} r={8} fill="none" stroke={color} strokeWidth={0.9}/>
            <path d="M52 34v4l2.5 1.5" stroke={color} strokeWidth={0.9} strokeLinecap="round"/>
          </>
        )}
        {variant === 'plain' && (
          <circle cx={52} cy={38} r={5} fill={color} opacity={0.3}/>
        )}
        <line x1={66} y1={38} x2={120} y2={38} stroke="#64748b" strokeWidth={1.2}/>
        <polygon points="120,35 120,41 126,38" fill="#64748b"/>
        <rect x={126} y={26} width={80} height={24} rx={4} fill="#ede9fe" stroke="#6366f1" strokeWidth={1.5}/>
        <text x={166} y={42} textAnchor="middle" fontSize={9} fill="#3730a3" fontWeight={500}>First Task</text>
        <line x1={206} y1={38} x2={244} y2={38} stroke="#64748b" strokeWidth={1.2}/>
        <polygon points="244,35 244,41 250,38" fill="#64748b"/>
        <circle cx={262} cy={38} r={11} fill="#fef2f2" stroke="#ef4444" strokeWidth={2.5}/>
      </svg>
    </div>
  );
}
