import { Handle, Position, NodeProps } from '@xyflow/react';
import type { BpmnNodeData } from '../bpmnTypes';
import { ELEMENT_COLORS } from '../bpmnTypes';

const PersonIcon = ({ color }: { color: string }) => (
  <svg width={10} height={10} viewBox="0 0 14 14" fill="none">
    <circle cx={7} cy={4} r={2.5} stroke={color} strokeWidth={1.2} />
    <path d="M2 12c0-2.76 2.24-5 5-5s5 2.24 5 5" stroke={color} strokeWidth={1.2} strokeLinecap="round" />
  </svg>
);

const GearIcon = ({ color }: { color: string }) => (
  <svg width={10} height={10} viewBox="0 0 14 14" fill="none">
    <circle cx={7} cy={7} r={2} stroke={color} strokeWidth={1.2} />
    <path
      d="M7 1v1.5M7 11.5V13M1 7h1.5M11.5 7H13M2.93 2.93l1.06 1.06M10.01 10.01l1.06 1.06M2.93 11.07l1.06-1.06M10.01 3.99l1.06-1.06"
      stroke={color} strokeWidth={1.2} strokeLinecap="round"
    />
  </svg>
);

export default function TaskNode({ data, selected }: NodeProps) {
  const d = data as BpmnNodeData;
  const colors = ELEMENT_COLORS[d.bpmnType] ?? ELEMENT_COLORS.userTask;
  const border = selected ? `1.5px solid #6366f1` : `1.5px solid ${colors.stroke}`;
  const handleStyle = { background: colors.stroke, opacity: selected ? 1 : 0 };

  return (
    <div style={{
      width: 72, height: 36,
      background: colors.fill,
      border,
      borderRadius: 5,
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      position: 'relative',
      padding: '2px 6px',
    }}>
      <Handle type="target" position={Position.Left} style={handleStyle} />
      <Handle type="target" position={Position.Top} style={handleStyle} />

      <div style={{ position: 'absolute', top: 3, left: 4 }}>
        {d.bpmnType === 'serviceTask'
          ? <GearIcon color={colors.icon} />
          : <PersonIcon color={colors.icon} />
        }
      </div>

      <div style={{
        fontSize: 11,
        fontWeight: 500,
        color: '#0f172a',
        textAlign: 'center',
        lineHeight: 1.2,
        maxWidth: '100%',
        overflow: 'hidden',
        textOverflow: 'ellipsis',
        whiteSpace: 'nowrap',
      }}>
        {d.label || 'Task'}
      </div>

      <Handle type="source" position={Position.Right} style={handleStyle} />
      <Handle type="source" position={Position.Bottom} style={handleStyle} />
    </div>
  );
}
