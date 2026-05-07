export default function BoundarySignalDiagram() {
  return (
    <div style={{ margin: '1rem 0', borderRadius: 8, background: '#f8fafc', padding: '10px 12px', border: '1px solid #e2e8f0' }}>
      <div style={{ display: 'flex', gap: 16, marginBottom: 8, fontSize: 11, color: '#64748b', flexWrap: 'wrap' }}>
        <span style={{ display: 'flex', alignItems: 'center', gap: 5 }}>
          <span style={{ width: 10, height: 10, borderRadius: 2, background: '#f59e0b', display: 'inline-block' }} />
          bottom port — attach to host task
        </span>
        <span style={{ display: 'flex', alignItems: 'center', gap: 5 }}>
          <span style={{ width: 10, height: 10, borderRadius: '50%', background: '#6366f1', display: 'inline-block' }} />
          right port — path when signal fires
        </span>
      </div>
      <svg width={220} height={96} viewBox="0 0 220 96" style={{ display: 'block', overflow: 'visible' }}>
        <rect x={4} y={6} width={68} height={26} rx={3} fill="#ede9fe" stroke="#6366f1" strokeWidth={1.4}/>
        <text x={38} y={23} textAnchor="middle" fontSize={9} fill="#3730a3" fontWeight={500}>Service Task</text>
        <text x={90} y={14} textAnchor="middle" fontSize={7} fill="#94a3b8">normal</text>
        <line x1={72} y1={19} x2={116} y2={19} stroke="#64748b" strokeWidth={1.2}/>
        <polygon points="116,16 116,22 122,19" fill="#64748b"/>
        <circle cx={132} cy={19} r={7} fill="#fef2f2" stroke="#ef4444" strokeWidth={2.5}/>
        <rect x={34} y={32} width={7} height={7} rx={1.5} fill="#f59e0b"/>
        <line x1={37} y1={39} x2={37} y2={62} stroke="#94a3b8" strokeWidth={1.2} strokeDasharray="3 2"/>
        <circle cx={37} cy={74} r={12} fill="#faf5ff" stroke="#7c3aed" strokeWidth={1.4} strokeDasharray="3 2"/>
        <circle cx={37} cy={74} r={7} fill="none" stroke="#7c3aed" strokeWidth={0.9}/>
        <polygon points="37,69 42,77 32,77" fill="#7c3aed" opacity={0.7}/>
        <circle cx={49} cy={74} r={3.5} fill="#6366f1"/>
        <text x={74} y={70} textAnchor="middle" fontSize={7} fill="#94a3b8">on signal</text>
        <line x1={53} y1={74} x2={116} y2={74} stroke="#64748b" strokeWidth={1.2}/>
        <polygon points="116,71 116,77 122,74" fill="#64748b"/>
        <rect x={122} y={67} width={56} height={14} rx={2} fill="#faf5ff" stroke="#7c3aed" strokeWidth={1.2}/>
        <text x={150} y={77} textAnchor="middle" fontSize={8} fill="#6d28d9">Handler</text>
      </svg>
    </div>
  );
}
