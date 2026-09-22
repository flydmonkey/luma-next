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

#[tokio::test]
async fn root_serves_a_shell_safe_webui() {
    let response = luma_engine::app()
        .oneshot(
            Request::builder()
                .uri("/")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("router response");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()["content-type"],
        "text/html; charset=utf-8"
    );
    let body = to_bytes(response.into_body(), 128 * 1024)
        .await
        .expect("response body");
    assert!(String::from_utf8_lossy(&body).contains("本机引擎已连接"));
}

#[tokio::test]
async fn start_fails_honestly_without_a_recorder_backend() {
    let response = luma_engine::app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/session/start")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("router response");
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("response body");
    let body: Value = serde_json::from_slice(&body).expect("json response");
    assert_eq!(body["ok"], false);
    assert!(
        body["error"]
            .as_str()
            .is_some_and(|value| value.contains("libobs"))
    );
}
