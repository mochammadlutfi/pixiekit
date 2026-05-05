//! End-to-end router tests using `tower::ServiceExt::oneshot` (no port bind).

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

use pixiekit_web_api::build_router;

fn app() -> axum::Router {
    build_router(Some(Vec::new()))
}

#[tokio::test]
async fn health_returns_ok_with_version() {
    let response = app()
        .oneshot(Request::get("/api/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["status"], "ok");
    assert!(body["version"].is_string());
}

#[tokio::test]
async fn vectorize_rejects_empty_body() {
    // Phase 3 wired: empty JSON body lacks required `input`/`output` →
    // axum's Json extractor returns 400.
    let response = app()
        .oneshot(
            Request::post("/api/vectorize")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn bg_remove_rejects_missing_content_type() {
    let response = app()
        .oneshot(
            Request::post("/api/bg-remove")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn bg_remove_json_returns_404_for_missing_input() {
    let body = serde_json::json!({
        "input": "/abs/path/to/nowhere/xyz-999",
        "output": "/tmp/pixiekit-out-test-2"
    });
    let response = app()
        .oneshot(
            Request::post("/api/bg-remove")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn bg_remove_json_processes_empty_directory() {
    let dir = tempfile::tempdir().unwrap();
    let out = tempfile::tempdir().unwrap();
    let body = serde_json::json!({
        "input": dir.path(),
        "output": out.path(),
        "options": {"fuzz": 0.35}
    });
    let response = app()
        .oneshot(
            Request::post("/api/bg-remove")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let json: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["processed"], 0);
    assert_eq!(json["failed"], 0);
}

#[tokio::test]
async fn video_to_sprite_rejects_unknown_content_type() {
    let response = app()
        .oneshot(
            Request::post("/api/video-to-sprite")
                .header("content-type", "text/yaml")
                .body(Body::from(""))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn unknown_route_returns_404() {
    let response = app()
        .oneshot(
            Request::get("/api/does-not-exist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
