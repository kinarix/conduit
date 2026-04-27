use roxmltree::Document;
use std::collections::HashMap;

use crate::error::{EngineError, Result};

const BPMN_NS: &str = "http://www.omg.org/spec/BPMN/20100524/MODEL";
// Camunda 7 / Activiti external-task attribute namespace
const CAMUNDA_NS: &str = "http://activiti.org/bpmn";

#[derive(Debug, Clone, PartialEq)]
pub enum FlowNodeKind {
    StartEvent,
    MessageStartEvent {
        message_name: String,
    },
    EndEvent,
    UserTask,
    ServiceTask {
        topic: Option<String>,
    },
    ExclusiveGateway {
        default_flow: Option<String>,
    },
    ParallelGateway,
    IntermediateTimerCatchEvent {
        duration: String,
    },
    IntermediateMessageCatchEvent {
        message_name: String,
        correlation_key_expr: Option<String>,
    },
    ReceiveTask {
        message_name: String,
        correlation_key_expr: Option<String>,
    },
    BoundaryTimerEvent {
        duration: String,
        attached_to: String,
        cancelling: bool,
    },
    SignalStartEvent {
        signal_name: String,
    },
    IntermediateSignalCatchEvent {
        signal_name: String,
    },
    BoundarySignalEvent {
        signal_name: String,
        attached_to: String,
        cancelling: bool,
    },
}

#[derive(Debug, Clone)]
pub struct FlowNode {
    pub id: String,
    pub name: Option<String>,
    pub kind: FlowNodeKind,
}

#[derive(Debug, Clone)]
pub struct SequenceFlow {
    pub id: String,
    pub source_ref: String,
    pub target_ref: String,
    pub condition: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ProcessGraph {
    pub process_id: String,
    pub process_name: Option<String>,
    pub nodes: HashMap<String, FlowNode>,
    pub flows: Vec<SequenceFlow>,
    /// outgoing[source_id] = [target_id, ...]
    pub outgoing: HashMap<String, Vec<String>>,
    /// incoming[target_id] = [source_id, ...]
    pub incoming: HashMap<String, Vec<String>>,
    /// attached_to[host_element_id] = [boundary_event_ids, ...]
    pub attached_to: HashMap<String, Vec<String>>,
}

/// Parse BPMN XML into a validated `ProcessGraph`.
///
/// Rejects any element that belongs to a future phase to prevent silent
/// mis-execution. The supported set for Phase 3:
///   startEvent, endEvent, userTask, serviceTask, sequenceFlow
pub fn parse(xml: &str) -> Result<ProcessGraph> {
    let doc = Document::parse(xml).map_err(|e| EngineError::Parse(format!("Invalid XML: {e}")))?;

    // Collect <message> definitions from the <definitions> root level (id → name)
    let message_defs: HashMap<String, String> = doc
        .root_element()
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "message")
        .filter_map(|n| {
            let id = n.attribute("id")?;
            let name = n.attribute("name")?;
            Some((id.to_string(), name.to_string()))
        })
        .collect();

