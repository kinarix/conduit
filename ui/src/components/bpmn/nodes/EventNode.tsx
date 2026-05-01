import { Handle, Position, NodeProps } from '@xyflow/react';
import type { BpmnNodeData } from '../bpmnTypes';
import { ELEMENT_COLORS, RUNTIME_STATUS_COLOR } from '../bpmnTypes';
import { useIsConnecting } from '../connectingContext';
import { useNodeWarnings } from '../warningsContext';

const BOUNDARY_TYPES = new Set([
  'boundaryTimerEvent', 'boundarySignalEvent', 'boundaryErrorEvent',
]);

const INTERMEDIATE_TYPES = new Set([
  'intermediateCatchTimerEvent', 'intermediateCatchMessageEvent', 'intermediateCatchSignalEvent',
]);

export default function EventNode({ id, data, selected }: NodeProps) {
  const d = data as BpmnNodeData;
  const warnings = useNodeWarnings(id);
  const isEnd = d.bpmnType === 'endEvent';
  const isBoundary = BOUNDARY_TYPES.has(d.bpmnType);
  const isIntermediate = INTERMEDIATE_TYPES.has(d.bpmnType);
  const colors = ELEMENT_COLORS[d.bpmnType];
  const statusColor = d.runtimeStatus ? RUNTIME_STATUS_COLOR[d.runtimeStatus] : null;
  const stroke = selected ? '#6366f1' : (statusColor ?? colors.stroke);
  const strokeWidth = statusColor ? 2.5 : (isEnd ? 2.5 : 1.5);
  const isConnecting = useIsConnecting();
  const showHandles = selected || isConnecting;
  const handleStyle = { background: colors.stroke, opacity: showHandles ? 1 : 0 };
  const attachHandleStyle = isBoundary
    ? { background: '#f59e0b', opacity: showHandles ? 1 : 0, borderRadius: '2px' }
    : handleStyle;
  const opacity = d.runtimeStatus === 'pending' ? 0.55 : 1;
  const ringHalo = d.runtimeStatus === 'active'
    ? <circle cx={11} cy={11} r={11} fill="none" stroke={statusColor!} strokeOpacity={0.25} strokeWidth={4} />
    : null;

  return (
    <div
      style={{ position: 'relative', width: 22, height: 22, opacity }}
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
      <Handle id="left-target"   type="target" position={Position.Left}   style={handleStyle} />
      {!isBoundary && (
        <Handle id="left-source"   type="source" position={Position.Left}   style={handleStyle} />
      )}
      {!isBoundary && (
        <Handle id="top-target"    type="target" position={Position.Top}    style={handleStyle} />
      )}
      {!isBoundary && (
        <Handle id="top-source"    type="source" position={Position.Top}    style={handleStyle} />
      )}
      <svg width={22} height={22} style={{ overflow: 'visible' }}>
        {ringHalo}
        <circle
          cx={11} cy={11} r={9}
          fill={colors.fill}
          stroke={stroke}
          strokeWidth={strokeWidth}
          strokeDasharray={isBoundary ? '3 2' : undefined}
        />
        {isIntermediate && (
          <circle cx={11} cy={11} r={6.5} fill="none" stroke={stroke} strokeWidth={1.2} />
        )}

        {/* Message icon */}
        {(d.bpmnType === 'messageStartEvent' || d.bpmnType === 'intermediateCatchMessageEvent') && (
          <>
            <rect x={6} y={7.5} width={10} height={7} rx={0.5} fill="none" stroke={colors.icon} strokeWidth={1.1}/>
            <path d="M6 7.5l5 4.5 5-4.5" fill="none" stroke={colors.icon} strokeWidth={1.1} strokeLinejoin="round"/>
          </>
        )}

        {/* Timer / clock icon */}
        {(d.bpmnType === 'timerStartEvent' || d.bpmnType === 'boundaryTimerEvent' || d.bpmnType === 'intermediateCatchTimerEvent') && (
          <>
            <circle cx={11} cy={11} r={5} fill="none" stroke={colors.icon} strokeWidth={1.1}/>
            <path d="M11 8v3l2 1.5" stroke={colors.icon} strokeWidth={1.1} strokeLinecap="round" strokeLinejoin="round"/>
          </>
        )}

        {/* Signal icon — filled triangle */}
        {(d.bpmnType === 'boundarySignalEvent' || d.bpmnType === 'intermediateCatchSignalEvent') && (
          <path d="M11 6.5l3.2 6.5H7.8L11 6.5z" fill="none" stroke={colors.icon} strokeWidth={1.1} strokeLinejoin="round"/>
        )}

        {/* Error icon — lightning bolt */}
        {d.bpmnType === 'boundaryErrorEvent' && (
          <path d="M12.5 7l-3 3.5h2.5l-3 4" stroke={colors.icon} strokeWidth={1.3} strokeLinecap="round" strokeLinejoin="round" fill="none"/>
        )}
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
      {!isBoundary && (
        <Handle id="right-target"  type="target" position={Position.Right}  style={handleStyle} />
      )}
      <Handle id="right-source"  type="source" position={Position.Right}  style={handleStyle} />
      {!isBoundary && (
        <Handle id="bottom-target" type="target" position={Position.Bottom} style={handleStyle} />
      )}
      <Handle id="bottom-source" type="source" position={Position.Bottom} style={attachHandleStyle} />
      {isBoundary && (
        <Handle id="top-source" type="source" position={Position.Top} style={attachHandleStyle} />
      )}
    </div>
  );
}
