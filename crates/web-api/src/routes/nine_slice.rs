//! POST /api/nine-slice — split image or generate 9-slice metadata.
//!
//! Two modes:
//! - JSON: Batch process folder/file.
//! - Multipart: Single file upload, returns either JSON metadata or first slice.

use std::path::{Path, PathBuf};
use std::time::Instant;

use axum::body::Body;
use axum::extract::{FromRequest, Multipart, Request};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use pixiekit_core::{batch, nine_slice};

use crate::error::{AppError, AppResult};
use crate::upload::collect_multipart;

const ALLOWED_EXTS: &[&str] = &["png", "jpg", "jpeg", "webp"];
const DEFAULT_UPLOAD_NAME: &str = "upload.png";

pub fn router() -> Router {
    Router::new().route("/api/nine-slice", post(handle))
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiOptions {
    #[serde(default)]
    pub top: u32,
    #[serde(default)]
    pub right: u32,
    #[serde(default)]
    pub bottom: u32,
    #[serde(default)]
    pub left: u32,
    #[serde(default)]
    pub mode: nine_slice::OutputMode,
}

impl Default for ApiOptions {
    fn default() -> Self {
        Self {
            top: 0,
            right: 0,
            bottom: 0,
            left: 0,
            mode: nine_slice::OutputMode::Metadata,
        }
    }
}

impl ApiOptions {
    fn core(&self) -> nine_slice::Options {
        nine_slice::Options {
            top: self.top,
            right: self.right,
            bottom: self.bottom,
            left: self.left,
            output_mode: self.mode,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct JsonRequest {
    pub input: PathBuf,
    pub output: PathBuf,
    #[serde(default)]
    pub options: ApiOptions,
}

#[derive(Debug, Serialize)]
pub struct FileEntry {
    pub input: PathBuf,
    pub outputs: Vec<PathBuf>,
    pub status: &'static str,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct JsonResponse {
    pub processed: usize,
    pub failed: usize,
    pub duration_ms: u128,
    pub files: Vec<FileEntry>,
}

async fn handle(req: Request) -> AppResult<Response> {
    let content_type = req
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();

    if content_type.starts_with("application/json") {
        let (_parts, body) = req.into_parts();
        let bytes = axum::body::to_bytes(body, crate::REQUEST_BODY_LIMIT)
            .await
            .map_err(|e| AppError::BadRequest(format!("read body: {e}")))?;
        let payload: JsonRequest = serde_json::from_slice(&bytes).map_err(AppError::Json)?;
        let resp = handle_json(payload).await?;
        Ok((StatusCode::OK, Json(resp)).into_response())
    } else if content_type.starts_with("multipart/form-data") {
        let multipart = Multipart::from_request(req, &())
            .await
            .map_err(|e| AppError::Multipart(e.to_string()))?;
        handle_multipart(multipart).await
    } else {
        Err(AppError::BadRequest(format!(
            "unsupported Content-Type: {:?} (expected application/json or multipart/form-data)",
            content_type
        )))
    }
}

async fn handle_json(req: JsonRequest) -> AppResult<JsonResponse> {
    tokio::task::spawn_blocking(move || run_batch(&req.input, &req.output, &req.options))
        .await
        .map_err(|e| AppError::Internal(format!("join error: {e}")))?
}

fn run_batch(input: &Path, output: &Path, opts: &ApiOptions) -> AppResult<JsonResponse> {
    let files = if input.is_file() {
        vec![input.to_path_buf()]
    } else {
        batch::list_images(input, false, ALLOWED_EXTS)?
    };

    if files.is_empty() {
        return Ok(JsonResponse {
            processed: 0,
            failed: 0,
            duration_ms: 0,
            files: Vec::new(),
        });
    }

    std::fs::create_dir_all(output)?;
    let core_opts = opts.core();
    let start = Instant::now();

    let mut entries = Vec::with_capacity(files.len());
    let mut processed = 0usize;
    let mut failed = 0usize;

    for input_path in &files {
        match nine_slice::process(input_path, output, &core_opts) {
            Ok(report) => {
                processed += 1;
                entries.push(FileEntry {
                    input: input_path.clone(),
                    outputs: report.output_files,
                    status: "ok",
                    error: None,
                });
            }
            Err(err) => {
                failed += 1;
                entries.push(FileEntry {
                    input: input_path.clone(),
                    outputs: Vec::new(),
                    status: "failed",
                    error: Some(err.to_string()),
                });
            }
        }
    }

    Ok(JsonResponse {
        processed,
        failed,
        duration_ms: start.elapsed().as_millis(),
        files: entries,
    })
}

async fn handle_multipart(multipart: Multipart) -> AppResult<Response> {
    let uploaded = collect_multipart(multipart, DEFAULT_UPLOAD_NAME).await?;
    let opts = match uploaded.options_json.as_deref() {
        Some(s) if !s.trim().is_empty() => serde_json::from_str::<ApiOptions>(s)?,
        _ => ApiOptions::default(),
    };

    let path = uploaded.path.clone();
    let core_opts = opts.core();
    let out_dir = uploaded.temp_dir.path().to_path_buf();

    let (bytes, ct) = tokio::task::spawn_blocking(move || -> AppResult<(Vec<u8>, String)> {
        let report = nine_slice::process(&path, &out_dir, &core_opts).map_err(AppError::Core)?;
        let first = report
            .output_files
            .first()
            .ok_or_else(|| AppError::Internal("no output files".into()))?;
        let bytes = std::fs::read(first)?;
        let ext = first.extension().and_then(|e| e.to_str()).unwrap_or("");
        let ct = match ext {
            "json" => "application/json",
            "webp" => "image/webp",
            _ => "image/png",
        };
        Ok((bytes, ct.to_string()))
    })
    .await
    .map_err(|e| AppError::Internal(format!("join error: {e}")))??;

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_str(&ct).unwrap());

    Ok((StatusCode::OK, headers, Body::from(bytes)).into_response())
}
