//! POST /api/bg-remove — chroma key + despill + alpha erode.
//!
//! Two modes, dispatched on the `Content-Type` header:
//! - `application/json`  → batch processing of a server-side path
//! - `multipart/form-data` → single-file upload, returns image bytes

use std::path::{Path, PathBuf};
use std::time::Instant;

use axum::body::Body;
use axum::extract::{FromRequest, Multipart, Request};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use pixiekit_core::{batch, bg_remove};

use crate::error::{AppError, AppResult};
use crate::upload::collect_multipart;

const ALLOWED_EXTS: &[&str] = &["png", "jpg", "jpeg", "webp"];
const DEFAULT_UPLOAD_NAME: &str = "upload.png";

pub fn router() -> Router {
    Router::new().route("/api/bg-remove", post(handle))
}

/// Output container for Mode A (JSON path mode).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    #[default]
    Png,
    Webp,
}

impl OutputFormat {
    fn extension(self) -> &'static str {
        match self {
            OutputFormat::Png => "png",
            OutputFormat::Webp => "webp",
        }
    }
}

/// API-facing options. `target_color` accepts either a hex string ("#00FF00")
/// or a `[u8; 3]` array; both deserialize cleanly.
#[derive(Debug, Clone, Deserialize)]
pub struct ApiOptions {
    #[serde(default = "default_target", deserialize_with = "deserialize_color")]
    pub target_color: [u8; 3],
    #[serde(default = "default_fuzz")]
    pub fuzz: f32,
    #[serde(default = "default_despill")]
    pub despill: bool,
    #[serde(default = "default_erode")]
    pub erode: u8,
    #[serde(default)]
    pub format: OutputFormat,
}

impl Default for ApiOptions {
    fn default() -> Self {
        Self {
            target_color: default_target(),
            fuzz: default_fuzz(),
            despill: default_despill(),
            erode: default_erode(),
            format: OutputFormat::default(),
        }
    }
}

impl ApiOptions {
    fn core(&self) -> bg_remove::Options {
        bg_remove::Options {
            target_color: self.target_color,
            fuzz: self.fuzz,
            despill: self.despill,
            erode: self.erode,
        }
    }
}

fn default_target() -> [u8; 3] {
    [0, 255, 0]
}
fn default_fuzz() -> f32 {
    0.35
}
fn default_despill() -> bool {
    true
}
fn default_erode() -> u8 {
    1
}

/// Hex string ("#00FF00") or `[u8; 3]` array → `[u8; 3]`.
fn deserialize_color<'de, D>(de: D) -> Result<[u8; 3], D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Repr {
        Hex(String),
        Array([u8; 3]),
    }

    match Repr::deserialize(de)? {
        Repr::Array(a) => Ok(a),
        Repr::Hex(s) => parse_hex(&s).map_err(D::Error::custom),
    }
}

fn parse_hex(s: &str) -> Result<[u8; 3], String> {
    let trimmed = s.trim_start_matches('#');
    if trimmed.len() != 6 {
        return Err(format!("hex color must be 6 chars, got {:?}", s));
    }
    let r = u8::from_str_radix(&trimmed[0..2], 16).map_err(|e| e.to_string())?;
    let g = u8::from_str_radix(&trimmed[2..4], 16).map_err(|e| e.to_string())?;
    let b = u8::from_str_radix(&trimmed[4..6], 16).map_err(|e| e.to_string())?;
    Ok([r, g, b])
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
    // Move the (potentially CPU-bound) work onto a blocking thread.
    tokio::task::spawn_blocking(move || run_batch(&req.input, &req.output, &req.options))
        .await
        .map_err(|e| AppError::Internal(format!("join error: {e}")))?
}

