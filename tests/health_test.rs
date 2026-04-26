mod common;

#[tokio::test]
async fn health_check_returns_ok_when_db_connected() {
    let app = common::spawn_test_app().await;

    let response = reqwest::get(&format!("{}/health", app.address))
        .await
        .expect("Failed to call health endpoint");

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response
        .json()
        .await
        .expect("Failed to parse health response");

    assert_eq!(body["status"], "ok");
    assert_eq!(body["database"], "connected");
    assert!(body["version"].is_string());
}

#[tokio::test]
async fn health_check_includes_version() {
    let app = common::spawn_test_app().await;

    let response = reqwest::get(&format!("{}/health", app.address))
        .await
        .unwrap();

    let body: serde_json::Value = response.json().await.unwrap();

    // Version should match Cargo.toml version
    assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));
}
