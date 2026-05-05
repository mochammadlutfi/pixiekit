//! POST /api/vectorize — raster (PNG/JPG/WebP) → SVG vector trace.
//!
//! Two modes (dispatched by `Content-Type`):
//! - `application/json`  → batch a server-side path, returns counts + file list
//! - `multipart/form-data` → single-file upload, returns SVG body

use std::path::{Path, PathBuf};
use std::time::Instant;

use axum::body::Body;
use axum::extract::{FromRequest, Multipart, Request};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use pixiekit_core::{batch, vectorize};

use crate::error::{AppError, AppResult};
use crate::upload::collect_multipart;

const ALLOWED_EXTS: &[&str] = &["png", "jpg", "jpeg", "webp"];
const DEFAULT_UPLOAD_NAME: &str = "upload.png";

pub fn router() -> Router {
    Router::new().route("/api/vectorize", post(handle))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApiMode {
    #[default]
    Color,
    Binary,
}

impl ApiMode {
    fn core(self) -> vectorize::Mode {
        match self {
            ApiMode::Color => vectorize::Mode::Color,
            ApiMode::Binary => vectorize::Mode::Binary,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiOptions {
    #[serde(default)]
    pub mode: ApiMode,
    /// 0-10 simple "smoothness" slider. When provided, overrides
    /// corner/length/splice thresholds via [`vectorize::smooth_to_params`].
    pub smooth: Option<u8>,
    pub filter_speckle: Option<u32>,
    pub color_precision: Option<u8>,
    pub layer_difference: Option<u8>,
    pub corner_threshold: Option<u8>,
    pub length_threshold: Option<f64>,
    pub splice_threshold: Option<u8>,
    pub path_precision: Option<u8>,
}

impl Default for ApiOptions {
    fn default() -> Self {
        Self {
            mode: ApiMode::default(),
            smooth: None,
            filter_speckle: None,
            color_precision: None,
            layer_difference: None,
            corner_threshold: None,
            length_threshold: None,
            splice_threshold: None,
            path_precision: None,
        }
    }
}

impl ApiOptions {
    fn core(&self) -> vectorize::Options {
        let defaults = vectorize::Options::default();
        let (corner, length, splice) = match self.smooth {
            Some(s) => vectorize::smooth_to_params(s),
            None => (
                defaults.corner_threshold,
                defaults.length_threshold,
                defaults.splice_threshold,
            ),
        };
        vectorize::Options {
            mode: self.mode.core(),
            filter_speckle: self.filter_speckle.unwrap_or(defaults.filter_speckle),
            color_precision: self.color_precision.unwrap_or(defaults.color_precision),
            layer_difference: self.layer_difference.unwrap_or(defaults.layer_difference),
            corner_threshold: self.corner_threshold.unwrap_or(corner),
            length_threshold: self.length_threshold.unwrap_or(length),
            splice_threshold: self.splice_threshold.unwrap_or(splice),
            path_precision: self.path_precision.unwrap_or(defaults.path_precision),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct PathRequest {
    pub input: PathBuf,
    pub output: PathBuf,
    #[serde(default)]
    pub options: ApiOptions,
}

#[derive(Debug, Serialize)]
pub struct FileResult {
    pub input: PathBuf,
    pub output: Option<PathBuf>,
    pub status: &'static str,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PathResponse {
    pub processed: usize,
    pub failed: usize,
    pub duration_ms: u128,
    pub files: Vec<FileResult>,
}

async fn handle(headers: HeaderMap, request: Request) -> AppResult<Response> {
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if content_type.starts_with("multipart/") {
        let multipart = Multipart::from_request(request, &())
            .await
            .map_err(|e| AppError::Multipart(e.to_string()))?;
        handle_multipart(multipart).await
    } else {
        let Json(req): Json<PathRequest> = Json::from_request(request, &())
            .await
            .map_err(|e| AppError::BadRequest(format!("Invalid JSON: {e}")))?;
        handle_path_batch(req).await
    }
}

async fn handle_path_batch(req: PathRequest) -> AppResult<Response> {
    let opts = req.options.core();

    let files = tokio::task::spawn_blocking({
        let input = req.input.clone();
        move || batch::list_images(&input, false, ALLOWED_EXTS)
    })
    .await
    .map_err(|e| AppError::Internal(format!("Join error: {e}")))?
    .map_err(AppError::Core)?;

    if files.is_empty() {
        return Ok(Json(PathResponse {
            processed: 0,
            failed: 0,
            duration_ms: 0,
            files: vec![],
        })
        .into_response());
    }

    let output_dir = req.output.clone();
    tokio::fs::create_dir_all(&output_dir).await?;

    let start = Instant::now();
    let opts_for_task = opts.clone();
    let results: Vec<FileResult> = tokio::task::spawn_blocking(move || {
        use rayon::prelude::*;
        files
            .par_iter()
            .map(|input_path| process_one(input_path, &output_dir, &opts_for_task))
            .collect()
    })
    .await
    .map_err(|e| AppError::Internal(format!("Join error: {e}")))?;

    let processed = results.iter().filter(|r| r.error.is_none()).count();
    let failed = results.len() - processed;

    Ok(Json(PathResponse {
        processed,
        failed,
        duration_ms: start.elapsed().as_millis(),
        files: results,
    })
    .into_response())
}

fn process_one(input_path: &Path, output_dir: &Path, opts: &vectorize::Options) -> FileResult {
    let stem = input_path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "out".into());
    let svg_path = output_dir.join(format!("{stem}.svg"));

    match vectorize::process(input_path, &svg_path, opts) {
        Ok(()) => FileResult {
            input: input_path.to_path_buf(),
            output: Some(svg_path),
            status: "ok",
            error: None,
        },
        Err(e) => FileResult {
            input: input_path.to_path_buf(),
            output: None,
            status: "failed",
            error: Some(e.to_string()),
        },
    }
}

async fn handle_multipart(multipart: Multipart) -> AppResult<Response> {
    let upload = collect_multipart(multipart, DEFAULT_UPLOAD_NAME).await?;
    let opts: ApiOptions = match &upload.options_json {
        Some(s) => serde_json::from_str(s)
            .map_err(|e| AppError::BadRequest(format!("Invalid options JSON: {e}")))?,
        None => ApiOptions::default(),
    };
    let core_opts = opts.core();

    let original_name = upload
        .original_filename
        .clone()
        .unwrap_or_else(|| DEFAULT_UPLOAD_NAME.to_string());
    let input_path = upload.path.clone();

    let svg_tmp = tempfile::Builder::new()
        .prefix("pixiekit-vec-out-")
        .suffix(".svg")
        .tempfile()
        .map_err(AppError::Io)?;
    let svg_path = svg_tmp.path().to_path_buf();

    tokio::task::spawn_blocking(move || vectorize::process(&input_path, &svg_path, &core_opts))
        .await
        .map_err(|e| AppError::Internal(format!("Join error: {e}")))?
        .map_err(AppError::Core)?;

    let svg_bytes = tokio::fs::read(svg_tmp.path()).await?;
    drop(upload); // upload temp_dir lives until here

    let stem = Path::new(&original_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let disposition = format!("inline; filename=\"{stem}.svg\"");

    let mut response = Response::new(Body::from(svg_bytes));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("image/svg+xml"),
    );
    if let Ok(value) = HeaderValue::from_str(&disposition) {
        response
            .headers_mut()
            .insert(header::CONTENT_DISPOSITION, value);
    }
    *response.status_mut() = StatusCode::OK;
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    #[tokio::test]
    async fn vectorize_empty_dir_returns_zero() {
        let dir =
            std::env::temp_dir().join(format!("pixiekit-web-vec-empty-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let app = router();
        let body = serde_json::json!({
            "input": dir,
            "output": dir,
        });
        let response = app
            .oneshot(
                Request::post("/api/vectorize")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["processed"], 0);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn vectorize_missing_input_returns_404() {
        let app = router();
        let body = serde_json::json!({
            "input": "/nope/does/not/exist",
            "output": "/tmp"
        });
        let response = app
            .oneshot(
                Request::post("/api/vectorize")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