fn run_batch(input: &Path, output: &Path, opts: &ApiOptions) -> AppResult<JsonResponse> {
    let files = batch::list_images(input, false, ALLOWED_EXTS)?;
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
    let ext = opts.format.extension();
    let start = Instant::now();

    let mut entries = Vec::with_capacity(files.len());
    let mut processed = 0usize;
    let mut failed = 0usize;

    for input_path in &files {
        match process_one(input_path, output, &core_opts, ext) {
            Ok(out_path) => {
                processed += 1;
                entries.push(FileEntry {
                    input: input_path.clone(),
                    output: Some(out_path),
                    status: "ok",
                    error: None,
                });
            }
            Err(err) => {
                failed += 1;
                entries.push(FileEntry {
                    input: input_path.clone(),
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

fn process_one(
    input_path: &Path,
    output_dir: &Path,
    opts: &bg_remove::Options,
    ext: &str,
) -> AppResult<PathBuf> {
    let img = image::open(input_path)?.into_rgba8();
    let processed = bg_remove::process(&img, opts);

    let stem = input_path
        .file_stem()
        .ok_or_else(|| AppError::BadRequest(format!("invalid filename: {}", input_path.display())))?
        .to_string_lossy()
        .into_owned();
    let output_path = output_dir.join(format!("{stem}.{ext}"));

    save_image(&processed, &output_path, ext)?;
    Ok(output_path)
}

fn save_image(img: &image::RgbaImage, path: &Path, ext: &str) -> AppResult<()> {
    match ext {
        "webp" => {
            let encoder = webp_encode(img)?;
            std::fs::write(path, encoder)?;
        }
        _ => {
            img.save(path)?;
        }
    }
    Ok(())
}

fn webp_encode(img: &image::RgbaImage) -> AppResult<Vec<u8>> {
    // Encode RGBA → WebP with lossless alpha at quality 90 (matches CLI default).
    let encoder = webp::Encoder::from_rgba(img.as_raw(), img.width(), img.height());
    Ok(encoder.encode(90.0).to_vec())
}

async fn handle_multipart(multipart: Multipart) -> AppResult<Response> {
    let uploaded = collect_multipart(multipart, DEFAULT_UPLOAD_NAME).await?;

    let opts = match uploaded.options_json.as_deref() {
        Some(s) if !s.trim().is_empty() => serde_json::from_str::<ApiOptions>(s)?,
        _ => ApiOptions::default(),
    };

    let path = uploaded.path.clone();
    let core_opts = opts.core();
    let ext = opts.format.extension();

    let bytes = tokio::task::spawn_blocking(move || -> AppResult<Vec<u8>> {
        let img = image::open(&path)?.into_rgba8();
        let processed = bg_remove::process(&img, &core_opts);
        encode_to_bytes(&processed, ext)
    })
    .await
    .map_err(|e| AppError::Internal(format!("join error: {e}")))??;

    // `uploaded.temp_dir` is dropped here, removing the temp file.
    drop(uploaded.temp_dir);

    let mut headers = HeaderMap::new();
    let ct = match ext {
        "webp" => "image/webp",
        _ => "image/png",
    };
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(ct));

    Ok((StatusCode::OK, headers, Body::from(bytes)).into_response())
}

fn encode_to_bytes(img: &image::RgbaImage, ext: &str) -> AppResult<Vec<u8>> {
    match ext {
        "webp" => webp_encode(img),
        _ => {
            let mut buf = std::io::Cursor::new(Vec::new());
            img.write_to(&mut buf, image::ImageFormat::Png)?;
            Ok(buf.into_inner())
        }
    }
}

// ---- helpers shared by tests ----

#[cfg(test)]
fn parse_payload(value: &serde_json::Value) -> Result<JsonRequest, serde_json::Error> {
    serde_json::from_value(value.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use serde_json::json;
    use tower::ServiceExt;

    #[test]
    fn parses_hex_target_color() {
        let payload = json!({
            "input": "/tmp/in",
            "output": "/tmp/out",
            "options": { "target_color": "#FF0080", "fuzz": 0.5 }
        });
        let req = parse_payload(&payload).unwrap();
        assert_eq!(req.options.target_color, [0xFF, 0x00, 0x80]);
        assert!((req.options.fuzz - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn parses_array_target_color() {
        let payload = json!({
            "input": "/tmp/in",
            "output": "/tmp/out",
            "options": { "target_color": [10, 20, 30] }
        });
        let req = parse_payload(&payload).unwrap();
        assert_eq!(req.options.target_color, [10, 20, 30]);
    }

    #[test]
    fn defaults_when_options_omitted() {
        let payload = json!({"input": "/tmp/in", "output": "/tmp/out"});
        let req = parse_payload(&payload).unwrap();
        assert_eq!(req.options.target_color, [0, 255, 0]);
        assert!(req.options.despill);
        assert_eq!(req.options.erode, 1);
        assert_eq!(req.options.format, OutputFormat::Png);
    }

    #[tokio::test]
    async fn json_mode_missing_path_returns_404() {
        let app = router();
        let body = json!({
            "input": "/path/definitely/does/not/exist/xyz123",
            "output": "/tmp/pixiekit-out-test"
        });
        let response = app
            .oneshot(
                Request::post("/api/bg-remove")
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
                Request::post("/api/bg-remove")
                    .header("content-type", "text/plain")
                    .body(Body::from("hello"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn json_mode_processes_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let app = router();
        let body = json!({
            "input": dir.path(),
            "output": out.path()
        });
        let response = app
            .oneshot(
                Request::post("/api/bg-remove")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
