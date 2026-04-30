use chrono::{DateTime, Duration, Utc};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::error::{EngineError, Result};
use crate::parser::{FlowNodeKind, ProcessGraph, TimerSpec};

use super::Engine;

/// Supported forms: PT<n>S, PT<n>M, PT<n>H, P<n>D, and combinations like PT1H30M.
pub fn parse_duration(s: &str) -> Result<Duration> {
    if s.is_empty() {
        return Err(EngineError::Parse("Empty duration string".to_string()));
    }

    let s = s.trim();
    let mut chars = s.chars().peekable();

    if chars.next() != Some('P') {
        return Err(EngineError::Parse(format!(
            "Invalid ISO 8601 duration: '{s}'"
        )));
    }

    let mut total_secs: i64 = 0;
    let mut in_time = false;
    let mut num_buf = String::new();
    let mut parsed_any = false;

    for ch in chars {
        match ch {
            'T' => {
                in_time = true;
            }
            '0'..='9' => {
                num_buf.push(ch);
            }
            'D' if !in_time => {
                let n: i64 = num_buf.parse().map_err(|_| {
                    EngineError::Parse(format!("Invalid number in duration: '{s}'"))
                })?;
                total_secs += n * 86_400;
                num_buf.clear();
                parsed_any = true;
            }
            'H' if in_time => {
                let n: i64 = num_buf.parse().map_err(|_| {
                    EngineError::Parse(format!("Invalid number in duration: '{s}'"))
                })?;
                total_secs += n * 3_600;
                num_buf.clear();
                parsed_any = true;
            }
            'M' if in_time => {
                let n: i64 = num_buf.parse().map_err(|_| {
                    EngineError::Parse(format!("Invalid number in duration: '{s}'"))
                })?;
                total_secs += n * 60;
                num_buf.clear();
                parsed_any = true;
            }
            'S' if in_time => {
                let n: i64 = num_buf.parse().map_err(|_| {
                    EngineError::Parse(format!("Invalid number in duration: '{s}'"))
                })?;
                total_secs += n;
                num_buf.clear();
                parsed_any = true;
            }
            _ => {
                return Err(EngineError::Parse(format!(
                    "Unexpected character '{ch}' in duration: '{s}'"
                )));
            }
        }
    }

    if !parsed_any {
        return Err(EngineError::Parse(format!(
            "No duration components found in: '{s}'"
        )));
    }

    Ok(Duration::seconds(total_secs))
}

pub(super) fn parse_date(s: &str) -> Result<DateTime<Utc>> {
    s.trim()
        .parse::<DateTime<Utc>>()
        .or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(s.trim(), "%Y-%m-%dT%H:%M:%S")
                .map(|dt| dt.and_utc())
        })
        .map_err(|_| EngineError::Parse(format!("Invalid timeDate: '{s}'")))
}

/// Returns `(repetitions_remaining, interval)`. `None` repetitions = infinite.
pub(super) fn parse_cycle(s: &str) -> Result<(Option<i32>, Duration)> {
    let s = s.trim();
    if !s.starts_with('R') {
        return Err(EngineError::Parse(format!("Invalid timeCycle: '{s}'")));
    }
    let rest = &s[1..];
    let slash = rest.find('/').ok_or_else(|| {
        EngineError::Parse(format!("Invalid timeCycle (no /): '{s}'"))
    })?;
    let count_str = &rest[..slash];
    let duration_str = &rest[slash + 1..];
    let repetitions = if count_str.is_empty() || count_str == "0" {
        None
    } else {
        Some(count_str.parse::<i32>().map_err(|_| {
            EngineError::Parse(format!("Invalid cycle count: '{count_str}'"))
        })?)
    };
    Ok((repetitions, parse_duration(duration_str)?))
}

/// Schedule a timer spec. Returns `(due_date, timer_expression, repetitions_remaining)`.
pub(super) fn timer_spec_schedule(
    spec: &TimerSpec,
) -> Result<(DateTime<Utc>, Option<String>, Option<i32>)> {
    match spec {
        TimerSpec::Duration(d) => Ok((Utc::now() + parse_duration(d)?, None, None)),
        TimerSpec::Cycle(expr) => {
            let (reps, interval) = parse_cycle(expr)?;
            Ok((Utc::now() + interval, Some(expr.clone()), reps))
        }
        TimerSpec::Date(dt_str) => Ok((parse_date(dt_str)?, Some(dt_str.clone()), Some(1))),
    }
}

