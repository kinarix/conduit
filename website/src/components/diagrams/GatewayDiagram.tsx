type GatewayVariant = 'exclusive' | 'parallel' | 'inclusive';

function GatewaySymbol({ cx, cy, variant, fill, stroke }: {
  cx: number; cy: number; variant: GatewayVariant; fill: string; stroke: string;
}) {
  const s = 16;
  return (
    <>
      <polygon points={`${cx},${cy-s} ${cx+s},${cy} ${cx},${cy+s} ${cx-s},${cy}`} fill={fill} stroke={stroke} strokeWidth={1.5}/>
      {variant === 'exclusive' && (
        <path d={`M${cx-7},${cy-7} L${cx+7},${cy+7} M${cx+7},${cy-7} L${cx-7},${cy+7}`} stroke={stroke} strokeWidth={1.8} strokeLinecap="round"/>
      )}
      {variant === 'parallel' && (
        <>
          <line x1={cx} y1={cy-8} x2={cx} y2={cy+8} stroke={stroke} strokeWidth={2} strokeLinecap="round"/>
          <line x1={cx-8} y1={cy} x2={cx+8} y2={cy} stroke={stroke} strokeWidth={2} strokeLinecap="round"/>
        </>
      )}
      {variant === 'inclusive' && (
        <circle cx={cx} cy={cy} r={7} fill="none" stroke={stroke} strokeWidth={1.8}/>
      )}
    </>
  );
}

export default function GatewayDiagram({ variant }: { variant: GatewayVariant }) {
  const fill = variant === 'exclusive' ? '#fef9ec' : variant === 'parallel' ? '#f0fdf4' : '#f0f9ff';
  const stroke = variant === 'exclusive' ? '#b45309' : variant === 'parallel' ? '#16a34a' : '#0284c7';
  const label1 = variant === 'exclusive' ? 'amount > 1000' : variant === 'inclusive' ? 'amount > 100' : '';
  const label2 = variant === 'exclusive' ? 'default' : variant === 'inclusive' ? 'status = "priority"' : '';

  if (variant === 'parallel') {
    return (
      <div style={{ margin: '1rem 0', borderRadius: 8, background: '#f8fafc', padding: '10px 12px', border: '1px solid #e2e8f0' }}>
        <svg width={320} height={110} viewBox="0 0 320 110" style={{ display: 'block', overflow: 'visible' }}>
          <circle cx={16} cy={55} r={10} fill="#f0fdf4" stroke="#16a34a" strokeWidth={1.4}/>
          <line x1={26} y1={55} x2={60} y2={55} stroke="#64748b" strokeWidth={1.2}/>
          <polygon points="60,52 60,58 66,55" fill="#64748b"/>
          <GatewaySymbol cx={82} cy={55} variant="parallel" fill={fill} stroke={stroke}/>
          <text x={82} y={82} textAnchor="middle" fontSize={8} fill="#64748b">fork</text>
          <line x1={98} y1={55} x2={124} y2={30} stroke="#64748b" strokeWidth={1.2}/>
          <line x1={98} y1={55} x2={124} y2={80} stroke="#64748b" strokeWidth={1.2}/>
          <rect x={124} y={20} width={60} height={20} rx={3} fill="#ede9fe" stroke="#6366f1" strokeWidth={1.4}/>
          <text x={154} y={34} textAnchor="middle" fontSize={8} fill="#3730a3">Branch A</text>
          <rect x={124} y={70} width={60} height={20} rx={3} fill="#ede9fe" stroke="#6366f1" strokeWidth={1.4}/>
          <text x={154} y={84} textAnchor="middle" fontSize={8} fill="#3730a3">Branch B</text>
          <line x1={184} y1={30} x2={210} y2={55} stroke="#64748b" strokeWidth={1.2}/>
          <line x1={184} y1={80} x2={210} y2={55} stroke="#64748b" strokeWidth={1.2}/>
          <GatewaySymbol cx={226} cy={55} variant="parallel" fill={fill} stroke={stroke}/>
          <text x={226} y={82} textAnchor="middle" fontSize={8} fill="#64748b">join</text>
          <line x1={242} y1={55} x2={274} y2={55} stroke="#64748b" strokeWidth={1.2}/>
          <polygon points="274,52 274,58 280,55" fill="#64748b"/>
          <circle cx={292} cy={55} r={10} fill="#fef2f2" stroke="#ef4444" strokeWidth={2.5}/>
        </svg>
      </div>
    );
  }

  return (
    <div style={{ margin: '1rem 0', borderRadius: 8, background: '#f8fafc', padding: '10px 12px', border: '1px solid #e2e8f0' }}>
      <svg width={320} height={110} viewBox="0 0 320 110" style={{ display: 'block', overflow: 'visible' }}>
        <circle cx={16} cy={55} r={10} fill="#f0fdf4" stroke="#16a34a" strokeWidth={1.4}/>
        <line x1={26} y1={55} x2={60} y2={55} stroke="#64748b" strokeWidth={1.2}/>
        <polygon points="60,52 60,58 66,55" fill="#64748b"/>
        <GatewaySymbol cx={82} cy={55} variant={variant} fill={fill} stroke={stroke}/>
        <line x1={98} y1={55} x2={124} y2={28} stroke="#64748b" strokeWidth={1.2}/>
        <text x={110} y={38} textAnchor="start" fontSize={7} fill="#94a3b8">{label1}</text>
        <rect x={124} y={16} width={68} height={24} rx={3} fill="#ede9fe" stroke="#6366f1" strokeWidth={1.4}/>
        <text x={158} y={32} textAnchor="middle" fontSize={8.5} fill="#3730a3">Path A</text>
        <line x1={192} y1={28} x2={238} y2={55} stroke="#64748b" strokeWidth={1.2}/>
        <polygon points="235,52 238,58 242,53" fill="#64748b"/>
        <line x1={98} y1={55} x2={124} y2={82} stroke="#64748b" strokeWidth={1.2}/>
        <text x={110} y={75} textAnchor="start" fontSize={7} fill="#94a3b8">{label2}</text>
        <rect x={124} y={70} width={68} height={24} rx={3} fill="#ede9fe" stroke="#6366f1" strokeWidth={1.4}/>
        <text x={158} y={86} textAnchor="middle" fontSize={8.5} fill="#3730a3">Path B</text>
        <line x1={192} y1={82} x2={238} y2={55} stroke="#64748b" strokeWidth={1.2}/>
        <polygon points="235,52 238,58 242,53" fill="#64748b"/>
        <circle cx={252} cy={55} r={10} fill="#fef2f2" stroke="#ef4444" strokeWidth={2.5}/>
      </svg>
    </div>
  );
}
