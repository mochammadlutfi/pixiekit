//! POST /api/audio — LUFS normalize, trim silence, format convert.
//!
//! Two modes (mirrors bg-remove / video-to-sprite):
//! - `application/json`  → server-side path batch
//! - `multipart/form-data` → single audio upload, returns processed bytes plus
//!   `X-Pixiekit-Duration-Ms-In`, `X-Pixiekit-Duration-Ms-Out`,
//!   `X-Pixiekit-Integrated-Lufs`

use std::path::{Path, PathBuf};
use std::time::Instant;

use axum::body::Body;
use axum::extract::{FromRequest, Multipart, Request};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use pixiekit_core::{audio, batch};

use crate::error::{AppError, AppResult};
use crate::upload::collect_multipart;

const ALLOWED_AUDIO_EXTS: &[&str] = &["wav", "mp3", "ogg", "m4a", "flac", "opus"];
const DEFAULT_UPLOAD_NAME: &str = "upload.audio";

pub fn router() -> Router {
    Router::new().route("/api/audio", post(handle))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TargetFormat {
    #[default]
    Ogg,
    Opus,
    Mp3,
    Wav,
}

impl From<TargetFormat> for audio::TargetFormat {
    fn from(v: TargetFormat) -> Self {
        match v {
            TargetFormat::Ogg => audio::TargetFormat::Ogg,
            TargetFormat::Opus => audio::TargetFormat::Opus,
            TargetFormat::Mp3 => audio::TargetFormat::Mp3,
            TargetFormat::Wav => audio::TargetFormat::Wav,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Channels {
    Mono,
    Stereo,
    #[default]
    Keep,
}

impl From<Channels> for audio::Channels {
    fn from(v: Channels) -> Self {
        match v {
            Channels::Mono => audio::Channels::Mono,
            Channels::Stereo => audio::Channels::Stereo,
            Channels::Keep => audio::Channels::Keep,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiOptions {
    #[serde(default)]
    pub target_format: TargetFormat,
    #[serde(default = "default_target_lufs")]
    pub target_lufs: f32,
    #[serde(default = "default_normalize")]
    pub normalize: bool,
    #[serde(default = "default_trim_silence")]
    pub trim_silence: bool,
    #[serde(default = "default_silence_threshold_db")]
    pub silence_threshold_db: f32,
    #[serde(default = "default_sample_rate")]
    pub sample_rate: u32,
    #[serde(default)]
    pub channels: Channels,
    #[serde(default = "default_bitrate_kbps")]
    pub bitrate_kbps: u16,
}

impl Default for ApiOptions {
    fn default() -> Self {
        Self {
            target_format: TargetFormat::default(),
            target_lufs: default_target_lufs(),
            normalize: default_normalize(),
            trim_silence: default_trim_silence(),
            silence_threshold_db: default_silence_threshold_db(),
            sample_rate: default_sample_rate(),
            channels: Channels::default(),
            bitrate_kbps: default_bitrate_kbps(),
        }
    }
}

impl ApiOptions {
    fn core(&self) -> audio::Options {
        audio::Options {
            target_format: self.target_format.into(),
            target_lufs: self.target_lufs,
            normalize: self.normalize,
            trim_silence: self.trim_silence,
            silence_threshold_db: self.silence_threshold_db,
            sample_rate: self.sample_rate,
            channels: self.channels.into(),
            bitrate_kbps: self.bitrate_kbps,
        }
    }
}

fn default_target_lufs() -> f32 {
    -16.0
}
fn default_normalize() -> bool {
    true
}
fn default_trim_silence() -> bool {
    true
}
fn default_silence_threshold_db() -> f32 {
    -50.0
}
fn default_sample_rate() -> u32 {
    44_100
}
fn default_bitrate_kbps() -> u16 {
    128
}

#[derive(Debug, Deserialize)]
pub struct JsonRequest {
    pub input: PathBuf,
    pub output: PathBuf,
    #[serde(default)]
    pub options: ApiOptions,
}

#[derive(Debug, Serialize)]
pub struct AudioEntry {
    pub input: PathBuf,
    pub output: Option<PathBuf>,
    pub duration_ms_in: Option<u32>,
    pub duration_ms_out: Option<u32>,
    pub integrated_lufs: Option<f32>,
    pub status: &'static str,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct JsonResponse {
    pub processed: usize,
    pub failed: usize,
    pub duration_ms: u128,
    pub files: Vec<AudioEntry>,
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
    let inputs = batch::list_images(input, false, ALLOWED_AUDIO_EXTS)?;
    if inputs.is_empty() {
        return Ok(JsonResponse {
            processed: 0,
            failed: 0,
            duration_ms: 0,
            files: Vec::new(),
        });
    }

    std::fs::create_dir_all(output)?;
    audio::check_ffmpeg()?;
    let core_opts = opts.core();
    let ext = core_opts.target_format.extension();
    let start = Instant::now();

    let mut entries = Vec::with_capacity(inputs.len());
    let mut processed = 0usize;
    let mut failed = 0usize;

    for input_path in &inputs {
        let stem = match input_path.file_stem() {
            Some(s) => s.to_string_lossy().into_owned(),
            None => {
                failed += 1;
                entries.push(AudioEntry {
                    input: input_path.clone(),
                    output: None,
                    duration_ms_in: None,
                    duration_ms_out: None,
                    integrated_lufs: None,
                    status: "failed",
                    error: Some(format!("invalid filename: {}", input_path.display())),
                });
                continue;
            }
        };
        let out_path = output.join(format!("{stem}.{ext}"));
        match audio::process(input_path, &out_path, &core_opts) {
            Ok(report) => {
                processed += 1;
                entries.push(AudioEntry {
                    input: input_path.clone(),
                    output: Some(out_path),
                    duration_ms_in: Some(report.duration_ms_in),
                    duration_ms_out: Some(report.duration_ms_out),
                    integrated_lufs: report.integrated_lufs,
                    status: "ok",
                    error: None,
                });
            }
            Err(err) => {
                failed += 1;
                entries.push(AudioEntry {
                    input: input_path.clone(),
                    output: None,
                    duration_ms_in: None,
                    duration_ms_out: None,
                    integrated_lufs: None,
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

    let core_opts = opts.core();
    let ext = core_opts.target_format.extension();
    let content_type = core_opts.target_format.content_type();

    let input_path = uploaded.path.clone();
    let out_path = uploaded.temp_dir.path().join(format!("out.{ext}"));
    let out_path_for_task = out_path.clone();

    let report = tokio::task::spawn_blocking(move || -> AppResult<audio::AudioReport> {
        audio::check_ffmpeg()?;
        Ok(audio::process(&input_path, &out_path_for_task, &core_opts)?)
    })
    .await
    .map_err(|e| AppError::Internal(format!("join error: {e}")))??;

    let bytes = tokio::fs::read(&out_path).await?;

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    headers.insert(
        "x-pixiekit-duration-ms-in",
        HeaderValue::from_str(&report.duration_ms_in.to_string())
            .map_err(|e| AppError::Internal(e.to_string()))?,
    );
    headers.insert(
        "x-pixiekit-duration-ms-out",
        HeaderValue::from_str(&report.duration_ms_out.to_string())
            .map_err(|e| AppError::Internal(e.to_string()))?,
    );
    if let Some(lufs) = report.integrated_lufs {
        headers.insert(
            "x-pixiekit-integrated-lufs",
            HeaderValue::from_str(&format!("{lufs}"))
                .map_err(|e| AppError::Internal(e.to_string()))?,
        );
    }

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
        let payload = json!({"input": "/tmp/a", "output": "/tmp/out"});
        let req: JsonRequest = serde_json::from_value(payload).unwrap();
        assert_eq!(req.options.target_format, TargetFormat::Ogg);
        assert!((req.options.target_lufs - -16.0).abs() < f32::EPSILON);
        assert!(req.options.normalize);
        assert!(req.options.trim_silence);
        assert!((req.options.silence_threshold_db - -50.0).abs() < f32::EPSILON);
        assert_eq!(req.options.sample_rate, 44_100);
        assert_eq!(req.options.channels, Channels::Keep);
        assert_eq!(req.options.bitrate_kbps, 128);
    }

    #[test]
    fn parses_custom_options() {
        let payload = json!({
            "input": "/tmp/a",
            "output": "/tmp/out",
            "options": {
                "target_format": "mp3",
                "target_lufs": -19.0,
                "normalize": false,
                "trim_silence": false,
                "silence_threshold_db": -40.0,
                "sample_rate": 48000,
                "channels": "mono",
                "bitrate_kbps": 192
            }
        });
        let req: JsonRequest = serde_json::from_value(payload).unwrap();
        assert_eq!(req.options.target_format, TargetFormat::Mp3);
        assert!((req.options.target_lufs - -19.0).abs() < f32::EPSILON);
        assert!(!req.options.normalize);
        assert!(!req.options.trim_silence);
        assert_eq!(req.options.sample_rate, 48000);
        assert_eq!(req.options.channels, Channels::Mono);
        assert_eq!(req.options.bitrate_kbps, 192);
    }

    #[tokio::test]
    async fn json_mode_missing_path_returns_404() {
        let app = router();
        let body = json!({
            "input": "/path/does/not/exist/audio-xyz",
            "output": "/tmp/pixiekit-audio-out-test"
        });
        let response = app
            .oneshot(
                Request::post("/api/audio")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn json_mode_empty_dir_returns_200_with_zero() {
        let app = router();
        let dir = std::env::temp_dir().join(format!("pixiekit-audio-empty-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let body = json!({
            "input": dir.to_string_lossy(),
            "output": dir.to_string_lossy()
        });
        let response = app
            .oneshot(
                Request::post("/api/audio")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn rejects_unknown_content_type() {
        let app = router();
        let response = app
            .oneshot(
                Request::post("/api/audio")
                    .header("content-type", "text/plain")
                    .body(Body::from("hello"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