    // Collect <signal> definitions from the <definitions> root level (id → name)
    let signal_defs: HashMap<String, String> = doc
        .root_element()
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "signal")
        .filter_map(|n| {
            let id = n.attribute("id")?;
            let name = n.attribute("name")?;
            Some((id.to_string(), name.to_string()))
        })
        .collect();

    // Find the <process> element — works for both bare `<process>` and `<bpmn:process>`
    let process_node = doc
        .root_element()
        .children()
        .find(|n| {
            n.is_element()
                && n.tag_name().name() == "process"
                && n.tag_name().namespace() == Some(BPMN_NS)
        })
        .ok_or_else(|| EngineError::Parse("No <process> element found".to_string()))?;

    let process_id = process_node
        .attribute("id")
        .ok_or_else(|| EngineError::Parse("Process element missing id attribute".to_string()))?
        .to_string();
    let process_name = process_node.attribute("name").map(|s| s.to_string());

    let mut nodes: HashMap<String, FlowNode> = HashMap::new();
    let mut flows: Vec<SequenceFlow> = Vec::new();

    for child in process_node.children().filter(|n| n.is_element()) {
        let local = child.tag_name().name();
        let ns = child.tag_name().namespace();

        // Skip non-BPMN namespace elements (e.g. bpmndi: inside process, camunda: listeners)
        if let Some(ns_uri) = ns {
            if ns_uri != BPMN_NS {
                continue;
            }
        }

        match local {
            "startEvent" => {
                let id = require_id(&child, local)?;
                let name = child.attribute("name").map(|s| s.to_string());
                let kind = if let Some(msg_name) = extract_message_name(&child, &message_defs, &id)?
                {
                    FlowNodeKind::MessageStartEvent {
                        message_name: msg_name,
                    }
                } else if let Some(sig_name) = extract_signal_name(&child, &signal_defs, &id)? {
                    FlowNodeKind::SignalStartEvent {
                        signal_name: sig_name,
                    }
                } else {
                    FlowNodeKind::StartEvent
                };
                nodes.insert(id.clone(), FlowNode { id, name, kind });
            }
            "endEvent" => {
                let id = require_id(&child, local)?;
                let name = child.attribute("name").map(|s| s.to_string());
                nodes.insert(
                    id.clone(),
                    FlowNode {
                        id,
                        name,
                        kind: FlowNodeKind::EndEvent,
                    },
                );
            }
            "userTask" => {
                let id = require_id(&child, local)?;
                let name = child.attribute("name").map(|s| s.to_string());
                nodes.insert(
                    id.clone(),
                    FlowNode {
                        id,
                        name,
                        kind: FlowNodeKind::UserTask,
                    },
                );
            }
            "serviceTask" => {
                let id = require_id(&child, local)?;
                let name = child.attribute("name").map(|s| s.to_string());
                let topic = extract_topic(&child);
                nodes.insert(
                    id.clone(),
                    FlowNode {
                        id,
                        name,
                        kind: FlowNodeKind::ServiceTask { topic },
                    },
                );
            }
            "sequenceFlow" => {
                let id = require_id(&child, local)?;
                let source_ref = child
                    .attribute("sourceRef")
                    .ok_or_else(|| {
                        EngineError::Parse(format!("sequenceFlow '{id}' missing sourceRef"))
                    })?
                    .to_string();
                let target_ref = child
                    .attribute("targetRef")
                    .ok_or_else(|| {
                        EngineError::Parse(format!("sequenceFlow '{id}' missing targetRef"))
                    })?
                    .to_string();
                let condition = extract_condition(&child);
                flows.push(SequenceFlow {
                    id,
                    source_ref,
                    target_ref,
                    condition,
                });
            }

            "exclusiveGateway" => {
                let id = require_id(&child, local)?;
                let name = child.attribute("name").map(|s| s.to_string());
                let default_flow = child.attribute("default").map(|s| s.to_string());
                nodes.insert(
                    id.clone(),
                    FlowNode {
                        id,
                        name,
                        kind: FlowNodeKind::ExclusiveGateway { default_flow },
                    },
                );
            }

            "intermediateCatchEvent" => {
                let id = require_id(&child, local)?;
                let name = child.attribute("name").map(|s| s.to_string());
                let has_timer = child
                    .children()
                    .any(|n| n.is_element() && n.tag_name().name() == "timerEventDefinition");
                let kind = if has_timer {
                    FlowNodeKind::IntermediateTimerCatchEvent {
                        duration: extract_timer_duration(&child)?,
                    }
                } else if let Some(msg_name) = extract_message_name(&child, &message_defs, &id)? {
                    let correlation_key_expr = extract_correlation_key(&child);
                    FlowNodeKind::IntermediateMessageCatchEvent {
                        message_name: msg_name,
                        correlation_key_expr,
                    }
                } else if let Some(sig_name) = extract_signal_name(&child, &signal_defs, &id)? {
                    FlowNodeKind::IntermediateSignalCatchEvent {
                        signal_name: sig_name,
                    }
                } else {
                    return Err(EngineError::Parse(format!(
                        "intermediateCatchEvent '{id}' has no supported event definition"
                    )));
                };
                nodes.insert(id.clone(), FlowNode { id, name, kind });
            }

            "boundaryEvent" => {
                let id = require_id(&child, local)?;
                let name = child.attribute("name").map(|s| s.to_string());
                let attached_to = child
                    .attribute("attachedToRef")
                    .ok_or_else(|| {
                        EngineError::Parse(format!("boundaryEvent '{id}' missing attachedToRef"))
                    })?
                    .to_string();
                let cancelling = child
                    .attribute("cancelActivity")
                    .map(|s| s != "false")
                    .unwrap_or(true);
                let has_timer = child
                    .children()
                    .any(|n| n.is_element() && n.tag_name().name() == "timerEventDefinition");
                let kind = if has_timer {
                    FlowNodeKind::BoundaryTimerEvent {
                        duration: extract_timer_duration(&child)?,
                        attached_to,
                        cancelling,
                    }
                } else if let Some(sig_name) = extract_signal_name(&child, &signal_defs, &id)? {
                    FlowNodeKind::BoundarySignalEvent {
                        signal_name: sig_name,
                        attached_to,
                        cancelling,
                    }
                } else {
                    return Err(EngineError::Parse(format!(
                        "boundaryEvent '{id}' has no supported event definition (timer or signal)"
                    )));
                };
                nodes.insert(id.clone(), FlowNode { id, name, kind });
            }

            "parallelGateway" => {
                let id = require_id(&child, local)?;
                let name = child.attribute("name").map(|s| s.to_string());
                nodes.insert(
                    id.clone(),
                    FlowNode {
                        id,
                        name,
                        kind: FlowNodeKind::ParallelGateway,
                    },
                );
            }

            "receiveTask" => {
                let id = require_id(&child, local)?;
                let name = child.attribute("name").map(|s| s.to_string());
                let message_name =
                    extract_message_name(&child, &message_defs, &id)?.ok_or_else(|| {
                        EngineError::Parse(format!(
                            "receiveTask '{id}' missing messageRef or message definition"
                        ))
                    })?;
                let correlation_key_expr = extract_correlation_key(&child);
                nodes.insert(
                    id.clone(),
                    FlowNode {
                        id,
                        name,
                        kind: FlowNodeKind::ReceiveTask {
                            message_name,
                            correlation_key_expr,
                        },
                    },
                );
            }

            // Future-phase semantic elements — reject explicitly so callers get a clear error
            "inclusiveGateway"
            | "eventBasedGateway"
            | "complexGateway"
            | "intermediateThrowEvent"
            | "subProcess"
            | "transaction"
            | "adHocSubProcess"
            | "sendTask"
            | "businessRuleTask"
            | "manualTask"
            | "scriptTask"
            | "callActivity" => return Err(EngineError::UnsupportedElement(local.to_string())),

            // Non-semantic / presentation elements that are harmless to ignore
            "laneSet"
            | "extensionElements"
            | "documentation"
            | "ioSpecification"
            | "property"
            | "dataObject"
            | "dataObjectReference"
            | "dataStoreReference"
            | "textAnnotation"
            | "association"
            | "group" => {}

            other => return Err(EngineError::UnsupportedElement(other.to_string())),
        }
    }

    validate(&process_id, &nodes, &flows)?;

    let mut outgoing: HashMap<String, Vec<String>> = HashMap::new();
    let mut incoming: HashMap<String, Vec<String>> = HashMap::new();
    for flow in &flows {
        outgoing
            .entry(flow.source_ref.clone())
            .or_default()
            .push(flow.target_ref.clone());
        incoming
            .entry(flow.target_ref.clone())
            .or_default()
            .push(flow.source_ref.clone());
    }

    let mut attached_to: HashMap<String, Vec<String>> = HashMap::new();
    for node in nodes.values() {
        let host_id = match &node.kind {
            FlowNodeKind::BoundaryTimerEvent { attached_to: h, .. } => Some(h.clone()),
            FlowNodeKind::BoundarySignalEvent { attached_to: h, .. } => Some(h.clone()),
            _ => None,
        };
        if let Some(h) = host_id {
            attached_to.entry(h).or_default().push(node.id.clone());
        }
    }

    Ok(ProcessGraph {
        process_id,
        process_name,
        nodes,
        flows,
        outgoing,
        incoming,
        attached_to,
    })
}

