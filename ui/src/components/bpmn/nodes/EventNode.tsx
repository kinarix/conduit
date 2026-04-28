import { Handle, Position, NodeProps } from '@xyflow/react';
import type { BpmnNodeData } from '../bpmnTypes';
import { ELEMENT_COLORS } from '../bpmnTypes';

export default function EventNode({ data, selected }: NodeProps) {
  const d = data as BpmnNodeData;
  const isEnd = d.bpmnType === 'endEvent';
  const colors = ELEMENT_COLORS[d.bpmnType];
  const stroke = selected ? '#6366f1' : colors.stroke;
  const handleStyle = { background: colors.stroke, opacity: selected ? 1 : 0 };

  return (
    <div style={{ position: 'relative', width: 22, height: 22 }}>
      <Handle type="target" position={Position.Left} style={handleStyle} />
      <Handle type="target" position={Position.Top} style={handleStyle} />
      <svg width={22} height={22}>
        <circle
          cx={11} cy={11} r={9}
          fill={colors.fill}
          stroke={stroke}
          strokeWidth={isEnd ? 2.5 : 1.5}
        />
      </svg>
      {d.label && (
        <div style={{
          position: 'absolute',
          top: 26,
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
      <Handle type="source" position={Position.Right} style={handleStyle} />
      <Handle type="source" position={Position.Bottom} style={handleStyle} />
    </div>
  );
}