impl Engine {
    pub(super) fn element_type_str(kind: &FlowNodeKind) -> &'static str {
        match kind {
            FlowNodeKind::StartEvent => "startEvent",
            FlowNodeKind::MessageStartEvent { .. } => "startEvent",
            FlowNodeKind::EndEvent => "endEvent",
            FlowNodeKind::UserTask => "userTask",
            FlowNodeKind::ServiceTask { .. } => "serviceTask",
            FlowNodeKind::ExclusiveGateway { .. } => "exclusiveGateway",
            FlowNodeKind::InclusiveGateway { .. } => "inclusiveGateway",
            FlowNodeKind::ParallelGateway => "parallelGateway",
            FlowNodeKind::IntermediateTimerCatchEvent { .. } => "intermediateCatchEvent",
            FlowNodeKind::IntermediateMessageCatchEvent { .. } => "intermediateCatchEvent",
            FlowNodeKind::ReceiveTask { .. } => "receiveTask",
            FlowNodeKind::BoundaryTimerEvent { .. } => "boundaryEvent",
            FlowNodeKind::SignalStartEvent { .. } => "startEvent",
            FlowNodeKind::IntermediateSignalCatchEvent { .. } => "intermediateCatchEvent",
            FlowNodeKind::BoundarySignalEvent { .. } => "boundaryEvent",
            FlowNodeKind::BoundaryErrorEvent { .. } => "boundaryErrorEvent",
            FlowNodeKind::SubProcess { .. } => "subProcess",
            FlowNodeKind::BusinessRuleTask { .. } => "businessRuleTask",
            FlowNodeKind::SendTask { .. } => "sendTask",
            FlowNodeKind::TimerStartEvent { .. } => "startEvent",
        }
    }

    /// Recursively search the graph hierarchy for the sub-graph containing `element_id`.
    /// Returns `(containing_graph, outer_chain)` where `outer_chain` is the list of
    /// ancestor graphs from outermost to innermost (empty if the element is in `graph` itself).
    pub(super) fn find_element_graph<'g>(
        element_id: &str,
        graph: &'g ProcessGraph,
    ) -> Option<(&'g ProcessGraph, Vec<&'g ProcessGraph>)> {
        Self::find_element_graph_inner(element_id, graph, &mut vec![])
    }

    pub(super) fn find_element_graph_inner<'g>(
        element_id: &str,
        graph: &'g ProcessGraph,
        outer: &mut Vec<&'g ProcessGraph>,
    ) -> Option<(&'g ProcessGraph, Vec<&'g ProcessGraph>)> {
        if graph.nodes.contains_key(element_id) {
            return Some((graph, outer.clone()));
        }
        for node in graph.nodes.values() {
            if let FlowNodeKind::SubProcess { sub_graph } = &node.kind {
                outer.push(graph);
                if let Some(result) = Self::find_element_graph_inner(element_id, sub_graph, outer) {
                    return Some(result);
                }
                outer.pop();
            }
        }
        None
    }

    /// Resolve a correlation key expression against current instance variables.
    /// Supports `${varName}` variable references; any other value is used as a literal.
    pub(super) async fn resolve_correlation_key(
        expr: &str,
        instance_id: Uuid,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> Result<Option<String>> {
        let trimmed = expr.trim();
        if trimmed.starts_with("${") && trimmed.ends_with('}') {
            let var_name = &trimmed[2..trimmed.len() - 1];
            let row: Option<(JsonValue,)> = sqlx::query_as(
                "SELECT value FROM variables WHERE instance_id = $1 AND name = $2 LIMIT 1",
            )
            .bind(instance_id)
            .bind(var_name)
            .fetch_optional(&mut **tx)
            .await?;
            return Ok(row.map(|(val,)| match val {
                JsonValue::String(s) => s,
                other => other.to_string(),
            }));
        }
        Ok(Some(trimmed.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_cycle, parse_date, parse_duration, timer_spec_schedule};
    use crate::engine::Engine;
    use crate::parser::types::{FlowNode, FlowNodeKind, ProcessGraph};
    use crate::parser::TimerSpec;
    use chrono::Utc;
    use std::collections::HashMap;

    // ── parse_duration ────────────────────────────────────────────────────────

    #[test]
    fn duration_seconds() {
        assert_eq!(parse_duration("PT30S").unwrap().num_seconds(), 30);
    }

    #[test]
    fn duration_minutes() {
        assert_eq!(parse_duration("PT5M").unwrap().num_seconds(), 300);
    }

    #[test]
    fn duration_hours() {
        assert_eq!(parse_duration("PT2H").unwrap().num_seconds(), 7200);
    }

    #[test]
    fn duration_days() {
        assert_eq!(parse_duration("P1D").unwrap().num_seconds(), 86_400);
    }

    #[test]
    fn duration_combined() {
        assert_eq!(parse_duration("PT1H30M").unwrap().num_seconds(), 5400);
    }

    #[test]
    fn duration_days_and_hours() {
        assert_eq!(parse_duration("P2DT3H").unwrap().num_seconds(), 2 * 86_400 + 3 * 3_600);
    }

    #[test]
    fn duration_empty_is_err() {
        assert!(parse_duration("").is_err());
    }

    #[test]
    fn duration_no_p_prefix_is_err() {
        assert!(parse_duration("T30S").is_err());
    }

    #[test]
    fn duration_no_components_is_err() {
        assert!(parse_duration("P").is_err());
    }

    #[test]
    fn duration_invalid_char_is_err() {
        assert!(parse_duration("PT5X").is_err());
    }

    // ── parse_date ────────────────────────────────────────────────────────────

    #[test]
    fn date_rfc3339() {
        let dt = parse_date("2025-06-15T10:00:00Z").unwrap();
        assert_eq!(dt.format("%Y-%m-%d").to_string(), "2025-06-15");
    }

    #[test]
    fn date_naive_no_offset() {
        let dt = parse_date("2030-01-01T00:00:00").unwrap();
        assert_eq!(dt.format("%Y").to_string(), "2030");
    }

    #[test]
    fn date_invalid_is_err() {
        assert!(parse_date("not-a-date").is_err());
    }

    // ── parse_cycle ───────────────────────────────────────────────────────────

    #[test]
    fn cycle_finite() {
        let (reps, interval) = parse_cycle("R3/PT1M").unwrap();
        assert_eq!(reps, Some(3));
        assert_eq!(interval.num_seconds(), 60);
    }

    #[test]
    fn cycle_infinite_empty_count() {
        let (reps, interval) = parse_cycle("R/PT5S").unwrap();
        assert!(reps.is_none());
        assert_eq!(interval.num_seconds(), 5);
    }

    #[test]
    fn cycle_infinite_zero_count() {
        let (reps, interval) = parse_cycle("R0/PT10S").unwrap();
        assert!(reps.is_none());
        assert_eq!(interval.num_seconds(), 10);
    }

    #[test]
    fn cycle_no_r_prefix_is_err() {
        assert!(parse_cycle("3/PT1M").is_err());
    }

    #[test]
    fn cycle_no_slash_is_err() {
        assert!(parse_cycle("R3PT1M").is_err());
    }

    // ── timer_spec_schedule ───────────────────────────────────────────────────

    #[test]
    fn schedule_duration_is_in_future() {
        let before = Utc::now();
        let (due, expr, reps) = timer_spec_schedule(&TimerSpec::Duration("PT1H".to_string())).unwrap();
        assert!(due > before);
        assert!(expr.is_none());
        assert!(reps.is_none());
    }

    #[test]
    fn schedule_cycle_returns_expr_and_reps() {
        let (due, expr, reps) = timer_spec_schedule(&TimerSpec::Cycle("R3/PT5M".to_string())).unwrap();
        let before = Utc::now();
        assert!(due < before + chrono::Duration::hours(1));
        assert_eq!(expr.as_deref(), Some("R3/PT5M"));
        assert_eq!(reps, Some(3));
    }

    #[test]
    fn schedule_date_returns_parsed_instant() {
        let (due, expr, reps) = timer_spec_schedule(&TimerSpec::Date("2040-01-01T00:00:00Z".to_string())).unwrap();
        assert_eq!(due.format("%Y").to_string(), "2040");
        assert!(expr.is_some());
        assert_eq!(reps, Some(1));
    }

    // ── Engine::element_type_str ──────────────────────────────────────────────

    #[test]
    fn element_type_str_spot_check() {
        assert_eq!(Engine::element_type_str(&FlowNodeKind::StartEvent), "startEvent");
        assert_eq!(Engine::element_type_str(&FlowNodeKind::EndEvent), "endEvent");
        assert_eq!(Engine::element_type_str(&FlowNodeKind::UserTask), "userTask");
        assert_eq!(Engine::element_type_str(&FlowNodeKind::ParallelGateway), "parallelGateway");
        assert_eq!(Engine::element_type_str(&FlowNodeKind::ServiceTask { topic: None, url: None }), "serviceTask");
        assert_eq!(Engine::element_type_str(&FlowNodeKind::SubProcess { sub_graph: Box::new(empty_graph("sub")) }), "subProcess");
        assert_eq!(Engine::element_type_str(&FlowNodeKind::TimerStartEvent { timer: TimerSpec::Duration("PT1S".to_string()) }), "startEvent");
    }

    // ── Engine::find_element_graph ────────────────────────────────────────────

    fn node(id: &str) -> FlowNode {
        FlowNode { id: id.to_string(), name: None, kind: FlowNodeKind::UserTask }
    }

    fn empty_graph(id: &str) -> ProcessGraph {
        ProcessGraph {
            process_id: id.to_string(),
            process_name: None,
            nodes: HashMap::new(),
            flows: vec![],
            outgoing: HashMap::new(),
            incoming: HashMap::new(),
            attached_to: HashMap::new(),
            input_schema: None,
        }
    }

    fn graph_with_nodes(id: &str, node_ids: &[&str]) -> ProcessGraph {
        let mut nodes = HashMap::new();
        for &nid in node_ids {
            nodes.insert(nid.to_string(), node(nid));
        }
        ProcessGraph { process_id: id.to_string(), process_name: None, nodes, flows: vec![], outgoing: HashMap::new(), incoming: HashMap::new(), attached_to: HashMap::new(), input_schema: None }
    }

    #[test]
    fn find_element_in_root_graph() {
        let graph = graph_with_nodes("proc", &["task1", "task2"]);
        let (found_graph, chain) = Engine::find_element_graph("task1", &graph).unwrap();
        assert_eq!(found_graph.process_id, "proc");
        assert!(chain.is_empty());
    }

    #[test]
    fn find_element_not_present_returns_none() {
        let graph = graph_with_nodes("proc", &["task1"]);
        assert!(Engine::find_element_graph("task_missing", &graph).is_none());
    }

    #[test]
    fn find_element_in_subprocess() {
        let inner = graph_with_nodes("sub", &["inner_task"]);
        let mut outer_nodes = HashMap::new();
        outer_nodes.insert("outer_task".to_string(), node("outer_task"));
        outer_nodes.insert(
            "sub1".to_string(),
            FlowNode {
                id: "sub1".to_string(),
                name: None,
                kind: FlowNodeKind::SubProcess { sub_graph: Box::new(inner) },
            },
        );
        let outer = ProcessGraph {
            process_id: "proc".to_string(),
            process_name: None,
            nodes: outer_nodes,
            flows: vec![],
            outgoing: HashMap::new(),
            incoming: HashMap::new(),
            attached_to: HashMap::new(),
            input_schema: None,
        };

        let (found_graph, chain) = Engine::find_element_graph("inner_task", &outer).unwrap();
        assert_eq!(found_graph.process_id, "sub");
        assert_eq!(chain.len(), 1);
        assert_eq!(chain[0].process_id, "proc");
    }

    #[test]
    fn find_element_empty_graph_returns_none() {
        let graph = empty_graph("proc");
        assert!(Engine::find_element_graph("any", &graph).is_none());
    }
}