fn require_id<'a>(node: &roxmltree::Node<'a, '_>, element: &str) -> Result<String> {
    node.attribute("id")
        .map(|s| s.to_string())
        .ok_or_else(|| EngineError::Parse(format!("<{element}> element missing id attribute")))
}

/// Extract the external-task topic from a serviceTask node.
/// Checks (in order): plain `topic` attribute, Camunda `camunda:topic` attribute,
/// and a `<topic>` child text element inside `<extensionElements>`.
fn extract_topic(node: &roxmltree::Node) -> Option<String> {
    if let Some(t) = node.attribute("topic") {
        return Some(t.to_string());
    }
    if let Some(t) = node.attribute((CAMUNDA_NS, "topic")) {
        return Some(t.to_string());
    }
    for ext in node
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "extensionElements")
    {
        for inner in ext.children().filter(|n| n.is_element()) {
            if inner.tag_name().name() == "topic" {
                return inner
                    .text()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty());
            }
        }
    }
    None
}

fn extract_timer_duration(node: &roxmltree::Node) -> Result<String> {
    for def in node
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "timerEventDefinition")
    {
        for dur_node in def
            .children()
            .filter(|n| n.is_element() && n.tag_name().name() == "timeDuration")
        {
            if let Some(text) = dur_node.text() {
                let s = text.trim().to_string();
                if !s.is_empty() {
                    return Ok(s);
                }
            }
        }
    }
    Err(EngineError::Parse(
        "Timer event missing timerEventDefinition/timeDuration".to_string(),
    ))
}

