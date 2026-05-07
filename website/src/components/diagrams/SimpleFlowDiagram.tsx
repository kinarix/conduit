type ElementShape = 'task' | 'subprocess' | 'gateway-xor' | 'gateway-par' | 'gateway-inc';

interface Props {
  label: string;
  shape?: ElementShape;
  fill?: string;
  stroke?: string;
  icon?: 'user' | 'gear' | 'script' | 'rule' | 'send' | 'receive' | 'sub';
  apiNote?: string;
}

function TaskRect({ cx, cy, w, h, fill, stroke, label, icon }: {
  cx: number; cy: number; w: number; h: number;
  fill: string; stroke: string; label: string;
  icon?: Props['icon'];
}) {
  const x = cx - w / 2;
  const y = cy - h / 2;
  return (
    <>
      <rect x={x} y={y} width={w} height={h} rx={4} fill={fill} stroke={stroke} strokeWidth={1.5}/>
      {icon === 'user' && (
        <>
          <circle cx={x + 10} cy={y + 8} r={3} fill={stroke} opacity={0.6}/>
          <path d={`M${x+5} ${y+17} q5-5 10 0`} stroke={stroke} strokeWidth={1} fill="none" opacity={0.6}/>
        </>
      )}
      {icon === 'gear' && (
        <circle cx={x + 10} cy={y + 10} r={4} fill="none" stroke={stroke} strokeWidth={1} opacity={0.6}/>
      )}
      {icon === 'script' && (
        <>
          <line x1={x+7} y1={y+7} x2={x+15} y2={y+7} stroke={stroke} strokeWidth={0.9} opacity={0.6}/>
          <line x1={x+7} y1={y+10} x2={x+13} y2={y+10} stroke={stroke} strokeWidth={0.9} opacity={0.6}/>
          <line x1={x+7} y1={y+13} x2={x+14} y2={y+13} stroke={stroke} strokeWidth={0.9} opacity={0.6}/>
        </>
      )}
      {icon === 'rule' && (
        <>
          <rect x={x+6} y={y+6} width={10} height={8} rx={1} fill="none" stroke={stroke} strokeWidth={0.9} opacity={0.6}/>
          <line x1={x+6} y1={y+9} x2={x+16} y2={y+9} stroke={stroke} strokeWidth={0.7} opacity={0.6}/>
        </>
      )}
      {icon === 'send' && (
        <polygon points={`${x+6},${y+7} ${x+16},${y+12} ${x+6},${y+17}`} fill={stroke} opacity={0.5}/>
      )}
      {icon === 'receive' && (
        <>
          <rect x={x+6} y={y+7} width={10} height={8} rx={1} fill="none" stroke={stroke} strokeWidth={0.9} opacity={0.6}/>
          <path d={`M${x+6} ${y+8} l5 3 5-3`} stroke={stroke} strokeWidth={0.7} fill="none" opacity={0.6}/>
        </>
      )}
      {icon === 'sub' && (
        <>
          <rect x={x+6} y={y+12} width={10} height={6} rx={1} fill="none" stroke={stroke} strokeWidth={0.8} opacity={0.5}/>
          <text x={cx} y={y+17} textAnchor="middle" fontSize={6} fill={stroke} opacity={0.5}>+</text>
        </>
      )}
      <text x={cx} y={cy + 5} textAnchor="middle" fontSize={9} fill={stroke.replace('f1', 'a3').replace('#', '#3730')} fontWeight={500}
        style={{ fontFamily: 'sans-serif' }}>
        {label}
      </text>
    </>
  );
}

export default function SimpleFlowDiagram({ label, shape = 'task', fill = '#ede9fe', stroke = '#6366f1', icon, apiNote }: Props) {
  const totalH = apiNote ? 100 : 72;
  const cy = apiNote ? 52 : 32;

  return (
    <div style={{ margin: '1rem 0', borderRadius: 8, background: '#f8fafc', padding: '10px 12px', border: '1px solid #e2e8f0' }}>
      <svg width={300} height={totalH} viewBox={`0 0 300 ${totalH}`} style={{ display: 'block', overflow: 'visible' }}>
        {/* Start */}
        <circle cx={24} cy={cy} r={11} fill="#f0fdf4" stroke="#16a34a" strokeWidth={1.5}/>
        <line x1={35} y1={cy} x2={68} y2={cy} stroke="#64748b" strokeWidth={1.2}/>
        <polygon points={`68,${cy-3} 68,${cy+3} 74,${cy}`} fill="#64748b"/>

        {/* Element */}
        {shape === 'task' || shape === 'subprocess' ? (
          <TaskRect cx={140} cy={cy} w={104} h={28} fill={fill} stroke={stroke} label={label} icon={icon}/>
        ) : shape === 'gateway-xor' ? (
          <>
            <polygon points={`140,${cy-16} 156,${cy} 140,${cy+16} 124,${cy}`} fill={fill} stroke={stroke} strokeWidth={1.5}/>
            <path d={`M132,${cy-8} L148,${cy+8} M148,${cy-8} L132,${cy+8}`} stroke={stroke} strokeWidth={1.5} strokeLinecap="round"/>
            <text x={140} y={cy+30} textAnchor="middle" fontSize={9} fill="#64748b">{label}</text>
          </>
        ) : shape === 'gateway-par' ? (
          <>
            <polygon points={`140,${cy-16} 156,${cy} 140,${cy+16} 124,${cy}`} fill={fill} stroke={stroke} strokeWidth={1.5}/>
            <line x1={140} y1={cy-8} x2={140} y2={cy+8} stroke={stroke} strokeWidth={2} strokeLinecap="round"/>
            <line x1={132} y1={cy} x2={148} y2={cy} stroke={stroke} strokeWidth={2} strokeLinecap="round"/>
            <text x={140} y={cy+30} textAnchor="middle" fontSize={9} fill="#64748b">{label}</text>
          </>
        ) : (
          <>
            <polygon points={`140,${cy-16} 156,${cy} 140,${cy+16} 124,${cy}`} fill={fill} stroke={stroke} strokeWidth={1.5}/>
            <circle cx={140} cy={cy} r={6} fill="none" stroke={stroke} strokeWidth={1.5}/>
            <text x={140} y={cy+30} textAnchor="middle" fontSize={9} fill="#64748b">{label}</text>
          </>
        )}

        {/* End */}
        <line x1={193} y1={cy} x2={238} y2={cy} stroke="#64748b" strokeWidth={1.2}/>
        <polygon points={`238,${cy-3} 238,${cy+3} 244,${cy}`} fill="#64748b"/>
        <circle cx={256} cy={cy} r={11} fill="#fef2f2" stroke="#ef4444" strokeWidth={2.5}/>

        {/* API note */}
        {apiNote && (
          <>
            <line x1={140} y1={cy - 14} x2={140} y2={18} stroke="#64748b" strokeWidth={0.9} strokeDasharray="3 2"/>
            <text x={140} y={14} textAnchor="middle" fontSize={7.5} fill="#94a3b8">{apiNote}</text>
          </>
        )}
      </svg>
    </div>
  );
}
