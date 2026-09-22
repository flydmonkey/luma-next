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
async fn root_serves_the_browser_webui() {
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
    assert!(String::from_utf8_lossy(&body).contains("SCREEN RECORDER"));
}

#[tokio::test]
async fn settings_round_trip_and_reject_unsupported_values() {
    let temp = tempfile::tempdir().expect("temp directory");
    let output = temp.path().join("recordings");
    let app = luma_engine::app_with_paths(temp.path().join("settings.json"), output.clone());
    let valid = json!({
        "output_directory": output,
        "record_system_audio": true,
        "record_microphone": false,
        "quality": "1080p30"
    });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/settings")
                .header("content-type", "application/json")
                .body(Body::from(valid.to_string()))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/settings")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    let body = to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("body");
    let body: Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(body["data"]["quality"], "1080p30");

    let invalid = json!({"output_directory": output, "record_system_audio": true, "record_microphone": true, "quality": "1080p30"});
    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/settings")
                .header("content-type", "application/json")
                .body(Body::from(invalid.to_string()))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn library_lists_recordings_and_delete_cannot_traverse() {
    let temp = tempfile::tempdir().expect("temp directory");
    let output = temp.path().join("recordings");
    std::fs::create_dir_all(&output).expect("recording directory");
    std::fs::write(output.join("sample.mkv"), b"not-real-media").expect("recording fixture");
    std::fs::write(temp.path().join("outside.mkv"), b"keep").expect("outside fixture");
    let app = luma_engine::app_with_paths(temp.path().join("settings.json"), output);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/library")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    let body = to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("body");
    let body: Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(body["data"][0]["name"], "sample.mkv");

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/library/%2E%2E%5Coutside.mkv")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_ne!(response.status(), StatusCode::OK);
    assert!(temp.path().join("outside.mkv").exists());

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/library/sample.mkv")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn unsupported_start_mode_is_rejected_before_backend_access() {
    let response = luma_engine::app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/session/start")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"mode":"camera"}"#))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
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
