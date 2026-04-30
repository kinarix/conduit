use std::collections::HashMap;

use crate::error::{EngineError, Result};
use crate::parser::types::TimerSpec;

pub(super) fn require_id(node: &roxmltree::Node, element: &str) -> Result<String> {
    node.attribute("id")
        .map(|s| s.to_string())
        .ok_or_else(|| EngineError::Parse(format!("<{element}> element missing id attribute")))
}

/// Extract a JSON Schema from <conduit:inputSchema> inside the process's <extensionElements>.
pub(super) fn extract_input_schema(
    process_node: &roxmltree::Node,
    conduit_ns: &str,
) -> Result<Option<serde_json::Value>> {
    for ext in process_node
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "extensionElements")
    {
        for inner in ext.children().filter(|n| n.is_element()) {
            let is_conduit_schema = inner.tag_name().name() == "inputSchema"
                && inner
                    .tag_name()
                    .namespace()
                    .is_some_and(|ns| ns == conduit_ns);
            if is_conduit_schema {
                if let Some(text) = inner.text() {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        let schema = serde_json::from_str(trimmed).map_err(|e| {
                            EngineError::Parse(format!(
                                "conduit:inputSchema is not valid JSON: {e}"
                            ))
                        })?;
                        return Ok(Some(schema));
                    }
                }
            }
        }
    }
    Ok(None)
}

/// Extract the external-task topic from a serviceTask node.
pub(super) fn extract_topic(node: &roxmltree::Node, camunda_ns: &str) -> Option<String> {
    if let Some(t) = node.attribute("topic") {
        return Some(t.to_string());
    }
    if let Some(t) = node.attribute((camunda_ns, "topic")) {
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

pub(super) fn extract_url(node: &roxmltree::Node, camunda_ns: &str) -> Option<String> {
    if let Some(u) = node.attribute("url") {
        return Some(u.to_string());
    }
    if let Some(u) = node.attribute((camunda_ns, "url")) {
        return Some(u.to_string());
    }
    None
}

pub(super) fn extract_timer_spec(node: &roxmltree::Node) -> Result<TimerSpec> {
    for def in node
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "timerEventDefinition")
    {
        for child in def.children().filter(|n| n.is_element()) {
            let text = child
                .text()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            match (child.tag_name().name(), text) {
                ("timeCycle", Some(s)) => return Ok(TimerSpec::Cycle(s)),
                ("timeDate", Some(s)) => return Ok(TimerSpec::Date(s)),
                ("timeDuration", Some(s)) => return Ok(TimerSpec::Duration(s)),
                _ => {}
            }
        }
    }
    Err(EngineError::Parse(
        "Timer event missing timerEventDefinition with timeDuration/timeCycle/timeDate".to_string(),
    ))
}

pub(super) fn extract_condition(node: &roxmltree::Node) -> Option<String> {
    node.children()
        .find(|n| n.is_element() && n.tag_name().name() == "conditionExpression")
        .and_then(|n| n.text())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Resolve the message name for an event node that contains a `<messageEventDefinition>`.
pub(super) fn extract_message_name(
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

    if let Some(msg_ref) = def.attribute("messageRef") {
        let bare_id = msg_ref.split(':').next_back().unwrap_or(msg_ref);
        if let Some(name) = message_defs.get(bare_id) {
            return Ok(Some(name.clone()));
        }
        return Ok(Some(bare_id.to_string()));
    }

    Err(EngineError::Parse(format!(
        "messageEventDefinition on '{element_id}' is missing messageRef attribute"
    )))
}

/// Extract the correlation key expression from a message event node.
pub(super) fn extract_correlation_key(node: &roxmltree::Node, camunda_ns: &str) -> Option<String> {
    if let Some(v) = node.attribute("correlationKey") {
        return Some(v.to_string());
    }
    if let Some(v) = node.attribute((camunda_ns, "correlationKey")) {
        return Some(v.to_string());
    }
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
pub(super) fn extract_signal_name(
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

/// Extract error code for a boundary error event.
pub(super) fn extract_error_code(
    node: &roxmltree::Node,
    error_defs: &HashMap<String, String>,
) -> Option<Option<String>> {
    let def = node
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "errorEventDefinition")?;

    if let Some(error_ref) = def.attribute("errorRef") {
        let bare_id = error_ref.split(':').next_back().unwrap_or(error_ref);
        let code = error_defs.get(bare_id).map(|c| c.as_str()).unwrap_or(bare_id);
        if code.is_empty() {
            return Some(None);
        }
        return Some(Some(code.to_string()));
    }

    Some(None)
}
