mod common;

use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn create_secret_returns_201_without_value() {
    let app = common::spawn_test_app().await;
    let org_id = common::create_test_org(&app).await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{}/api/v1/orgs/{}/secrets", app.address, org_id))
        .json(&json!({ "name": "stripe_key", "value": "sk_test_super_secret" }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["name"], "stripe_key");
    assert_eq!(body["org_id"], org_id.to_string());
    assert!(body.get("value").is_none(), "value must never be returned");
    assert!(body.get("value_encrypted").is_none());
    assert!(body.get("nonce").is_none());
}

#[tokio::test]
async fn list_secrets_omits_values() {
    let app = common::spawn_test_app().await;
    let org_id = common::create_test_org(&app).await;
    let client = reqwest::Client::new();

    for n in &["k1", "k2", "k3"] {
        client
            .post(format!("{}/api/v1/orgs/{}/secrets", app.address, org_id))
            .json(&json!({ "name": n, "value": "the-value" }))
            .send()
            .await
            .unwrap();
    }

    let resp = client
        .get(format!("{}/api/v1/orgs/{}/secrets", app.address, org_id))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 3);
    for entry in arr {
        assert!(entry.get("value").is_none());
        assert!(entry.get("value_encrypted").is_none());
    }
}

#[tokio::test]
async fn get_secret_metadata_omits_value() {
    let app = common::spawn_test_app().await;
    let org_id = common::create_test_org(&app).await;
    let client = reqwest::Client::new();

    client
        .post(format!("{}/api/v1/orgs/{}/secrets", app.address, org_id))
        .json(&json!({ "name": "api_token", "value": "live_xyz" }))
        .send()
        .await
        .unwrap();

    let resp = client
        .get(format!(
            "{}/api/v1/orgs/{}/secrets/api_token",
            app.address, org_id
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["name"], "api_token");
    assert!(body.get("value").is_none());
}

#[tokio::test]
async fn duplicate_name_in_same_org_returns_409() {
    let app = common::spawn_test_app().await;
    let org_id = common::create_test_org(&app).await;
    let client = reqwest::Client::new();

    let body = json!({ "name": "dup", "value": "v1" });
    let r1 = client
        .post(format!("{}/api/v1/orgs/{}/secrets", app.address, org_id))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(r1.status(), 201);

    let r2 = client
        .post(format!("{}/api/v1/orgs/{}/secrets", app.address, org_id))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(r2.status(), 409);
}

#[tokio::test]
async fn same_name_in_different_orgs_is_allowed() {
    let app = common::spawn_test_app().await;
    let org_a = common::create_test_org(&app).await;
    let org_b = common::create_test_org(&app).await;
    let client = reqwest::Client::new();

    for org in &[org_a, org_b] {
        let resp = client
            .post(format!("{}/api/v1/orgs/{}/secrets", app.address, org))
            .json(&json!({ "name": "shared_name", "value": "any" }))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 201);
    }
}

