//! Verifies the `#[handler]` proc-macro: generated struct name, topic
//! literal, delegation to the wrapped fn, and `Handler: Send + Sync`.

use std::sync::Arc;

use conduit_worker::{handler, ExternalTask, Handler, HandlerError, HandlerResult, Variable};

#[handler(topic = "http.call")]
async fn http_call(task: &ExternalTask) -> Result<HandlerResult, HandlerError> {
    Ok(HandlerResult::complete(vec![Variable::string(
        "echoed_topic",
        task.topic.clone().unwrap_or_default(),
    )]))
}

#[handler(topic = "policy.check")]
async fn policy_check(_task: &ExternalTask) -> Result<HandlerResult, HandlerError> {
    Ok(HandlerResult::bpmn_error("DENIED", "not allowed"))
}

fn assert_send_sync<T: Send + Sync>() {}

#[tokio::test]
async fn macro_generates_handler_with_topic_and_delegation() {
    assert_send_sync::<HttpCallHandler>();
    assert_send_sync::<PolicyCheckHandler>();

    let h: Arc<dyn Handler> = Arc::new(HttpCallHandler);
    assert_eq!(h.topic(), "http.call");

    let task = ExternalTask {
        id: uuid::Uuid::nil(),
        instance_id: uuid::Uuid::nil(),
        execution_id: uuid::Uuid::nil(),
        topic: Some("http.call".to_string()),
        locked_until: None,
        retries: 3,
        retry_count: 0,
        variables: Vec::new(),
    };
    let result = h.handle(&task).await.expect("handler returned Err");
    match result {
        HandlerResult::Complete { variables } => {
            assert_eq!(variables.len(), 1);
            assert_eq!(variables[0].name, "echoed_topic");
        }
        other => panic!("expected Complete, got {other:?}"),
    }

    let p: Arc<dyn Handler> = Arc::new(PolicyCheckHandler);
    assert_eq!(p.topic(), "policy.check");
    match p.handle(&task).await.unwrap() {
        HandlerResult::BpmnError { code, .. } => assert_eq!(code, "DENIED"),
        other => panic!("expected BpmnError, got {other:?}"),
    }
}
