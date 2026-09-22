use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use tower::ServiceExt;

async fn get_json(path: &str) -> (StatusCode, Value) {
    let response = luma_engine::app()
        .oneshot(
            Request::builder()
                .uri(path)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("router response");
    let status = response.status();
    let body = to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("response body");
    (
        status,
        serde_json::from_slice(&body).expect("json response"),
    )
}

#[tokio::test]
async fn probe_uses_the_legacy_envelope() {
    let (status, body) = get_json("/api/v1").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({
            "ok": true,
            "data": { "name": "luma-next", "control": true },
            "error": null
        })
    );
}

#[tokio::test]
async fn session_starts_idle() {
    let (status, body) = get_json("/api/v1/session").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["ok"], true);
    assert_eq!(body["data"]["state"], "idle");
    assert!(body["data"]["output_path"].is_null());
    assert!(body["error"].is_null());
}