#[tokio::test]
async fn list_is_org_isolated() {
    let app = common::spawn_test_app().await;
    let org_a = common::create_test_org(&app).await;
    let org_b = common::create_test_org(&app).await;
    let client = reqwest::Client::new();

    client
        .post(format!("{}/api/v1/orgs/{}/secrets", app.address, org_a))
        .json(&json!({ "name": "only_in_a", "value": "x" }))
        .send()
        .await
        .unwrap();

    let resp = client
        .get(format!("{}/api/v1/orgs/{}/secrets", app.address, org_b))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body.as_array().unwrap().len(), 0);

    let resp = client
        .get(format!(
            "{}/api/v1/orgs/{}/secrets/only_in_a",
            app.address, org_b
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn delete_secret_removes_it() {
    let app = common::spawn_test_app().await;
    let org_id = common::create_test_org(&app).await;
    let client = reqwest::Client::new();

    client
        .post(format!("{}/api/v1/orgs/{}/secrets", app.address, org_id))
        .json(&json!({ "name": "temp", "value": "v" }))
        .send()
        .await
        .unwrap();

    let del = client
        .delete(format!(
            "{}/api/v1/orgs/{}/secrets/temp",
            app.address, org_id
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(del.status(), 204);

    let get = client
        .get(format!(
            "{}/api/v1/orgs/{}/secrets/temp",
            app.address, org_id
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(get.status(), 404);
}

#[tokio::test]
async fn delete_unknown_returns_404() {
    let app = common::spawn_test_app().await;
    let org_id = common::create_test_org(&app).await;
    let client = reqwest::Client::new();

    let resp = client
        .delete(format!(
            "{}/api/v1/orgs/{}/secrets/never_existed",
            app.address, org_id
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn empty_name_or_value_rejected() {
    let app = common::spawn_test_app().await;
    let org_id = common::create_test_org(&app).await;
    let client = reqwest::Client::new();

    let r1 = client
        .post(format!("{}/api/v1/orgs/{}/secrets", app.address, org_id))
        .json(&json!({ "name": "", "value": "v" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r1.status(), 400);

    let r2 = client
        .post(format!("{}/api/v1/orgs/{}/secrets", app.address, org_id))
        .json(&json!({ "name": "ok", "value": "" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r2.status(), 400);
}

#[tokio::test]
async fn ciphertext_in_db_is_not_plaintext() {
    let app = common::spawn_test_app().await;
    let org_id = common::create_test_org(&app).await;
    let client = reqwest::Client::new();

    let plaintext = "this-is-the-secret-value-do-not-leak";
    client
        .post(format!("{}/api/v1/orgs/{}/secrets", app.address, org_id))
        .json(&json!({ "name": "leak_test", "value": plaintext }))
        .send()
        .await
        .unwrap();

    let row: (Vec<u8>, Vec<u8>) = sqlx::query_as(
        "SELECT value_encrypted, nonce FROM secrets WHERE org_id = $1 AND name = $2",
    )
    .bind(org_id)
    .bind("leak_test")
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert!(!row.0.is_empty());
    assert_eq!(row.1.len(), 12, "nonce must be 12 bytes");
    let pt_bytes = plaintext.as_bytes();
    assert!(
        row.0.windows(pt_bytes.len()).all(|w| w != pt_bytes),
        "plaintext appears verbatim in ciphertext"
    );
}

#[tokio::test]
async fn reveal_round_trips_via_db_helper() {
    // Verifies the engine-internal `reveal()` path returns the original value.
    // This is what the HTTP connector will call at fire time (C3).
    let app = common::spawn_test_app().await;
    let org_id = common::create_test_org(&app).await;
    let client = reqwest::Client::new();

    let plaintext = "bearer-token-1234567890";
    client
        .post(format!("{}/api/v1/orgs/{}/secrets", app.address, org_id))
        .json(&json!({ "name": "bearer", "value": plaintext }))
        .send()
        .await
        .unwrap();

    let test_key = [0xA5u8; 32];
    let revealed = conduit::db::secrets::reveal(&app.pool, &test_key, org_id, "bearer")
        .await
        .unwrap();
    assert_eq!(revealed, plaintext);

    let missing = conduit::db::secrets::reveal(&app.pool, &test_key, org_id, "no_such_thing").await;
    assert!(missing.is_err());
}

#[tokio::test]
async fn reveal_with_wrong_org_returns_not_found() {
    let app = common::spawn_test_app().await;
    let org_a = common::create_test_org(&app).await;
    let org_b = common::create_test_org(&app).await;
    let client = reqwest::Client::new();

    client
        .post(format!("{}/api/v1/orgs/{}/secrets", app.address, org_a))
        .json(&json!({ "name": "private_a", "value": "secret_a" }))
        .send()
        .await
        .unwrap();

    let test_key = [0xA5u8; 32];
    let result = conduit::db::secrets::reveal(&app.pool, &test_key, org_b, "private_a").await;
    assert!(result.is_err(), "org B must not see org A's secret value");
    let _unused = Uuid::nil();
}
