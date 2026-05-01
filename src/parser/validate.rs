use std::collections::{HashMap, HashSet};

use crate::error::{EngineError, Result};
use crate::parser::types::{FlowNode, FlowNodeKind, SequenceFlow};

pub(super) fn validate(
    process_id: &str,
    nodes: &HashMap<String, FlowNode>,
    flows: &[SequenceFlow],
) -> Result<()> {
    let plain_start_count = nodes
        .values()
        .filter(|n| matches!(n.kind, FlowNodeKind::StartEvent))
        .count();
    let message_start_count = nodes
        .values()
        .filter(|n| matches!(n.kind, FlowNodeKind::MessageStartEvent { .. }))
        .count();
    let signal_start_count = nodes
        .values()
        .filter(|n| matches!(n.kind, FlowNodeKind::SignalStartEvent { .. }))
        .count();
    let timer_start_count = nodes
        .values()
        .filter(|n| matches!(n.kind, FlowNodeKind::TimerStartEvent { .. }))
        .count();
    let total_start_count =
        plain_start_count + message_start_count + signal_start_count + timer_start_count;

    if total_start_count == 0 {
        return Err(EngineError::Parse(format!(
            "Process '{process_id}' has no start event"
        )));
    }
    if plain_start_count > 1 {
        return Err(EngineError::Parse(format!(
            "Process '{process_id}' has {plain_start_count} plain start events; only one supported"
        )));
    }

    let end_count = nodes
        .values()
        .filter(|n| matches!(n.kind, FlowNodeKind::EndEvent))
        .count();

    if end_count == 0 {
        return Err(EngineError::Parse(format!(
            "Process '{process_id}' has no end event"
        )));
    }

    for flow in flows {
        if !nodes.contains_key(&flow.source_ref) {
            return Err(EngineError::Parse(format!(
                "SequenceFlow '{}' has unknown sourceRef '{}'",
                flow.id, flow.source_ref
            )));
        }
        if !nodes.contains_key(&flow.target_ref) {
            return Err(EngineError::Parse(format!(
                "SequenceFlow '{}' has unknown targetRef '{}'",
                flow.id, flow.target_ref
            )));
        }
    }

    for node in nodes.values() {
        if let FlowNodeKind::ExclusiveGateway {
            default_flow: Some(default_id),
        } = &node.kind
        {
            if !flows.iter().any(|f| &f.id == default_id) {
                return Err(EngineError::Parse(format!(
                    "ExclusiveGateway '{}' references unknown default flow '{}'",
                    node.id, default_id
                )));
            }
        }

        let (gateway_kind, default_id_opt) = match &node.kind {
            FlowNodeKind::ExclusiveGateway { default_flow } => {
                (Some("ExclusiveGateway"), default_flow.as_deref())
            }
            FlowNodeKind::InclusiveGateway { default_flow } => {
                (Some("InclusiveGateway"), default_flow.as_deref())
            }
            _ => (None, None),
        };
        if let Some(kind_label) = gateway_kind {
            let outgoing = flows.iter().filter(|f| f.source_ref == node.id).count();
            if outgoing == 0 {
                return Err(EngineError::Parse(format!(
                    "{kind_label} '{}' has no outgoing sequence flow",
                    node.id
                )));
            }
            if let Some(default_id) = default_id_opt {
                let default_originates_here = flows
                    .iter()
                    .any(|f| f.id == default_id && f.source_ref == node.id);
                if !default_originates_here {
                    return Err(EngineError::Parse(format!(
                        "{kind_label} '{}' default flow '{default_id}' does not originate from this gateway",
                        node.id
                    )));
                }
            }
        }

        let boundary_host = match &node.kind {
            FlowNodeKind::BoundaryTimerEvent { attached_to, .. } => Some(attached_to.as_str()),
            FlowNodeKind::BoundarySignalEvent { attached_to, .. } => Some(attached_to.as_str()),
            FlowNodeKind::BoundaryErrorEvent { attached_to, .. } => Some(attached_to.as_str()),
            _ => None,
        };
        if let Some(host_id) = boundary_host {
            if !nodes.contains_key(host_id) {
                return Err(EngineError::Parse(format!(
                    "BoundaryEvent '{}' attachedToRef '{}' not found in process",
                    node.id, host_id
                )));
            }
        }
    }

    let start_ids: HashSet<&str> = nodes
        .values()
        .filter(|n| {
            matches!(
                n.kind,
                FlowNodeKind::StartEvent
                    | FlowNodeKind::MessageStartEvent { .. }
                    | FlowNodeKind::SignalStartEvent { .. }
                    | FlowNodeKind::TimerStartEvent { .. }
            )
        })
        .map(|n| n.id.as_str())
        .collect();
    for flow in flows {
        if start_ids.contains(flow.target_ref.as_str()) {
            return Err(EngineError::Parse(format!(
                "SequenceFlow '{}' targets start event '{}'; start events cannot have incoming flows",
                flow.id, flow.target_ref
            )));
        }
    }

    let end_ids: HashSet<&str> = nodes
        .values()
        .filter(|n| matches!(n.kind, FlowNodeKind::EndEvent))
        .map(|n| n.id.as_str())
        .collect();
    for flow in flows {
        if end_ids.contains(flow.source_ref.as_str()) {
            return Err(EngineError::Parse(format!(
                "SequenceFlow '{}' originates from end event '{}'; end events cannot have outgoing flows",
                flow.id, flow.source_ref
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate;
    use crate::parser::types::{FlowNode, FlowNodeKind, SequenceFlow};
    use std::collections::HashMap;

    fn node(id: &str, kind: FlowNodeKind) -> (String, FlowNode) {
        (
            id.to_string(),
            FlowNode {
                id: id.to_string(),
                name: None,
                kind,
            },
        )
    }

    fn flow(id: &str, src: &str, tgt: &str) -> SequenceFlow {
        SequenceFlow {
            id: id.to_string(),
            source_ref: src.to_string(),
            target_ref: tgt.to_string(),
            condition: None,
        }
    }

    fn minimal_nodes() -> HashMap<String, FlowNode> {
        [
            node("start", FlowNodeKind::StartEvent),
            node("end", FlowNodeKind::EndEvent),
        ]
        .into_iter()
        .collect()
    }

    fn minimal_flows() -> Vec<SequenceFlow> {
        vec![flow("f1", "start", "end")]
    }

    // ── happy path ────────────────────────────────────────────────────────────

    #[test]
    fn valid_minimal_process() {
        assert!(validate("proc", &minimal_nodes(), &minimal_flows()).is_ok());
    }

    // ── start event checks ────────────────────────────────────────────────────

    #[test]
    fn no_start_event_is_err() {
        let nodes = [node("end", FlowNodeKind::EndEvent)].into_iter().collect();
        assert!(validate("proc", &nodes, &[]).is_err());
    }

    #[test]
    fn multiple_plain_start_events_is_err() {
        let mut nodes = minimal_nodes();
        nodes.insert(
            "start2".to_string(),
            FlowNode {
                id: "start2".to_string(),
                name: None,
                kind: FlowNodeKind::StartEvent,
            },
        );
        assert!(validate("proc", &nodes, &minimal_flows()).is_err());
    }

    #[test]
    fn message_start_counts_toward_start_total() {
        let nodes = [
            node(
                "mstart",
                FlowNodeKind::MessageStartEvent {
                    message_name: "msg".to_string(),
                },
            ),
            node("end", FlowNodeKind::EndEvent),
        ]
        .into_iter()
        .collect();
        let flows = vec![flow("f1", "mstart", "end")];
        assert!(validate("proc", &nodes, &flows).is_ok());
    }

    // ── end event checks ──────────────────────────────────────────────────────

    #[test]
    fn no_end_event_is_err() {
        let nodes = [node("start", FlowNodeKind::StartEvent)]
            .into_iter()
            .collect();
        assert!(validate("proc", &nodes, &[]).is_err());
    }

    // ── sequence flow ref checks ──────────────────────────────────────────────

    #[test]
    fn unknown_source_ref_is_err() {
        let flows = vec![flow("f1", "missing", "end")];
        assert!(validate("proc", &minimal_nodes(), &flows).is_err());
    }

    #[test]
    fn unknown_target_ref_is_err() {
        let flows = vec![flow("f1", "start", "missing")];
        assert!(validate("proc", &minimal_nodes(), &flows).is_err());
    }

    // ── exclusive gateway default flow ────────────────────────────────────────

    #[test]
    fn exclusive_gateway_unknown_default_flow_is_err() {
        let mut nodes = minimal_nodes();
        nodes.insert(
            "gw".to_string(),
            FlowNode {
                id: "gw".to_string(),
                name: None,
                kind: FlowNodeKind::ExclusiveGateway {
                    default_flow: Some("nonexistent_flow".to_string()),
                },
            },
        );
        let flows = vec![flow("f1", "start", "gw"), flow("f2", "gw", "end")];
        assert!(validate("proc", &nodes, &flows).is_err());
    }

    #[test]
    fn exclusive_gateway_known_default_flow_ok() {
        let mut nodes = minimal_nodes();
        nodes.insert(
            "gw".to_string(),
            FlowNode {
                id: "gw".to_string(),
                name: None,
                kind: FlowNodeKind::ExclusiveGateway {
                    default_flow: Some("f2".to_string()),
                },
            },
        );
        let flows = vec![flow("f1", "start", "gw"), flow("f2", "gw", "end")];
        assert!(validate("proc", &nodes, &flows).is_ok());
    }

    #[test]
    fn exclusive_gateway_no_outgoing_is_err() {
        let mut nodes = minimal_nodes();
        nodes.insert(
            "gw".to_string(),
            FlowNode {
                id: "gw".to_string(),
                name: None,
                kind: FlowNodeKind::ExclusiveGateway { default_flow: None },
            },
        );
        // f1: start -> gw, no flow originating from gw
        let flows = vec![flow("f1", "start", "gw")];
        let err = validate("proc", &nodes, &flows).unwrap_err();
        assert!(format!("{err}").contains("no outgoing"));
    }

    #[test]
    fn inclusive_gateway_no_outgoing_is_err() {
        let mut nodes = minimal_nodes();
        nodes.insert(
            "gw".to_string(),
            FlowNode {
                id: "gw".to_string(),
                name: None,
                kind: FlowNodeKind::InclusiveGateway { default_flow: None },
            },
        );
        let flows = vec![flow("f1", "start", "gw")];
        assert!(validate("proc", &nodes, &flows).is_err());
    }

    #[test]
    fn exclusive_gateway_default_must_originate_from_gateway() {
        let mut nodes = minimal_nodes();
        nodes.insert(
            "gw".to_string(),
            FlowNode {
                id: "gw".to_string(),
                name: None,
                kind: FlowNodeKind::ExclusiveGateway {
                    default_flow: Some("f_other".to_string()),
                },
            },
        );
        // f_other exists but does not originate from gw
        let flows = vec![
            flow("f1", "start", "gw"),
            flow("f2", "gw", "end"),
            flow("f_other", "start", "end"),
        ];
        let err = validate("proc", &nodes, &flows).unwrap_err();
        assert!(format!("{err}").contains("does not originate"));
    }

    // ── boundary event attached_to check ─────────────────────────────────────

    #[test]
    fn boundary_event_unknown_attached_to_is_err() {
        let mut nodes = minimal_nodes();
        nodes.insert(
            "boundary".to_string(),
            FlowNode {
                id: "boundary".to_string(),
                name: None,
                kind: FlowNodeKind::BoundaryTimerEvent {
                    timer: crate::parser::TimerSpec::Duration("PT1M".to_string()),
                    attached_to: "nonexistent_task".to_string(),
                    cancelling: true,
                },
            },
        );
        assert!(validate("proc", &nodes, &minimal_flows()).is_err());
    }

    // ── no flows into start events ────────────────────────────────────────────

    #[test]
    fn flow_targeting_start_event_is_err() {
        let mut nodes = minimal_nodes();
        nodes.insert(
            "task".to_string(),
            FlowNode {
                id: "task".to_string(),
                name: None,
                kind: FlowNodeKind::UserTask,
            },
        );
        let flows = vec![flow("f1", "start", "end"), flow("f2", "task", "start")];
        assert!(validate("proc", &nodes, &flows).is_err());
    }

    // ── no flows out of end events ────────────────────────────────────────────

    #[test]
    fn flow_originating_from_end_event_is_err() {
        let mut nodes = minimal_nodes();
        nodes.insert(
            "task".to_string(),
            FlowNode {
                id: "task".to_string(),
                name: None,
                kind: FlowNodeKind::UserTask,
            },
        );
        let flows = vec![flow("f1", "start", "end"), flow("f2", "end", "task")];
        assert!(validate("proc", &nodes, &flows).is_err());
    }
}
