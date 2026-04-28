import { Handle, Position, NodeProps } from '@xyflow/react';
import type { BpmnNodeData } from '../bpmnTypes';
import { ELEMENT_COLORS } from '../bpmnTypes';

const MARKER: Record<string, string> = {
  exclusiveGateway: '×',
  parallelGateway:  '+',
  inclusiveGateway: '○',
};

export default function GatewayNode({ data, selected }: NodeProps) {
  const d = data as BpmnNodeData;
  const colors = ELEMENT_COLORS[d.bpmnType];
  const stroke = selected ? '#6366f1' : colors.stroke;
  const handleStyle = (base?: React.CSSProperties) => ({
    ...base,
    background: colors.stroke,
    opacity: selected ? 1 : 0,
  });

  return (
    <div style={{ position: 'relative', width: 30, height: 30 }}>
      <Handle type="target" position={Position.Left} style={handleStyle()} />
      <Handle type="target" position={Position.Top} style={handleStyle()} />

      <svg width={30} height={30}>
        <rect
          x={5} y={5} width={20} height={20}
          transform="rotate(45 15 15)"
          fill={colors.fill}
          stroke={stroke}
          strokeWidth={1.5}
        />
        <text
          x={15} y={19}
          textAnchor="middle"
          fontSize={10}
          fontWeight="bold"
          fill={stroke}
        >
          {MARKER[d.bpmnType] ?? '?'}
        </text>
      </svg>

      {d.label && (
        <div style={{
          position: 'absolute',
          top: 34,
          left: '50%',
          transform: 'translateX(-50%)',
          whiteSpace: 'nowrap',
          fontSize: 10,
          color: '#0f172a',
          pointerEvents: 'none',
        }}>
          {d.label}
        </div>
      )}

      <Handle type="source" position={Position.Right} style={handleStyle()} />
      <Handle type="source" position={Position.Bottom} style={handleStyle()} />
    </div>
  );
}
