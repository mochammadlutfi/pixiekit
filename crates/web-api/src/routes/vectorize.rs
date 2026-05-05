//! POST /api/vectorize — STUB (Phase 3 not yet merged).
//!
//! Returns 501 Not Implemented unconditionally. After Phase 3 merges
//! `core::vectorize`, replace the body with JSON + multipart handlers
//! mirroring `bg_remove.rs`.

// TODO Phase 3 wiring: replace this stub with calls to `pixiekit_core::vectorize`.

use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{routing::post, Json, Router};
use serde_json::json;

pub fn router() -> Router {
    Router::new().route("/api/vectorize", post(vectorize_stub))
}

async fn vectorize_stub() -> impl IntoResponse {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({
            "error": "vectorize endpoint will be available after Phase 3 merge"
        })),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    #[tokio::test]
    async fn vectorize_returns_501() {
        let app = router();
        let response = app
            .oneshot(
                Request::post("/api/vectorize")
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
    }
}
