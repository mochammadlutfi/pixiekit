//! POST /api/svg-optimize — minify SVG via usvg parse + serialize.
//!
//! Two modes (dispatched by `Content-Type`):
//! - `application/json`     → batch a server-side path, returns counts + file list
//! - `multipart/form-data`  → single SVG upload, returns minified SVG bytes

use std::path::{Path, PathBuf};
use std::time::Instant;

use axum::body::Body;
use axum::extract::{FromRequest, Multipart, Request};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use pixiekit_core::{batch, svg_optimize};

use crate::error::{AppError, AppResult};
use crate::upload::collect_multipart;

const ALLOWED_EXTS: &[&str] = &["svg"];
const DEFAULT_UPLOAD_NAME: &str = "upload.svg";

pub fn router() -> Router {
    Router::new().route("/api/svg-optimize", post(handle))
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiOptions {
    #[serde(default = "default_precision")]
    pub precision: u8,
    #[serde(default = "default_true")]
    pub remove_metadata: bool,
    #[serde(default = "default_true")]
    pub remove_hidden: bool,
    #[serde(default = "default_true")]
    pub merge_paths: bool,
    #[serde(default)]
    pub pretty: bool,
}

fn default_precision() -> u8 {
    3
}
fn default_true() -> bool {
    true
}

impl Default for ApiOptions {
    fn default() -> Self {
        Self {
            precision: default_precision(),
            remove_metadata: true,
            remove_hidden: true,
            merge_paths: true,
            pretty: false,
        }
    }
}

impl ApiOptions {
    fn core(&self) -> svg_optimize::Options {
        svg_optimize::Options {
            precision: self.precision,
            remove_metadata: self.remove_metadata,
            remove_hidden: self.remove_hidden,
            merge_paths: self.merge_paths,
            pretty: self.pretty,
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
    pub input_size: Option<u64>,
    pub output_size: Option<u64>,
    pub ratio: Option<f32>,
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

fn process_one(input_path: &Path, output_dir: &Path, opts: &svg_optimize::Options) -> FileResult {
    let file_name = match input_path.file_name() {
        Some(n) => n,
        None => {
            return FileResult {
                input: input_path.to_path_buf(),
                output: None,
                input_size: None,
                output_size: None,
                ratio: None,
                status: "failed",
                error: Some(format!("Invalid filename: {}", input_path.display())),
            }
        }
    };
    let out_path = output_dir.join(file_name);

    match svg_optimize::process(input_path, &out_path, opts) {
        Ok(report) => FileResult {
            input: input_path.to_path_buf(),
            output: Some(out_path),
            input_size: Some(report.input_size),
            output_size: Some(report.output_size),
            ratio: Some(report.ratio),
            status: "ok",
            error: None,
        },
        Err(e) => FileResult {
            input: input_path.to_path_buf(),
            output: None,
            input_size: None,
            output_size: None,
            ratio: None,
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

    let out_tmp = tempfile::Builder::new()
        .prefix("pixiekit-svg-out-")
        .suffix(".svg")
        .tempfile()
        .map_err(AppError::Io)?;
    let out_path = out_tmp.path().to_path_buf();

    tokio::task::spawn_blocking(move || svg_optimize::process(&input_path, &out_path, &core_opts))
        .await
        .map_err(|e| AppError::Internal(format!("Join error: {e}")))?
        .map_err(AppError::Core)?;

    let bytes = tokio::fs::read(out_tmp.path()).await?;
    drop(upload);

    let stem = Path::new(&original_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let disposition = format!("inline; filename=\"{stem}.svg\"");

    let mut response = Response::new(Body::from(bytes));
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
    async fn svg_optimize_empty_dir_returns_zero() {
        let dir =
            std::env::temp_dir().join(format!("pixiekit-web-svgopt-empty-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let app = router();
        let body = serde_json::json!({
            "input": dir,
            "output": dir,
        });
        let response = app
            .oneshot(
                Request::post("/api/svg-optimize")
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
    async fn svg_optimize_missing_input_returns_404() {
        let app = router();
        let body = serde_json::json!({
            "input": "/nope/does/not/exist",
            "output": "/tmp"
        });
        let response = app
            .oneshot(
                Request::post("/api/svg-optimize")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn defaults_when_options_omitted() {
        let payload = serde_json::json!({"input": "/tmp/in", "output": "/tmp/out"});
        let req: PathRequest = serde_json::from_value(payload).unwrap();
        assert_eq!(req.options.precision, 3);
        assert!(req.options.remove_metadata);
        assert!(req.options.remove_hidden);
        assert!(req.options.merge_paths);
        assert!(!req.options.pretty);
    }
}
