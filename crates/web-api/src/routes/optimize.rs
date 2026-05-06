//! POST /api/optimize — PNG/JPG/WebP byte-size optimization.
//!
//! Two modes (dispatched by `Content-Type`):
//! - `application/json`  → batch a server-side path, returns counts + file list
//! - `multipart/form-data` → single-file upload, returns optimized bytes

use std::path::{Path, PathBuf};
use std::time::Instant;

use axum::body::Body;
use axum::extract::{FromRequest, Multipart, Request};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use pixiekit_core::{batch, optimize};

use crate::error::{AppError, AppResult};
use crate::upload::collect_multipart;

const ALLOWED_EXTS: &[&str] = &["png", "jpg", "jpeg", "webp"];
const DEFAULT_UPLOAD_NAME: &str = "upload.png";

pub fn router() -> Router {
    Router::new().route("/api/optimize", post(handle))
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ApiOptions {
    #[serde(default)]
    pub target_format: Option<TargetFormat>,
    pub quality: Option<u8>,
    pub lossless: Option<bool>,
    pub strip_metadata: Option<bool>,
    pub optimization_level: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TargetFormat {
    Png,
    Webp,
    Keep,
}

impl ApiOptions {
    fn core(&self) -> optimize::Options {
        let defaults = optimize::Options::default();
        let target_format = match self.target_format {
            Some(TargetFormat::Png) => optimize::TargetFormat::Png,
            Some(TargetFormat::Webp) => optimize::TargetFormat::Webp,
            Some(TargetFormat::Keep) => optimize::TargetFormat::Keep,
            None => defaults.target_format,
        };
        optimize::Options {
            target_format,
            quality: self.quality.unwrap_or(defaults.quality),
            lossless: self.lossless.unwrap_or(defaults.lossless),
            strip_metadata: self.strip_metadata.unwrap_or(defaults.strip_metadata),
            optimization_level: self
                .optimization_level
                .unwrap_or(defaults.optimization_level),
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

fn process_one(input_path: &Path, output_dir: &Path, opts: &optimize::Options) -> FileResult {
    let stem = input_path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "out".into());
    let target_stub = output_dir.join(stem);

    match optimize::process(input_path, &target_stub, opts) {
        Ok(report) => FileResult {
            input: input_path.to_path_buf(),
            output: Some(report.output_path),
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
        Some(s) if !s.trim().is_empty() => serde_json::from_str(s)
            .map_err(|e| AppError::BadRequest(format!("Invalid options JSON: {e}")))?,
        _ => ApiOptions::default(),
    };
    let core_opts = opts.core();

    let original_name = upload
        .original_filename
        .clone()
        .unwrap_or_else(|| DEFAULT_UPLOAD_NAME.to_string());
    let input_path = upload.path.clone();
    let temp_dir = upload.temp_dir;
    let stub = temp_dir.path().join("optimized");

    let report =
        tokio::task::spawn_blocking(move || optimize::process(&input_path, &stub, &core_opts))
            .await
            .map_err(|e| AppError::Internal(format!("Join error: {e}")))?
            .map_err(AppError::Core)?;

    let bytes = tokio::fs::read(&report.output_path).await?;
    let resolved_ext = report
        .output_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin")
        .to_string();
    drop(temp_dir);

    let stem = Path::new(&original_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let disposition = format!("inline; filename=\"{stem}.{resolved_ext}\"");

    let content_type = match resolved_ext.as_str() {
        "png" => "image/png",
        "webp" => "image/webp",
        "jpg" | "jpeg" => "image/jpeg",
        _ => "application/octet-stream",
    };

    let mut response = Response::new(Body::from(bytes));
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
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
    async fn optimize_empty_dir_returns_zero() {
        let dir = std::env::temp_dir().join(format!(
            "pixiekit-web-optimize-empty-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let app = router();
        let body = serde_json::json!({
            "input": dir,
            "output": dir,
        });
        let response = app
            .oneshot(
                Request::post("/api/optimize")
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
    async fn optimize_missing_input_returns_404() {
        let app = router();
        let body = serde_json::json!({
            "input": "/nope/does/not/exist",
            "output": "/tmp"
        });
        let response = app
            .oneshot(
                Request::post("/api/optimize")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
