mod extract;
pub mod types;
mod validate;

pub use types::{FlowNode, FlowNodeKind, ProcessGraph, SequenceFlow, TimerSpec};

use roxmltree::Document;
use std::collections::HashMap;

use crate::error::{EngineError, Result};
use extract::{
    extract_condition, extract_correlation_key, extract_error_code, extract_http_config,
    extract_input_schema, extract_message_name, extract_result_variable, extract_signal_name,
    extract_timer_spec, extract_topic, extract_url, require_id,
};
use validate::validate;

const BPMN_NS: &str = "http://www.omg.org/spec/BPMN/20100524/MODEL";
const CAMUNDA_NS: &str = "http://activiti.org/bpmn";
const CONDUIT_NS: &str = "http://conduit.io/ext";

/// Parse BPMN XML into a validated `ProcessGraph`.
pub fn parse(xml: &str) -> Result<ProcessGraph> {
    let doc = Document::parse(xml).map_err(|e| EngineError::Parse(format!("Invalid XML: {e}")))?;

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

    let error_defs: HashMap<String, String> = doc
        .root_element()
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "error")
        .filter_map(|n| {
            let id = n.attribute("id")?;
            let code = n.attribute("errorCode").unwrap_or("");
            Some((id.to_string(), code.to_string()))
        })
        .collect();

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

    let input_schema = extract_input_schema(&process_node, CONDUIT_NS)?;

    let (nodes, flows) = parse_children(&process_node, &message_defs, &signal_defs, &error_defs)?;

    validate(&process_id, &nodes, &flows)?;

    Ok(build_graph(
        process_id,
        process_name,
        nodes,
        flows,
        input_schema,
    ))
}

