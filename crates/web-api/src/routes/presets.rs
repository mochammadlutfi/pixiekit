//! `/api/presets` — CRUD for saved tool configurations.
//!
//! Backed by `pixiekit_core::preset`, which persists each preset as JSON under
//! `~/.config/pixiekit/presets/<name>.json` (or `$PIXIEKIT_CONFIG_DIR`).
//!
//! Endpoints:
//! - `GET    /api/presets`             → list all presets (full bodies)
//! - `GET    /api/presets/:name`       → fetch one preset by name
//! - `PUT    /api/presets/:name`       → create or overwrite (body: `{tool, options}`)
//! - `DELETE /api/presets/:name`       → remove
//!
//! `tool` must be one of `bg-remove`, `vectorize`, `video-to-sprite` — the
//! kebab-case strings that match `pixiekit_core::preset::TOOL_*`.

use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use pixiekit_core::preset;

use crate::error::{AppError, AppResult};

pub fn router() -> Router {
    Router::new()
        .route("/api/presets", get(list_presets))
        .route(
            "/api/presets/{name}",
            get(get_preset).put(put_preset).delete(delete_preset),
        )
}

#[derive(Debug, Serialize)]
pub struct PresetView {
    pub name: String,
    pub tool: String,
    pub version: u32,
    pub options: serde_json::Value,
}

impl From<preset::Preset> for PresetView {
    fn from(p: preset::Preset) -> Self {
        Self {
            name: p.name,
            tool: p.tool,
            version: p.version,
            options: p.options,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ListResponse {
    pub presets: Vec<PresetView>,
    pub presets_dir: String,
}

#[derive(Debug, Deserialize)]
pub struct PutBody {
    pub tool: String,
    pub options: serde_json::Value,
}

async fn list_presets() -> AppResult<Json<ListResponse>> {
    let names = preset::list()?;
    let presets_dir = preset::presets_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    // Hydrate full bodies so the frontend doesn't need N follow-up GETs.
    let presets = names
        .into_iter()
        .filter_map(|name| preset::load(&name).ok().map(PresetView::from))
        .collect();
    Ok(Json(ListResponse {
        presets,
        presets_dir,
    }))
}

async fn get_preset(Path(name): Path<String>) -> AppResult<Json<PresetView>> {
    let p = preset::load(&name)?;
    Ok(Json(PresetView::from(p)))
}

async fn put_preset(
    Path(name): Path<String>,
    Json(body): Json<PutBody>,
) -> AppResult<(StatusCode, Json<PresetView>)> {
    if !is_known_tool(&body.tool) {
        return Err(AppError::BadRequest(format!(
            "unknown tool '{}': expected bg-remove | vectorize | video-to-sprite",
            body.tool
        )));
    }
    preset::save(&name, &body.tool, body.options)?;
    let saved = preset::load(&name)?;
    Ok((StatusCode::OK, Json(PresetView::from(saved))))
}

async fn delete_preset(Path(name): Path<String>) -> AppResult<impl IntoResponse> {
    preset::delete(&name)?;
    Ok(StatusCode::NO_CONTENT)
}

fn is_known_tool(tool: &str) -> bool {
    matches!(
        tool,
        preset::TOOL_BG_REMOVE | preset::TOOL_VECTORIZE | preset::TOOL_VIDEO_TO_SPRITE
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use std::sync::{Mutex, MutexGuard, OnceLock};
    use tower::ServiceExt;

    fn env_lock() -> MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|p| p.into_inner())
    }

    struct ScopedConfigDir {
        _tmp: tempfile::TempDir,
        _guard: MutexGuard<'static, ()>,
    }

    impl ScopedConfigDir {
        fn new() -> Self {
            let guard = env_lock();
            let tmp = tempfile::Builder::new()
                .prefix("pixiekit-web-preset-test-")
                .tempdir()
                .unwrap();
            std::env::set_var("PIXIEKIT_CONFIG_DIR", tmp.path());
            Self {
                _tmp: tmp,
                _guard: guard,
            }
        }
    }

    impl Drop for ScopedConfigDir {
        fn drop(&mut self) {
            std::env::remove_var("PIXIEKIT_CONFIG_DIR");
        }
    }

    async fn body_to_json(body: Body) -> serde_json::Value {
        let bytes = body.collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn list_returns_empty_for_fresh_dir() {
        let _scope = ScopedConfigDir::new();
        let app = router();
        let res = app
            .oneshot(Request::get("/api/presets").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = body_to_json(res.into_body()).await;
        assert!(body["presets"].is_array());
        assert_eq!(body["presets"].as_array().unwrap().len(), 0);
        assert!(body["presets_dir"].is_string());
    }

    #[tokio::test]
    async fn put_then_get_roundtrip() {
        let _scope = ScopedConfigDir::new();
        let app = router();
        let put_body = serde_json::json!({
            "tool": "bg-remove",
            "options": { "fuzz": 0.5, "erode": 2 }
        });
        let res = app
            .clone()
            .oneshot(
                Request::put("/api/presets/clean")
                    .header("content-type", "application/json")
                    .body(Body::from(put_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let res = app
            .oneshot(
                Request::get("/api/presets/clean")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = body_to_json(res.into_body()).await;
        assert_eq!(body["name"], "clean");
        assert_eq!(body["tool"], "bg-remove");
        assert_eq!(body["options"]["fuzz"], 0.5);
        assert_eq!(body["options"]["erode"], 2);
    }

    #[tokio::test]
    async fn put_rejects_unknown_tool() {
        let _scope = ScopedConfigDir::new();
        let app = router();
        let body = serde_json::json!({ "tool": "rainbow", "options": {} });
        let res = app
            .oneshot(
                Request::put("/api/presets/foo")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn put_rejects_invalid_name() {
        let _scope = ScopedConfigDir::new();
        let app = router();
        let body = serde_json::json!({ "tool": "bg-remove", "options": {} });
        let res = app
            .oneshot(
                Request::put("/api/presets/has.dot")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn get_missing_returns_404() {
        let _scope = ScopedConfigDir::new();
        let app = router();
        let res = app
            .oneshot(
                Request::get("/api/presets/ghost")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn delete_then_404_on_get() {
        let _scope = ScopedConfigDir::new();
        let app = router();
        // Create
        let body = serde_json::json!({ "tool": "vectorize", "options": {"smooth": 4} });
        app.clone()
            .oneshot(
                Request::put("/api/presets/temp")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        // Delete
        let res = app
            .clone()
            .oneshot(
                Request::delete("/api/presets/temp")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NO_CONTENT);
        // GET → 404
        let res = app
            .oneshot(
                Request::get("/api/presets/temp")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn list_returns_full_bodies_after_save() {
        let _scope = ScopedConfigDir::new();
        let app = router();
        let body = serde_json::json!({ "tool": "bg-remove", "options": {"fuzz": 0.42} });
        app.clone()
            .oneshot(
                Request::put("/api/presets/alpha")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let res = app
            .oneshot(Request::get("/api/presets").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let body = body_to_json(res.into_body()).await;
        let presets = body["presets"].as_array().unwrap();
        assert_eq!(presets.len(), 1);
        assert_eq!(presets[0]["name"], "alpha");
        assert_eq!(presets[0]["options"]["fuzz"], 0.42);
    }
}
