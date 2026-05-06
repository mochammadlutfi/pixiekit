//! POST /api/scale — multi-density resampling.
//!
//! Two modes (dispatched by `Content-Type`):
//! - `application/json`  → batch a server-side path, returns counts + per-file
//!   variant lists.
//! - `multipart/form-data` → single-file upload, returns the *first* requested
//!   variant as raw image bytes (a tar/zip of variants is out of scope; the
//!   caller can request a single density to get a one-shot resample).

use std::path::{Path, PathBuf};
use std::time::Instant;

use axum::body::Body;
use axum::extract::{FromRequest, Multipart, Request};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use pixiekit_core::{batch, scale};

use crate::error::{AppError, AppResult};
use crate::upload::collect_multipart;

const ALLOWED_EXTS: &[&str] = &["png", "jpg", "jpeg", "webp"];
const DEFAULT_UPLOAD_NAME: &str = "upload.png";

pub fn router() -> Router {
    Router::new().route("/api/scale", post(handle))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApiNamingMode {
    #[default]
    Flutter,
    Suffix,
    Nested,
}

impl ApiNamingMode {
    fn core(self) -> scale::NamingMode {
        match self {
            ApiNamingMode::Flutter => scale::NamingMode::Flutter,
            ApiNamingMode::Suffix => scale::NamingMode::Suffix,
            ApiNamingMode::Nested => scale::NamingMode::Nested,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApiFilter {
    #[default]
    Lanczos,
    Bilinear,
    Nearest,
}

impl ApiFilter {
    fn core(self) -> scale::Filter {
        match self {
            ApiFilter::Lanczos => scale::Filter::Lanczos,
            ApiFilter::Bilinear => scale::Filter::Bilinear,
            ApiFilter::Nearest => scale::Filter::Nearest,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ApiOptions {
    pub base_scale: Option<f32>,
    pub target_scales: Option<Vec<f32>>,
    #[serde(default)]
    pub naming: Option<ApiNamingMode>,
    #[serde(default)]
    pub filter: Option<ApiFilter>,
}

impl ApiOptions {
    fn core(&self) -> scale::Options {
        let defaults = scale::Options::default();
        scale::Options {
            base_scale: self.base_scale.unwrap_or(defaults.base_scale),
            target_scales: self
                .target_scales
                .clone()
                .filter(|v| !v.is_empty())
                .unwrap_or(defaults.target_scales),
            naming: self.naming.map(|n| n.core()).unwrap_or(defaults.naming),
            filter: self.filter.map(|f| f.core()).unwrap_or(defaults.filter),
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
    pub variants: Vec<PathBuf>,
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

fn process_one(input_path: &Path, output_dir: &Path, opts: &scale::Options) -> FileResult {
    match scale::process(input_path, output_dir, opts) {
        Ok(report) => FileResult {
            input: input_path.to_path_buf(),
            variants: report.variants,
            status: "ok",
            error: None,
        },
        Err(e) => FileResult {
            input: input_path.to_path_buf(),
            variants: Vec::new(),
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
    let outdir = temp_dir.path().to_path_buf();

    let report =
        tokio::task::spawn_blocking(move || scale::process(&input_path, &outdir, &core_opts))
            .await
            .map_err(|e| AppError::Internal(format!("Join error: {e}")))?
            .map_err(AppError::Core)?;

    let first = report
        .variants
        .first()
        .cloned()
        .ok_or_else(|| AppError::Internal("scale returned no variants".into()))?;
    let bytes = tokio::fs::read(&first).await?;
    let resolved_ext = first
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
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
    async fn scale_empty_dir_returns_zero() {
        let dir =
            std::env::temp_dir().join(format!("pixiekit-web-scale-empty-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let app = router();
        let body = serde_json::json!({
            "input": dir,
            "output": dir,
        });
        let response = app
            .oneshot(
                Request::post("/api/scale")
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
    async fn scale_missing_input_returns_404() {
        let app = router();
        let body = serde_json::json!({
            "input": "/nope/does/not/exist",
            "output": "/tmp"
        });
        let response = app
            .oneshot(
                Request::post("/api/scale")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