fn parse_children(
    container: &roxmltree::Node,
    message_defs: &HashMap<String, String>,
    signal_defs: &HashMap<String, String>,
    error_defs: &HashMap<String, String>,
) -> Result<(HashMap<String, FlowNode>, Vec<SequenceFlow>)> {
    let mut nodes: HashMap<String, FlowNode> = HashMap::new();
    let mut flows: Vec<SequenceFlow> = Vec::new();

    for child in container.children().filter(|n| n.is_element()) {
        let local = child.tag_name().name();
        let ns = child.tag_name().namespace();

        if let Some(ns_uri) = ns {
            if ns_uri != BPMN_NS {
                continue;
            }
        }

        match local {
            "startEvent" => {
                let id = require_id(&child, local)?;
                let name = child.attribute("name").map(|s| s.to_string());
                let kind = if let Some(msg_name) = extract_message_name(&child, message_defs, &id)?
                {
                    FlowNodeKind::MessageStartEvent {
                        message_name: msg_name,
                    }
                } else if let Some(sig_name) = extract_signal_name(&child, signal_defs, &id)? {
                    FlowNodeKind::SignalStartEvent {
                        signal_name: sig_name,
                    }
                } else if child
                    .children()
                    .any(|n| n.is_element() && n.tag_name().name() == "timerEventDefinition")
                {
                    FlowNodeKind::TimerStartEvent {
                        timer: extract_timer_spec(&child)?,
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
                let topic = extract_topic(&child, CONDUIT_NS, CAMUNDA_NS);
                let url = extract_url(&child, CAMUNDA_NS);
                let http = extract_http_config(&child, CONDUIT_NS)?;
                nodes.insert(
                    id.clone(),
                    FlowNode {
                        id,
                        name,
                        kind: FlowNodeKind::ServiceTask { topic, url, http },
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
            "inclusiveGateway" => {
                let id = require_id(&child, local)?;
                let name = child.attribute("name").map(|s| s.to_string());
                let default_flow = child.attribute("default").map(|s| s.to_string());
                nodes.insert(
                    id.clone(),
                    FlowNode {
                        id,
                        name,
                        kind: FlowNodeKind::InclusiveGateway { default_flow },
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
                        timer: extract_timer_spec(&child)?,
                    }
                } else if let Some(msg_name) = extract_message_name(&child, message_defs, &id)? {
                    let correlation_key_expr = extract_correlation_key(&child, CAMUNDA_NS);
                    FlowNodeKind::IntermediateMessageCatchEvent {
                        message_name: msg_name,
                        correlation_key_expr,
                    }
                } else if let Some(sig_name) = extract_signal_name(&child, signal_defs, &id)? {
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
                    let timer = extract_timer_spec(&child)?;
                    if cancelling {
                        if let TimerSpec::Cycle(_) = &timer {
                            return Err(EngineError::Parse(format!(
                                "Boundary timer '{id}': interrupting boundary event cannot use timeCycle"
                            )));
                        }
                    }
                    FlowNodeKind::BoundaryTimerEvent {
                        timer,
                        attached_to,
                        cancelling,
                    }
                } else if let Some(sig_name) = extract_signal_name(&child, signal_defs, &id)? {
                    FlowNodeKind::BoundarySignalEvent {
                        signal_name: sig_name,
                        attached_to,
                        cancelling,
                    }
                } else if let Some(error_code) = extract_error_code(&child, error_defs) {
                    FlowNodeKind::BoundaryErrorEvent {
                        error_code,
                        attached_to,
                        cancelling,
                    }
                } else {
                    return Err(EngineError::Parse(format!(
                        "boundaryEvent '{id}' has no supported event definition (timer, signal, or error)"
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
                    extract_message_name(&child, message_defs, &id)?.ok_or_else(|| {
                        EngineError::Parse(format!(
                            "receiveTask '{id}' missing messageRef or message definition"
                        ))
                    })?;
                let correlation_key_expr = extract_correlation_key(&child, CAMUNDA_NS);
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
            "subProcess" => {
                let id = require_id(&child, local)?;
                let name = child.attribute("name").map(|s| s.to_string());
                let (inner_nodes, inner_flows) =
                    parse_children(&child, message_defs, signal_defs, error_defs)?;
                validate(&id, &inner_nodes, &inner_flows)?;
                let sub_graph =
                    build_graph(id.clone(), name.clone(), inner_nodes, inner_flows, None);
                nodes.insert(
                    id.clone(),
                    FlowNode {
                        id,
                        name,
                        kind: FlowNodeKind::SubProcess {
                            sub_graph: Box::new(sub_graph),
                        },
                    },
                );
            }
            "businessRuleTask" => {
                let id = require_id(&child, local)?;
                let name = child.attribute("name").map(|s| s.to_string());
                let decision_ref = child
                    .attribute((CAMUNDA_NS, "decisionRef"))
                    .ok_or_else(|| {
                        EngineError::Parse(format!(
                            "businessRuleTask '{id}' missing camunda:decisionRef"
                        ))
                    })?
                    .to_string();
                nodes.insert(
                    id.clone(),
                    FlowNode {
                        id,
                        name,
                        kind: FlowNodeKind::BusinessRuleTask { decision_ref },
                    },
                );
            }
            "sendTask" => {
                let id = require_id(&child, local)?;
                let name = child.attribute("name").map(|s| s.to_string());
                let message_name =
                    extract_message_name(&child, message_defs, &id)?.ok_or_else(|| {
                        EngineError::Parse(format!(
                            "sendTask '{id}' missing messageRef or message definition"
                        ))
                    })?;
                nodes.insert(
                    id.clone(),
                    FlowNode {
                        id,
                        name,
                        kind: FlowNodeKind::SendTask { message_name },
                    },
                );
            }
            "scriptTask" => {
                let id = require_id(&child, local)?;
                let name = child.attribute("name").map(|s| s.to_string());
                let script = child
                    .children()
                    .find(|n| n.is_element() && n.tag_name().name() == "script")
                    .and_then(|n| n.text().map(|s| s.trim().to_string()))
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| {
                        EngineError::Parse(format!(
                            "scriptTask '{id}' missing <script> child element"
                        ))
                    })?;
                let result_variable = extract_result_variable(&child, CONDUIT_NS);
                nodes.insert(
                    id.clone(),
                    FlowNode {
                        id,
                        name,
                        kind: FlowNodeKind::ScriptTask {
                            script,
                            result_variable,
                        },
                    },
                );
            }

            // Future-phase elements — reject explicitly
            "eventBasedGateway"
            | "complexGateway"
            | "intermediateThrowEvent"
            | "transaction"
            | "adHocSubProcess"
            | "manualTask"
            | "callActivity" => return Err(EngineError::UnsupportedElement(local.to_string())),

            // Non-semantic / presentation elements — safe to ignore
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

    Ok((nodes, flows))
}

#[cfg(test)]
mod tests {
    use super::parse;
    use crate::parser::types::FlowNodeKind;

    fn minimal_bpmn(extra: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="def1">
  <process id="proc1" name="Test">
    <startEvent id="start"/>
    <endEvent id="end"/>
    <sequenceFlow id="f1" sourceRef="start" targetRef="end"/>
    {extra}
  </process>
</definitions>"#
        )
    }

    // ── happy path ────────────────────────────────────────────────────────────

    #[test]
    fn parses_minimal_process() {
        let g = parse(&minimal_bpmn("")).unwrap();
        assert_eq!(g.process_id, "proc1");
        assert_eq!(g.process_name.as_deref(), Some("Test"));
        assert!(g.nodes.contains_key("start"));
        assert!(g.nodes.contains_key("end"));
        assert_eq!(g.flows.len(), 1);
    }

    #[test]
    fn outgoing_and_incoming_built() {
        let g = parse(&minimal_bpmn("")).unwrap();
        assert_eq!(g.outgoing["start"], vec!["end"]);
        assert_eq!(g.incoming["end"], vec!["start"]);
    }

    #[test]
    fn parses_service_task() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="d1">
  <process id="p1">
    <startEvent id="start"/>
    <serviceTask id="svc" xmlns:camunda="http://activiti.org/bpmn" camunda:topic="my-topic"/>
    <endEvent id="end"/>
    <sequenceFlow id="f1" sourceRef="start" targetRef="svc"/>
    <sequenceFlow id="f2" sourceRef="svc" targetRef="end"/>
  </process>
</definitions>"#;
        let g = parse(xml).unwrap();
        let svc = &g.nodes["svc"];
        assert!(
            matches!(&svc.kind, FlowNodeKind::ServiceTask { topic: Some(t), .. } if t == "my-topic")
        );
    }

    #[test]
    fn parses_exclusive_gateway() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="d1">
  <process id="p1">
    <startEvent id="start"/>
    <exclusiveGateway id="gw" default="f3"/>
    <endEvent id="end1"/>
    <endEvent id="end2"/>
    <sequenceFlow id="f1" sourceRef="start" targetRef="gw"/>
    <sequenceFlow id="f2" sourceRef="gw" targetRef="end1"><conditionExpression>x &gt; 0</conditionExpression></sequenceFlow>
    <sequenceFlow id="f3" sourceRef="gw" targetRef="end2"/>
  </process>
</definitions>"#;
        let g = parse(xml).unwrap();
        let gw = &g.nodes["gw"];
        assert!(
            matches!(&gw.kind, FlowNodeKind::ExclusiveGateway { default_flow: Some(f) } if f == "f3")
        );
        let flow_with_cond = g.flows.iter().find(|f| f.id == "f2").unwrap();
        assert_eq!(flow_with_cond.condition.as_deref(), Some("x > 0"));
    }

    #[test]
    fn parses_parallel_gateway() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="d1">
  <process id="p1">
    <startEvent id="start"/>
    <parallelGateway id="fork"/>
    <userTask id="t1"/>
    <userTask id="t2"/>
    <parallelGateway id="join"/>
    <endEvent id="end"/>
    <sequenceFlow id="f1" sourceRef="start" targetRef="fork"/>
    <sequenceFlow id="f2" sourceRef="fork" targetRef="t1"/>
    <sequenceFlow id="f3" sourceRef="fork" targetRef="t2"/>
    <sequenceFlow id="f4" sourceRef="t1" targetRef="join"/>
    <sequenceFlow id="f5" sourceRef="t2" targetRef="join"/>
    <sequenceFlow id="f6" sourceRef="join" targetRef="end"/>
  </process>
</definitions>"#;
        let g = parse(xml).unwrap();
        assert!(matches!(
            g.nodes["fork"].kind,
            FlowNodeKind::ParallelGateway
        ));
    }

    #[test]
    fn parses_message_start_event() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="d1">
  <message id="msg1" name="OrderReceived"/>
  <process id="p1">
    <startEvent id="start">
      <messageEventDefinition messageRef="msg1"/>
    </startEvent>
    <endEvent id="end"/>
    <sequenceFlow id="f1" sourceRef="start" targetRef="end"/>
  </process>
</definitions>"#;
        let g = parse(xml).unwrap();
        assert!(
            matches!(&g.nodes["start"].kind, FlowNodeKind::MessageStartEvent { message_name: n } if n == "OrderReceived")
        );
    }

    #[test]
    fn parses_timer_intermediate_event() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="d1">
  <process id="p1">
    <startEvent id="start"/>
    <intermediateCatchEvent id="timer">
      <timerEventDefinition><timeDuration>PT5M</timeDuration></timerEventDefinition>
    </intermediateCatchEvent>
    <endEvent id="end"/>
    <sequenceFlow id="f1" sourceRef="start" targetRef="timer"/>
    <sequenceFlow id="f2" sourceRef="timer" targetRef="end"/>
  </process>
</definitions>"#;
        let g = parse(xml).unwrap();
        assert!(matches!(
            &g.nodes["timer"].kind,
            FlowNodeKind::IntermediateTimerCatchEvent { .. }
        ));
    }

    #[test]
    fn parses_boundary_timer_event() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="d1">
  <process id="p1">
    <startEvent id="start"/>
    <userTask id="task"/>
    <boundaryEvent id="bt" attachedToRef="task">
      <timerEventDefinition><timeDuration>PT10M</timeDuration></timerEventDefinition>
    </boundaryEvent>
    <endEvent id="end"/>
    <endEvent id="end2"/>
    <sequenceFlow id="f1" sourceRef="start" targetRef="task"/>
    <sequenceFlow id="f2" sourceRef="task" targetRef="end"/>
    <sequenceFlow id="f3" sourceRef="bt" targetRef="end2"/>
  </process>
</definitions>"#;
        let g = parse(xml).unwrap();
        assert!(
            matches!(&g.nodes["bt"].kind, FlowNodeKind::BoundaryTimerEvent { attached_to, .. } if attached_to == "task")
        );
        assert_eq!(g.attached_to["task"], vec!["bt"]);
    }

    #[test]
    fn parses_subprocess() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="d1">
  <process id="p1">
    <startEvent id="start"/>
    <subProcess id="sub">
      <startEvent id="inner_start"/>
      <endEvent id="inner_end"/>
      <sequenceFlow id="if1" sourceRef="inner_start" targetRef="inner_end"/>
    </subProcess>
    <endEvent id="end"/>
    <sequenceFlow id="f1" sourceRef="start" targetRef="sub"/>
    <sequenceFlow id="f2" sourceRef="sub" targetRef="end"/>
  </process>
</definitions>"#;
        let g = parse(xml).unwrap();
        if let FlowNodeKind::SubProcess { sub_graph } = &g.nodes["sub"].kind {
            assert!(sub_graph.nodes.contains_key("inner_start"));
        } else {
            panic!("expected SubProcess");
        }
    }

    #[test]
    fn parses_input_schema_from_extension_elements() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="d1">
  <process id="p1">
    <extensionElements>
      <conduit:inputSchema xmlns:conduit="http://conduit.io/ext">{"type":"object"}</conduit:inputSchema>
    </extensionElements>
    <startEvent id="start"/>
    <endEvent id="end"/>
    <sequenceFlow id="f1" sourceRef="start" targetRef="end"/>
  </process>
</definitions>"#;
        let g = parse(xml).unwrap();
        assert!(g.input_schema.is_some());
    }

    // ── error cases ───────────────────────────────────────────────────────────

    #[test]
    fn bad_xml_is_err() {
        assert!(parse("<not-closed").is_err());
    }

    #[test]
    fn no_process_element_is_err() {
        let xml = r#"<?xml version="1.0"?><definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL"/>"#;
        assert!(parse(xml).is_err());
    }

    #[test]
    fn process_missing_id_is_err() {
        let xml = r#"<?xml version="1.0"?><definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL"><process><startEvent id="s"/><endEvent id="e"/><sequenceFlow id="f" sourceRef="s" targetRef="e"/></process></definitions>"#;
        assert!(parse(xml).is_err());
    }

    #[test]
    fn unsupported_element_is_err() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="d1">
  <process id="p1">
    <startEvent id="start"/>
    <callActivity id="ca"/>
    <endEvent id="end"/>
    <sequenceFlow id="f1" sourceRef="start" targetRef="end"/>
  </process>
</definitions>"#;
        assert!(parse(xml).is_err());
    }

    #[test]
    fn intermediate_catch_event_without_definition_is_err() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="d1">
  <process id="p1">
    <startEvent id="start"/>
    <intermediateCatchEvent id="ice"/>
    <endEvent id="end"/>
    <sequenceFlow id="f1" sourceRef="start" targetRef="ice"/>
    <sequenceFlow id="f2" sourceRef="ice" targetRef="end"/>
  </process>
</definitions>"#;
        assert!(parse(xml).is_err());
    }

    #[test]
    fn business_rule_task_without_decision_ref_is_err() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL" id="d1">
  <process id="p1">
    <startEvent id="start"/>
    <businessRuleTask id="brt"/>
    <endEvent id="end"/>
    <sequenceFlow id="f1" sourceRef="start" targetRef="brt"/>
    <sequenceFlow id="f2" sourceRef="brt" targetRef="end"/>
  </process>
</definitions>"#;
        assert!(parse(xml).is_err());
    }
}

fn build_graph(
    process_id: String,
    process_name: Option<String>,
    nodes: HashMap<String, FlowNode>,
    flows: Vec<SequenceFlow>,
    input_schema: Option<serde_json::Value>,
) -> ProcessGraph {
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
            FlowNodeKind::BoundaryErrorEvent { attached_to: h, .. } => Some(h.clone()),
            _ => None,
        };
        if let Some(h) = host_id {
            attached_to.entry(h).or_default().push(node.id.clone());
        }
    }

    ProcessGraph {
        process_id,
        process_name,
        nodes,
        flows,
        outgoing,
        incoming,
        attached_to,
        input_schema,
    }
}
