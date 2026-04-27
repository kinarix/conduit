use conduit::error::EngineError;
use conduit::parser::{self, FlowNodeKind};

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!("tests/fixtures/bpmn/{name}.bpmn"))
        .unwrap_or_else(|_| panic!("Fixture not found: {name}.bpmn"))
}

// ── Valid parses ──────────────────────────────────────────────────────────────

#[test]
fn parse_minimal() {
    let graph = parser::parse(&fixture("minimal")).unwrap();
    assert_eq!(graph.process_id, "minimal_process");
    assert_eq!(graph.nodes.len(), 2);
    assert_eq!(graph.flows.len(), 1);

    let start = graph
        .nodes
        .values()
        .find(|n| n.kind == FlowNodeKind::StartEvent)
        .unwrap();
    let end = graph
        .nodes
        .values()
        .find(|n| n.kind == FlowNodeKind::EndEvent)
        .unwrap();

    assert_eq!(&graph.outgoing[&start.id], &[end.id.clone()]);
    assert_eq!(&graph.incoming[&end.id], &[start.id.clone()]);
}

#[test]
fn parse_simple_user_task() {
    let graph = parser::parse(&fixture("simple_user_task")).unwrap();
    assert_eq!(graph.process_id, "simple_user_task");
    assert_eq!(graph.process_name.as_deref(), Some("Simple User Task"));
    assert_eq!(graph.nodes.len(), 3);
    assert_eq!(graph.flows.len(), 2);

    let task = graph.nodes.get("task_review").unwrap();
    assert_eq!(task.kind, FlowNodeKind::UserTask);
    assert_eq!(task.name.as_deref(), Some("Review Order"));
}

#[test]
fn parse_service_and_user() {
    let graph = parser::parse(&fixture("service_and_user")).unwrap();
    assert_eq!(graph.nodes.len(), 4);

    let svc = graph.nodes.get("task_notify").unwrap();
    match &svc.kind {
        FlowNodeKind::ServiceTask { topic } => {
            assert_eq!(topic.as_deref(), Some("email-sender"));
        }
        _ => panic!("expected ServiceTask"),
    }
}

#[test]
fn parse_complex_subset() {
    let graph = parser::parse(&fixture("complex_subset")).unwrap();
    assert_eq!(graph.process_id, "order_fulfillment");

    // 1 start + 5 tasks + 1 end = 8 nodes, 7 flows
    assert_eq!(graph.nodes.len(), 8);
    assert_eq!(graph.flows.len(), 7);

    // Check two service tasks have topics
    let payment = graph.nodes.get("task_payment").unwrap();
    match &payment.kind {
        FlowNodeKind::ServiceTask { topic } => {
            assert_eq!(topic.as_deref(), Some("payment-validator"))
        }
        _ => panic!("expected ServiceTask"),
    }

    let dispatch = graph.nodes.get("task_dispatch").unwrap();
    match &dispatch.kind {
        FlowNodeKind::ServiceTask { topic } => assert_eq!(topic.as_deref(), Some("logistics")),
        _ => panic!("expected ServiceTask"),
    }
}

#[test]
fn parse_camunda_dialect() {
    let graph = parser::parse(&fixture("camunda_dialect")).unwrap();
    assert_eq!(graph.process_id, "order_process");
    assert_eq!(graph.process_name.as_deref(), Some("Order Process"));

    // 1 start + 2 user tasks + 2 service tasks + 1 end = 6 nodes
    assert_eq!(graph.nodes.len(), 6);
    assert_eq!(graph.flows.len(), 5);

    // Camunda topic extracted from camunda:topic attribute
    let payment = graph.nodes.get("task_payment").unwrap();
    match &payment.kind {
        FlowNodeKind::ServiceTask { topic } => {
            assert_eq!(topic.as_deref(), Some("payment-processor"));
        }
        _ => panic!("expected ServiceTask"),
    }

    let notify = graph.nodes.get("task_notify").unwrap();
    match &notify.kind {
        FlowNodeKind::ServiceTask { topic } => {
            assert_eq!(topic.as_deref(), Some("email-sender"));
        }
        _ => panic!("expected ServiceTask"),
    }

    // Adjacency: start → review → payment → ship → notify → end
    let outgoing_start = &graph.outgoing["start_1"];
    assert_eq!(outgoing_start, &["task_review"]);
}

// ── Validation errors ─────────────────────────────────────────────────────────

#[test]
fn reject_unsupported_gateway() {
    let result = parser::parse(&fixture("unsupported_gateway"));
    assert!(
        matches!(result, Err(EngineError::UnsupportedElement(ref el)) if el == "eventBasedGateway"),
        "expected UnsupportedElement(eventBasedGateway), got: {result:?}"
    );
}

#[test]
fn intermediate_timer_catch_event_is_supported() {
    // Phase 8: ICE is now a supported element; the fixture must parse cleanly.
    let result = parser::parse(&fixture("unsupported_timer"));
    assert!(result.is_ok(), "expected Ok, got: {result:?}");
}

#[test]
fn reject_missing_start_event() {
    let result = parser::parse(&fixture("missing_start_event"));
    assert!(
        matches!(result, Err(EngineError::Parse(_))),
        "expected Parse error for missing start event, got: {result:?}"
    );
    if let Err(EngineError::Parse(msg)) = result {
        assert!(msg.contains("no start event"), "message: {msg}");
    }
}

#[test]
fn reject_missing_end_event() {
    let result = parser::parse(&fixture("missing_end_event"));
    assert!(
        matches!(result, Err(EngineError::Parse(_))),
        "expected Parse error for missing end event, got: {result:?}"
    );
    if let Err(EngineError::Parse(msg)) = result {
        assert!(msg.contains("no end event"), "message: {msg}");
    }
}

#[test]
fn reject_dangling_flow() {
    let result = parser::parse(&fixture("dangling_flow"));
    assert!(
        matches!(result, Err(EngineError::Parse(_))),
        "expected Parse error for dangling flow, got: {result:?}"
    );
    if let Err(EngineError::Parse(msg)) = result {
        assert!(msg.contains("unknown targetRef"), "message: {msg}");
    }
}

// ── Inline edge cases (no fixture file needed) ────────────────────────────────

#[test]
fn reject_invalid_xml() {
    let result = parser::parse("<not valid xml <<>>");
    assert!(
        matches!(result, Err(EngineError::Parse(_))),
        "expected Parse error for malformed XML"
    );
}

#[test]
fn reject_xml_without_process_element() {
    let xml = r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL">
</definitions>"#;
    let result = parser::parse(xml);
    assert!(
        matches!(result, Err(EngineError::Parse(ref m)) if m.contains("No <process>")),
        "expected Parse error about missing process element, got: {result:?}"
    );
}

#[test]
fn service_task_without_topic_is_valid() {
    let xml = r#"<?xml version="1.0"?>
<definitions xmlns="http://www.omg.org/spec/BPMN/20100524/MODEL">
  <process id="p1">
    <startEvent id="s"/>
    <serviceTask id="t"/>
    <endEvent id="e"/>
    <sequenceFlow id="f1" sourceRef="s" targetRef="t"/>
    <sequenceFlow id="f2" sourceRef="t" targetRef="e"/>
  </process>
</definitions>"#;
    let graph = parser::parse(xml).unwrap();
    let svc = graph.nodes.get("t").unwrap();
    assert!(matches!(
        &svc.kind,
        FlowNodeKind::ServiceTask { topic: None }
    ));
}