fn extract_condition(node: &roxmltree::Node) -> Option<String> {
    node.children()
        .find(|n| n.is_element() && n.tag_name().name() == "conditionExpression")
        .and_then(|n| n.text())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Resolve the message name for an event node that contains a `<messageEventDefinition>`.
/// Returns `None` if the node has no `messageEventDefinition` child.
/// Returns `Err` if the `messageRef` attribute is present but references an unknown message.
fn extract_message_name(
    node: &roxmltree::Node,
    message_defs: &HashMap<String, String>,
    element_id: &str,
) -> Result<Option<String>> {
    let def = node
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "messageEventDefinition");
    let def = match def {
        Some(d) => d,
        None => return Ok(None),
    };

    // messageRef may be a bare id or a prefixed QName like "bpmn:msg1" — strip prefix
    if let Some(msg_ref) = def.attribute("messageRef") {
        let bare_id = msg_ref.split(':').next_back().unwrap_or(msg_ref);
        if let Some(name) = message_defs.get(bare_id) {
            return Ok(Some(name.clone()));
        }
        // Inline message name fallback: some tools embed the name directly as messageRef
        return Ok(Some(bare_id.to_string()));
    }

    Err(EngineError::Parse(format!(
        "messageEventDefinition on '{element_id}' is missing messageRef attribute"
    )))
}

/// Extract the correlation key expression from a message event node.
/// Looks for a plain `correlationKey` attribute or a Camunda-namespace `camunda:correlationKey`.
fn extract_correlation_key(node: &roxmltree::Node) -> Option<String> {
    if let Some(v) = node.attribute("correlationKey") {
        return Some(v.to_string());
    }
    if let Some(v) = node.attribute((CAMUNDA_NS, "correlationKey")) {
        return Some(v.to_string());
    }
    // Also check inside <extensionElements>
    for ext in node
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "extensionElements")
    {
        for inner in ext.children().filter(|n| n.is_element()) {
            if inner.tag_name().name() == "correlationKey" {
                return inner
                    .text()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty());
            }
        }
    }
    None
}

/// Resolve the signal name for an event node that contains a `<signalEventDefinition>`.
/// Returns `None` if the node has no `signalEventDefinition` child.
/// Returns `Err` if the `signalRef` attribute is present but unresolvable.
fn extract_signal_name(
    node: &roxmltree::Node,
    signal_defs: &HashMap<String, String>,
    element_id: &str,
) -> Result<Option<String>> {
    let def = node
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "signalEventDefinition");
    let def = match def {
        Some(d) => d,
        None => return Ok(None),
    };

    if let Some(sig_ref) = def.attribute("signalRef") {
        let bare_id = sig_ref.split(':').next_back().unwrap_or(sig_ref);
        if let Some(name) = signal_defs.get(bare_id) {
            return Ok(Some(name.clone()));
        }
        return Ok(Some(bare_id.to_string()));
    }

    Err(EngineError::Parse(format!(
        "signalEventDefinition on '{element_id}' is missing signalRef attribute"
    )))
}

fn validate(
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
    let total_start_count = plain_start_count + message_start_count + signal_start_count;

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
        let boundary_host = match &node.kind {
            FlowNodeKind::BoundaryTimerEvent { attached_to, .. } => Some(attached_to.as_str()),
            FlowNodeKind::BoundarySignalEvent { attached_to, .. } => Some(attached_to.as_str()),
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

    Ok(())
}
