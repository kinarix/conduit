use roxmltree::Document;
use std::collections::HashMap;

use crate::error::{EngineError, Result};

const BPMN_NS: &str = "http://www.omg.org/spec/BPMN/20100524/MODEL";
// Camunda 7 / Activiti external-task attribute namespace
const CAMUNDA_NS: &str = "http://activiti.org/bpmn";

#[derive(Debug, Clone, PartialEq)]
pub enum FlowNodeKind {
    StartEvent,
    EndEvent,
    UserTask,
    ServiceTask {
        topic: Option<String>,
    },
    ExclusiveGateway {
        default_flow: Option<String>,
    },
    IntermediateTimerCatchEvent {
        duration: String,
    },
    BoundaryTimerEvent {
        duration: String,
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
                nodes.insert(
                    id.clone(),
                    FlowNode {
                        id,
                        name,
                        kind: FlowNodeKind::StartEvent,
                    },
                );
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
                let duration = extract_timer_duration(&child)?;
                nodes.insert(
                    id.clone(),
                    FlowNode {
                        id,
                        name,
                        kind: FlowNodeKind::IntermediateTimerCatchEvent { duration },
                    },
                );
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
                let duration = extract_timer_duration(&child)?;
                nodes.insert(
                    id.clone(),
                    FlowNode {
                        id,
                        name,
                        kind: FlowNodeKind::BoundaryTimerEvent {
                            duration,
                            attached_to,
                            cancelling,
                        },
                    },
                );
            }

            // Future-phase semantic elements — reject explicitly so callers get a clear error
            "parallelGateway"
            | "inclusiveGateway"
            | "eventBasedGateway"
            | "complexGateway"
            | "intermediateThrowEvent"
            | "subProcess"
            | "transaction"
            | "adHocSubProcess"
            | "receiveTask"
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
        if let FlowNodeKind::BoundaryTimerEvent {
            attached_to: host_id,
            ..
        } = &node.kind
        {
            attached_to
                .entry(host_id.clone())
                .or_default()
                .push(node.id.clone());
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

fn validate(
    process_id: &str,
    nodes: &HashMap<String, FlowNode>,
    flows: &[SequenceFlow],
) -> Result<()> {
    let start_count = nodes
        .values()
        .filter(|n| matches!(n.kind, FlowNodeKind::StartEvent))
        .count();

    if start_count == 0 {
        return Err(EngineError::Parse(format!(
            "Process '{process_id}' has no start event"
        )));
    }
    if start_count > 1 {
        return Err(EngineError::Parse(format!(
            "Process '{process_id}' has {start_count} start events; only one supported in Phase 3"
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
        if let FlowNodeKind::BoundaryTimerEvent { attached_to, .. } = &node.kind {
            if !nodes.contains_key(attached_to.as_str()) {
                return Err(EngineError::Parse(format!(
                    "BoundaryEvent '{}' attachedToRef '{}' not found in process",
                    node.id, attached_to
                )));
            }
        }
    }

    Ok(())
}
