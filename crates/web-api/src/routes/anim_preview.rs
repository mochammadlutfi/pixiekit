//! POST /api/anim-preview — convert sprite sheet or frame folder to animation.
//!
//! Two modes:
//! - JSON: Batch process folder/file.
//! - Multipart: Single file upload, returns the animation file.

use std::path::{Path, PathBuf};
use std::time::Instant;

use axum::body::Body;
use axum::extract::{FromRequest, Multipart, Request};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use pixiekit_core::anim_preview;

use crate::error::{AppError, AppResult};
use crate::upload::collect_multipart;

const ALLOWED_EXTS: &[&str] = &["png", "jpg", "jpeg", "webp"];
const DEFAULT_UPLOAD_NAME: &str = "upload.png";

pub fn router() -> Router {
    Router::new().route("/api/anim-preview", post(handle))
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiOptions {
    #[serde(default = "default_fps")]
    pub fps: u8,
    #[serde(default = "default_format")]
    pub format: anim_preview::PreviewFormat,
    #[serde(default = "default_loop")]
    pub loop_anim: bool,
    #[serde(default = "default_upscale")]
    pub upscale: u8,
    pub frame_size: Option<u32>,
}

fn default_fps() -> u8 { 8 }
fn default_format() -> anim_preview::PreviewFormat { anim_preview::PreviewFormat::Gif }
fn default_loop() -> bool { true }
fn default_upscale() -> u8 { 1 }

impl Default for ApiOptions {
    fn default() -> Self {
        Self {
            fps: default_fps(),
            format: default_format(),
            loop_anim: default_loop(),
            upscale: default_upscale(),
            frame_size: None,
        }
    }
}

impl ApiOptions {
    fn core(&self) -> anim_preview::Options {
        anim_preview::Options {
            fps: self.fps,
            output_format: self.format,
            loop_anim: self.loop_anim,
            upscale: self.upscale,
            frame_size: self.frame_size,
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
    pub output: Option<PathBuf>,
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
    // For anim_preview, input can be a file (sprite sheet) or a directory (frame folder).
    // If it's a directory, we might want to process it as ONE animation OR multiple if it contains subdirectories.
    // Existing pattern in core::anim_preview handles one input -> one output.
    
    // In batch mode, if input is a directory, we'll check if it's a "frame folder" (contains images).
    // If it contains images directly, we process it as one.
    // If it contains subdirectories, we process each subdirectory.
    
    let mut targets = Vec::new();
    if input.is_file() {
        targets.push(input.to_path_buf());
    } else {
        // Does it contain images directly?
        let has_images = std::fs::read_dir(input)?
            .filter_map(|e| e.ok())
            .any(|e| e.path().extension().is_some_and(|ext| ALLOWED_EXTS.contains(&ext.to_str().unwrap_or("").to_lowercase().as_str())));
            
        if has_images {
            targets.push(input.to_path_buf());
        } else {
            // Check subdirectories
            for entry in std::fs::read_dir(input)? {
                let entry = entry?;
                if entry.path().is_dir() {
                    targets.push(entry.path());
                } else if entry.path().is_file() {
                    if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
                        if ALLOWED_EXTS.contains(&ext.to_lowercase().as_str()) {
                            targets.push(entry.path());
                        }
                    }
                }
            }
        }
    }

    if targets.is_empty() {
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

    let mut entries = Vec::with_capacity(targets.len());
    let mut processed = 0usize;
    let mut failed = 0usize;

    for target in &targets {
        match anim_preview::process(target, output, &core_opts) {
            Ok(report) => {
                processed += 1;
                entries.push(FileEntry {
                    input: target.clone(),
                    output: Some(report.output_path),
                    status: "ok",
                    error: None,
                });
            }
            Err(err) => {
                failed += 1;
                entries.push(FileEntry {
                    input: target.clone(),
                    output: None,
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
        let report = anim_preview::process(&path, &out_dir, &core_opts).map_err(AppError::Core)?;
        let output_path = report.output_path;
        let bytes = std::fs::read(&output_path)?;
        
        let ct = match report.format {
            anim_preview::PreviewFormat::Gif => "image/gif",
            anim_preview::PreviewFormat::Mp4 => "video/mp4",
            anim_preview::PreviewFormat::Webm => "video/webm",
        };
        
        Ok((bytes, ct.to_string()))
    })
    .await
    .map_err(|e| AppError::Internal(format!("join error: {e}")))??;

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_str(&ct).unwrap());

    Ok((StatusCode::OK, headers, Body::from(bytes)).into_response())
}
