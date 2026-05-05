//! POST /api/video-to-sprite — extract video frames into a horizontal sprite sheet.
//!
//! Two modes (same dispatch as bg-remove):
//! - `application/json`  → server-side path batch
//! - `multipart/form-data` → single video upload, returns sprite bytes plus
//!   `X-Pixiekit-Frame-Count`, `X-Pixiekit-FPS`, `X-Pixiekit-Frame-Size`

use std::path::{Path, PathBuf};
use std::time::Instant;

use axum::body::Body;
use axum::extract::{FromRequest, Multipart, Request};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use pixiekit_core::{batch, bg_remove, video_to_sprite};

use crate::error::{AppError, AppResult};
use crate::upload::collect_multipart;

const ALLOWED_VIDEO_EXTS: &[&str] = &["mp4", "mov", "webm"];
const DEFAULT_UPLOAD_NAME: &str = "upload.mp4";

pub fn router() -> Router {
    Router::new().route("/api/video-to-sprite", post(handle))
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiOptions {
    #[serde(default = "default_fps")]
    pub fps: u8,
    #[serde(default = "default_frame_size")]
    pub frame_size: u32,
    #[serde(default)]
    pub format: OutputFormat,
    #[serde(default = "default_webp_quality")]
    pub webp_quality: u8,
    #[serde(default)]
    pub chroma_key: Option<bg_remove::Options>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    Png,
    #[default]
    Webp,
}

impl From<OutputFormat> for video_to_sprite::OutputFormat {
    fn from(v: OutputFormat) -> Self {
        match v {
            OutputFormat::Png => video_to_sprite::OutputFormat::Png,
            OutputFormat::Webp => video_to_sprite::OutputFormat::Webp,
        }
    }
}

impl Default for ApiOptions {
    fn default() -> Self {
        Self {
            fps: default_fps(),
            frame_size: default_frame_size(),
            format: OutputFormat::default(),
            webp_quality: default_webp_quality(),
            chroma_key: None,
        }
    }
}

impl ApiOptions {
    fn core(&self) -> video_to_sprite::Options {
        video_to_sprite::Options {
            fps: self.fps,
            frame_size: self.frame_size,
            output_format: self.format.into(),
            webp_quality: self.webp_quality,
            chroma_key: self.chroma_key.clone(),
        }
    }
}

fn default_fps() -> u8 {
    8
}
fn default_frame_size() -> u32 {
    256
}
fn default_webp_quality() -> u8 {
    90
}

#[derive(Debug, Deserialize)]
pub struct JsonRequest {
    pub input: PathBuf,
    pub output: PathBuf,
    #[serde(default)]
    pub options: ApiOptions,
}

#[derive(Debug, Serialize)]
pub struct VideoEntry {
    pub input: PathBuf,
    pub sprite: Option<PathBuf>,
    pub metadata: Option<PathBuf>,
    pub frame_count: Option<u32>,
    pub frame_size: Option<u32>,
    pub status: &'static str,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct JsonResponse {
    pub processed: usize,
    pub failed: usize,
    pub duration_ms: u128,
    pub files: Vec<VideoEntry>,
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
        let payload: JsonRequest = serde_json::from_slice(&bytes)?;
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
    let videos = batch::list_images(input, false, ALLOWED_VIDEO_EXTS)?;
    if videos.is_empty() {
        return Ok(JsonResponse {
            processed: 0,
            failed: 0,
            duration_ms: 0,
            files: Vec::new(),
        });
    }

    std::fs::create_dir_all(output)?;
    video_to_sprite::check_ffmpeg()?;
    let core_opts = opts.core();
    let start = Instant::now();

    let mut entries = Vec::with_capacity(videos.len());
    let mut processed = 0usize;
    let mut failed = 0usize;

    for video_path in &videos {
        match video_to_sprite::process(video_path, output, &core_opts) {
            Ok(report) => {
                processed += 1;
                entries.push(VideoEntry {
                    input: video_path.clone(),
                    sprite: Some(report.sprite_path),
                    metadata: Some(report.metadata_path),
                    frame_count: Some(report.frame_count),
                    frame_size: Some(report.frame_size),
                    status: "ok",
                    error: None,
                });
            }
            Err(err) => {
                failed += 1;
                entries.push(VideoEntry {
                    input: video_path.clone(),
                    sprite: None,
                    metadata: None,
                    frame_count: None,
                    frame_size: None,
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

    let video_path = uploaded.path.clone();
    let out_dir = uploaded.temp_dir.path().join("out");
    std::fs::create_dir_all(&out_dir)?;
    let core_opts = opts.core();
    let ext = match opts.format {
        OutputFormat::Png => "png",
        OutputFormat::Webp => "webp",
    };

    let report =
        tokio::task::spawn_blocking(move || -> AppResult<video_to_sprite::ProcessReport> {
            video_to_sprite::check_ffmpeg()?;
            Ok(video_to_sprite::process(&video_path, &out_dir, &core_opts)?)
        })
        .await
        .map_err(|e| AppError::Internal(format!("join error: {e}")))??;

    let bytes = tokio::fs::read(&report.sprite_path).await?;

    let mut headers = HeaderMap::new();
    let ct = match ext {
        "webp" => "image/webp",
        _ => "image/png",
    };
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(ct));
    headers.insert(
        "x-pixiekit-frame-count",
        HeaderValue::from_str(&report.frame_count.to_string())
            .map_err(|e| AppError::Internal(e.to_string()))?,
    );
    headers.insert(
        "x-pixiekit-fps",
        HeaderValue::from_str(&opts.fps.to_string())
            .map_err(|e| AppError::Internal(e.to_string()))?,
    );
    headers.insert(
        "x-pixiekit-frame-size",
        HeaderValue::from_str(&report.frame_size.to_string())
            .map_err(|e| AppError::Internal(e.to_string()))?,
    );

    // temp_dir drops here once the response is built.
    drop(uploaded.temp_dir);

    Ok((StatusCode::OK, headers, Body::from(bytes)).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use serde_json::json;
    use tower::ServiceExt;

    #[test]
    fn parses_default_options() {
        let payload = json!({"input": "/tmp/v", "output": "/tmp/out"});
        let req: JsonRequest = serde_json::from_value(payload).unwrap();
        assert_eq!(req.options.fps, 8);
        assert_eq!(req.options.frame_size, 256);
        assert_eq!(req.options.format, OutputFormat::Webp);
        assert!(req.options.chroma_key.is_none());
    }

    #[test]
    fn parses_chroma_key_options() {
        let payload = json!({
            "input": "/tmp/v",
            "output": "/tmp/out",
            "options": {
                "fps": 12,
                "frame_size": 128,
                "format": "png",
                "chroma_key": {"target_color": [0,255,0], "fuzz": 0.4, "despill": true, "erode": 1}
            }
        });
        let req: JsonRequest = serde_json::from_value(payload).unwrap();
        assert_eq!(req.options.fps, 12);
        assert_eq!(req.options.frame_size, 128);
        assert_eq!(req.options.format, OutputFormat::Png);
        assert!(req.options.chroma_key.is_some());
    }

    #[tokio::test]
    async fn json_mode_missing_path_returns_404() {
        let app = router();
        let body = json!({
            "input": "/path/does/not/exist/abc-xyz-789",
            "output": "/tmp/pixiekit-v2s-out-test"
        });
        let response = app
            .oneshot(
                Request::post("/api/video-to-sprite")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn rejects_unknown_content_type() {
        let app = router();
        let response = app
            .oneshot(
                Request::post("/api/video-to-sprite")
                    .header("content-type", "text/plain")
                    .body(Body::from("hello"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
