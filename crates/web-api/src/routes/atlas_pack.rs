//! POST /api/atlas-pack — pack a folder of PNG sprites into a texture atlas
//! plus Flame-compatible JSON metadata.
//!
//! JSON-only (atlas pack inherently consumes a folder of files; multipart
//! upload is not a natural fit and is intentionally unsupported here).

use std::path::PathBuf;
use std::time::Instant;

use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use pixiekit_core::{atlas_pack, batch};

use crate::error::{AppError, AppResult};

const ALLOWED_EXTS: &[&str] = &["png"];

pub fn router() -> Router {
    Router::new().route("/api/atlas-pack", post(handle))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    #[default]
    Png,
    Webp,
}

impl From<OutputFormat> for atlas_pack::OutputFormat {
    fn from(v: OutputFormat) -> Self {
        match v {
            OutputFormat::Png => atlas_pack::OutputFormat::Png,
            OutputFormat::Webp => atlas_pack::OutputFormat::Webp,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiOptions {
    #[serde(default = "default_name")]
    pub name: String,
    #[serde(default = "default_max_size")]
    pub max_size: u16,
    #[serde(default = "default_padding")]
    pub padding: u8,
    #[serde(default = "default_extrude")]
    pub extrude: u8,
    #[serde(default = "default_true")]
    pub power_of_two: bool,
    #[serde(default = "default_true")]
    pub trim: bool,
    #[serde(default)]
    pub format: OutputFormat,
    #[serde(default = "default_webp_quality")]
    pub webp_quality: u8,
}

impl Default for ApiOptions {
    fn default() -> Self {
        Self {
            name: default_name(),
            max_size: default_max_size(),
            padding: default_padding(),
            extrude: default_extrude(),
            power_of_two: true,
            trim: true,
            format: OutputFormat::default(),
            webp_quality: default_webp_quality(),
        }
    }
}

fn default_name() -> String {
    "atlas".to_string()
}
fn default_max_size() -> u16 {
    2048
}
fn default_padding() -> u8 {
    2
}
fn default_extrude() -> u8 {
    1
}
fn default_true() -> bool {
    true
}
fn default_webp_quality() -> u8 {
    90
}

impl ApiOptions {
    fn core(&self) -> atlas_pack::Options {
        atlas_pack::Options {
            name: self.name.clone(),
            max_size: self.max_size,
            padding: self.padding,
            extrude: self.extrude,
            power_of_two: self.power_of_two,
            trim: self.trim,
            format: self.format.into(),
            webp_quality: self.webp_quality,
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

#[derive(Debug, Serialize, Clone, Copy)]
pub struct AtlasSize {
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Serialize)]
pub struct JsonResponse {
    pub atlas_path: Option<PathBuf>,
    pub metadata_path: Option<PathBuf>,
    pub packed: u32,
    pub total: u32,
    pub atlas_size: AtlasSize,
    pub efficiency: f32,
    pub duration_ms: u128,
}

async fn handle(Json(req): Json<JsonRequest>) -> AppResult<impl IntoResponse> {
    let opts = req.options.core();

    let input = req.input.clone();
    let sprites =
        tokio::task::spawn_blocking(move || batch::list_images(&input, false, ALLOWED_EXTS))
            .await
            .map_err(|e| AppError::Internal(format!("Join error: {e}")))?
            .map_err(AppError::Core)?;

    if sprites.is_empty() {
        return Ok((
            StatusCode::OK,
            Json(JsonResponse {
                atlas_path: None,
                metadata_path: None,
                packed: 0,
                total: 0,
                atlas_size: AtlasSize { w: 0, h: 0 },
                efficiency: 0.0,
                duration_ms: 0,
            }),
        ));
    }

    let output_dir = req.output.clone();
    tokio::fs::create_dir_all(&output_dir).await?;

    let start = Instant::now();
    let report =
        tokio::task::spawn_blocking(move || atlas_pack::process(&sprites, &output_dir, &opts))
            .await
            .map_err(|e| AppError::Internal(format!("Join error: {e}")))?
            .map_err(AppError::Core)?;
    let duration_ms = start.elapsed().as_millis();

    Ok((
        StatusCode::OK,
        Json(JsonResponse {
            atlas_path: Some(report.atlas_path),
            metadata_path: Some(report.metadata_path),
            packed: report.packed,
            total: report.total,
            atlas_size: AtlasSize {
                w: report.atlas_size.0,
                h: report.atlas_size.1,
            },
            efficiency: report.efficiency,
            duration_ms,
        }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    #[tokio::test]
    async fn atlas_pack_empty_dir_returns_200_with_zero() {
        let dir =
            std::env::temp_dir().join(format!("pixiekit-web-atlas-empty-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let app = router();
        let body = serde_json::json!({
            "input": dir,
            "output": dir,
        });
        let response = app
            .oneshot(
                Request::post("/api/atlas-pack")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["packed"], 0);
        assert_eq!(json["total"], 0);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn atlas_pack_missing_input_returns_404() {
        let app = router();
        let body = serde_json::json!({
            "input": "/nope/does/not/exist/atlas-pack-xyz",
            "output": "/tmp"
        });
        let response = app
            .oneshot(
                Request::post("/api/atlas-pack")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn parses_default_options() {
        let payload = serde_json::json!({"input": "/tmp/in", "output": "/tmp/out"});
        let req: JsonRequest = serde_json::from_value(payload).unwrap();
        assert_eq!(req.options.name, "atlas");
        assert_eq!(req.options.max_size, 2048);
        assert_eq!(req.options.padding, 2);
        assert!(req.options.power_of_two);
        assert!(req.options.trim);
        assert_eq!(req.options.format, OutputFormat::Png);
    }

    #[test]
    fn parses_custom_options() {
        let payload = serde_json::json!({
            "input": "/tmp/in",
            "output": "/tmp/out",
            "options": {
                "name": "domdom",
                "max_size": 1024,
                "padding": 4,
                "extrude": 2,
                "power_of_two": false,
                "trim": false,
                "format": "webp",
                "webp_quality": 80
            }
        });
        let req: JsonRequest = serde_json::from_value(payload).unwrap();
        assert_eq!(req.options.name, "domdom");
        assert_eq!(req.options.max_size, 1024);
        assert!(!req.options.power_of_two);
        assert_eq!(req.options.format, OutputFormat::Webp);
    }
}
