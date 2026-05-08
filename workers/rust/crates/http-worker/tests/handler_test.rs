// Integration tests for HttpHandler against a mockito-backed upstream.
// Exercises the request shape, idempotency-key header, response mapping,
// and the BPMN-error-on-4xx path.

use std::collections::BTreeMap;

// The handler module is `pub mod handler` inside the `http-worker` binary,
// not a library — for testing we re-include the source. The config and
// render modules are pulled in solely so handler.rs's `use` statements
// resolve; many of their items aren't referenced from the test code itself.
#[allow(dead_code)]
#[path = "../src/config.rs"]
mod config;
#[allow(dead_code)]
#[path = "../src/handler.rs"]
mod handler;
#[allow(dead_code)]
#[path = "../src/render.rs"]
mod render;

use conduit_worker::{ExternalTask, Handler, HandlerResult, Variable};
use handler::HttpHandler;
use uuid::Uuid;

fn task_with_vars(vars: Vec<Variable>) -> ExternalTask {
    let json_str = format!(
        r#"{{
            "id": "{id}",
            "topic": "http.call",
            "instance_id": "{iid}",
            "execution_id": "{eid}",
            "locked_until": null,
            "retries": 3,
            "retry_count": 0,
            "variables": {vars}
        }}"#,
        id = Uuid::new_v4(),
        iid = Uuid::new_v4(),
        eid = Uuid::new_v4(),
        vars = serde_json::to_string(&vars).unwrap()
    );
    serde_json::from_str(&json_str).unwrap()
}

fn handler_config(url_template: &str) -> config::HandlerConfig {
    config::HandlerConfig {
        url_template: url_template.into(),
        method: "POST".into(),
        headers: BTreeMap::new(),
        request_template: Some(serde_json::json!({"hello": "{{var:name}}"})),
        response_mapping: {
            let mut m = BTreeMap::new();
            m.insert("order_id".into(), "$.id".into());
            m
        },
        auth: None,
        idempotency: config::IdempotencyConfig::default(),
        timeout_secs: 5,
        bpmn_error_on_4xx: None,
    }
}

#[tokio::test]
async fn happy_path_completes_with_mapped_variables() {
    let mut server = mockito::Server::new_async().await;
    let m = server
        .mock("POST", "/orders")
        .match_header("idempotency-key", mockito::Matcher::Regex("^task-".into()))
        .match_body(mockito::Matcher::JsonString(r#"{"hello":"alice"}"#.into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"id":"ord-99","total":42}"#)
        .create_async()
        .await;

    let url = format!("{}/orders", server.url());
    let mut cfg = handler_config(&url);
    cfg.url_template = url.clone();

    let h = HttpHandler::new("http.call".into(), cfg).unwrap();
    let task = task_with_vars(vec![Variable::string("name", "alice")]);
    let result = h.handle(&task).await.unwrap();

    let HandlerResult::Complete { variables } = result else {
        panic!("expected Complete, got {result:?}");
    };
    let order = variables.iter().find(|v| v.name == "order_id").unwrap();
    assert_eq!(order.value, serde_json::json!("ord-99"));

    m.assert_async().await;
}

#[tokio::test]
async fn four_oh_four_with_bpmn_error_code_returns_bpmn_error() {
    let mut server = mockito::Server::new_async().await;
    let _m = server
        .mock("POST", "/orders")
        .with_status(404)
        .with_body("not found")
        .create_async()
        .await;

    let url = format!("{}/orders", server.url());
    let mut cfg = handler_config(&url);
    cfg.bpmn_error_on_4xx = Some("ORDER_REJECTED".into());

    let h = HttpHandler::new("http.call".into(), cfg).unwrap();
    let task = task_with_vars(vec![]);
    let result = h.handle(&task).await.unwrap();

    let HandlerResult::BpmnError { code, .. } = result else {
        panic!("expected BpmnError, got {result:?}");
    };
    assert_eq!(code, "ORDER_REJECTED");
}

#[tokio::test]
async fn five_hundred_returns_handler_error_for_retry() {
    let mut server = mockito::Server::new_async().await;
    let _m = server
        .mock("POST", "/orders")
        .with_status(500)
        .with_body("boom")
        .create_async()
        .await;

    let url = format!("{}/orders", server.url());
    let cfg = handler_config(&url);

    let h = HttpHandler::new("http.call".into(), cfg).unwrap();
    let task = task_with_vars(vec![]);
    let err = h.handle(&task).await.unwrap_err();
    assert!(
        err.message.contains("500"),
        "expected 500 in error: {}",
        err.message
    );
}

#[tokio::test]
async fn idempotency_key_uses_task_id() {
    let mut server = mockito::Server::new_async().await;
    let task_id = Uuid::new_v4();
    let m = server
        .mock("POST", "/orders")
        .match_header("idempotency-key", format!("task-{task_id}").as_str())
        .with_status(200)
        .with_body("{}")
        .create_async()
        .await;

    let url = format!("{}/orders", server.url());
    let cfg = handler_config(&url);
    let h = HttpHandler::new("http.call".into(), cfg).unwrap();

    let task: ExternalTask = serde_json::from_str(&format!(
        r#"{{
            "id": "{id}",
            "topic": "http.call",
            "instance_id": "{iid}",
            "execution_id": "{eid}",
            "locked_until": null,
            "retries": 3,
            "retry_count": 0,
            "variables": [{{"name": "name", "value_type": "String", "value": "alice"}}]
        }}"#,
        id = task_id,
        iid = Uuid::new_v4(),
        eid = Uuid::new_v4()
    ))
    .unwrap();

    h.handle(&task).await.unwrap();
    m.assert_async().await;
}
