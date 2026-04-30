use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum TimerSpec {
    Duration(String),
    Cycle(String),
    Date(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum FlowNodeKind {
    StartEvent,
    TimerStartEvent {
        timer: TimerSpec,
    },
    MessageStartEvent {
        message_name: String,
    },
    EndEvent,
    UserTask,
    ServiceTask {
        topic: Option<String>,
        url: Option<String>,
    },
    ExclusiveGateway {
        default_flow: Option<String>,
    },
    InclusiveGateway {
        default_flow: Option<String>,
    },
    ParallelGateway,
    IntermediateTimerCatchEvent {
        timer: TimerSpec,
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
        timer: TimerSpec,
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
    BoundaryErrorEvent {
        error_code: Option<String>,
        attached_to: String,
        cancelling: bool,
    },
    SubProcess {
        sub_graph: Box<ProcessGraph>,
    },
    BusinessRuleTask {
        decision_ref: String,
    },
    SendTask {
        message_name: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct FlowNode {
    pub id: String,
    pub name: Option<String>,
    pub kind: FlowNodeKind,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SequenceFlow {
    pub id: String,
    pub source_ref: String,
    pub target_ref: String,
    pub condition: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
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
    /// JSON Schema for validating start_instance input variables.
    pub input_schema: Option<serde_json::Value>,
}
