import { Handle, Position, NodeProps } from '@xyflow/react';
import type { BpmnNodeData } from '../bpmnTypes';
import { ELEMENT_COLORS, RUNTIME_STATUS_COLOR } from '../bpmnTypes';
import { useIsConnecting } from '../connectingContext';
import { useNodeWarnings } from '../warningsContext';

const PersonIcon = ({ color }: { color: string }) => (
  <svg width={10} height={10} viewBox="0 0 14 14" fill="none">
    <circle cx={7} cy={4} r={2.5} stroke={color} strokeWidth={1.2} />
    <path d="M2 12c0-2.76 2.24-5 5-5s5 2.24 5 5" stroke={color} strokeWidth={1.2} strokeLinecap="round" />
  </svg>
);

const GearIcon = ({ color }: { color: string }) => (
  <svg width={11} height={11} viewBox="0 0 20 20">
    <path
      fillRule="evenodd"
      clipRule="evenodd"
      d="M11.49 3.17c-.38-1.56-2.6-1.56-2.98 0a1.532 1.532 0 0 1-2.286.948c-1.372-.836-2.942.734-2.106 2.106.54.886.061 2.042-.947 2.287-1.561.379-1.561 2.6 0 2.978a1.532 1.532 0 0 1 .947 2.287c-.836 1.372.734 2.942 2.106 2.106a1.532 1.532 0 0 1 2.287.947c.379 1.561 2.6 1.561 2.978 0a1.533 1.533 0 0 1 2.287-.947c1.372.836 2.942-.734 2.106-2.106a1.533 1.533 0 0 1 .947-2.287c1.561-.379 1.561-2.6 0-2.978a1.532 1.532 0 0 1-.947-2.287c.836-1.372-.734-2.942-2.106-2.106a1.532 1.532 0 0 1-2.287-.947zM10 13a3 3 0 1 0 0-6 3 3 0 0 0 0 6z"
      fill={color}
    />
  </svg>
);

const EnvelopeFilledIcon = ({ color, fill }: { color: string; fill: string }) => (
  <svg width={10} height={8} viewBox="0 0 12 9" fill="none">
    <rect x={0.5} y={0.5} width={11} height={8} rx={0.5} fill={color} stroke={color} strokeWidth={1}/>
    <path d="M0.5 0.5l5.5 4.5 5.5-4.5" fill="none" stroke={fill} strokeWidth={1} strokeLinejoin="round"/>
  </svg>
);

const EnvelopeOutlineIcon = ({ color }: { color: string }) => (
  <svg width={10} height={8} viewBox="0 0 12 9" fill="none">
    <rect x={0.5} y={0.5} width={11} height={8} rx={0.5} fill="none" stroke={color} strokeWidth={1}/>
    <path d="M0.5 0.5l5.5 4.5 5.5-4.5" fill="none" stroke={color} strokeWidth={1} strokeLinejoin="round"/>
  </svg>
);

export default function TaskNode({ id, data, selected }: NodeProps) {
  const d = data as BpmnNodeData;
  const colors = ELEMENT_COLORS[d.bpmnType] ?? ELEMENT_COLORS.userTask;
  const isConnecting = useIsConnecting();
  const warnings = useNodeWarnings(id);

  const statusColor = d.runtimeStatus ? RUNTIME_STATUS_COLOR[d.runtimeStatus] : null;
  const border = selected
    ? `1.5px solid #6366f1`
    : statusColor
      ? `2px solid ${statusColor}`
      : `1.5px solid ${colors.stroke}`;
  const handleStyle = { background: colors.stroke, opacity: selected || isConnecting ? 1 : 0 };
  const boxShadow = d.runtimeStatus === 'active'
    ? `0 0 0 4px ${statusColor}33`
    : undefined;
  const opacity = d.runtimeStatus === 'pending' ? 0.55 : 1;

  return (
    <div
      style={{
        width: 72, height: 36,
        background: colors.fill,
        border,
        borderRadius: 5,
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        position: 'relative',
        padding: '2px 6px',
        boxShadow,
        opacity,
      }}
      title={warnings.length > 0 ? warnings.join('\n') : undefined}
    >
      {warnings.length > 0 && (
        <div style={{
          position: 'absolute', top: -6, right: -6,
          width: 16, height: 16, borderRadius: '50%',
          background: '#f59e0b', border: '1.5px solid #fff',
          display: 'flex', alignItems: 'center', justifyContent: 'center',
          fontSize: 9, fontWeight: 700, color: '#fff',
          zIndex: 10, pointerEvents: 'none', lineHeight: 1,
        }}>
          {warnings.length}
        </div>
      )}
      <Handle id="target-left" type="target" position={Position.Left} style={handleStyle} />
      <Handle id="target-top"  type="target" position={Position.Top}  style={handleStyle} />

      <div style={{ position: 'absolute', top: 3, left: 4 }}>
        {d.bpmnType === 'serviceTask'
          ? <GearIcon color={colors.icon} />
          : d.bpmnType === 'sendTask'
          ? <EnvelopeFilledIcon color={colors.icon} fill={colors.fill} />
          : d.bpmnType === 'receiveTask'
          ? <EnvelopeOutlineIcon color={colors.icon} />
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

      <Handle id="source-right"  type="source" position={Position.Right}  style={handleStyle} />
      <Handle id="source-bottom" type="source" position={Position.Bottom} style={handleStyle} />
    </div>
  );
}
