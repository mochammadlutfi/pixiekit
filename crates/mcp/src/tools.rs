//! Tool definitions and `tools/call` dispatcher.
//!
//! Each tool corresponds to a function in `pixiekit_core::*`. Schemas mirror
//! PRD §7.3.2. Handlers convert MCP arguments → core options, execute, and
//! shape the response per PRD §7.3.3.

use std::path::{Path, PathBuf};
use std::time::Instant;

use rayon::prelude::*;
use serde_json::{json, Value};

use pixiekit_core::{batch, bg_remove, video_to_sprite};

use crate::server::{ERR_INTERNAL, ERR_INVALID_PARAMS, ERR_METHOD_NOT_FOUND};

/// Error returned by a tool handler. Maps directly to a JSON-RPC error.
#[derive(Debug)]
pub struct ToolError {
    pub code: i32,
    pub message: String,
}

impl ToolError {
    fn invalid_params(msg: impl Into<String>) -> Self {
        Self {
            code: ERR_INVALID_PARAMS,
            message: msg.into(),
        }
    }

    fn internal(msg: impl Into<String>) -> Self {
        Self {
            code: ERR_INTERNAL,
            message: msg.into(),
        }
    }

    fn unknown_tool(name: &str) -> Self {
        Self {
            code: ERR_METHOD_NOT_FOUND,
            message: format!("Unknown tool: {name}"),
        }
    }
}

/// Static catalog returned by `tools/list`. Schemas mirror PRD §7.3.2 verbatim.
pub fn list_tools() -> Vec<Value> {
    vec![
        json!({
            "name": "bg_remove",
            "description": "Remove green/blue screen background from images. Batch process folder.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "input": {"type": "string", "description": "Path to image file or folder"},
                    "output": {"type": "string", "description": "Output folder path"},
                    "target_color": {"type": "string", "default": "#00FF00", "pattern": "^#[0-9a-fA-F]{6}$"},
                    "fuzz": {"type": "number", "minimum": 0, "maximum": 1, "default": 0.35},
                    "despill": {"type": "boolean", "default": true},
                    "erode": {"type": "integer", "minimum": 0, "maximum": 5, "default": 1},
                    "format": {"type": "string", "enum": ["png", "webp"], "default": "png"}
                },
                "required": ["input", "output"]
            }
        }),
        json!({
            "name": "vectorize",
            "description": "Convert raster image (PNG/JPG/WebP) to SVG vector path.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "input": {"type": "string"},
                    "output": {"type": "string"},
                    "mode": {"type": "string", "enum": ["color", "binary"], "default": "color"},
                    "smooth": {"type": "integer", "minimum": 0, "maximum": 10, "default": 4}
                },
                "required": ["input", "output"]
            }
        }),
        json!({
            "name": "video_to_sprite",
            "description": "Extract frames from video and stitch into horizontal sprite sheet for game engines.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "input": {"type": "string"},
                    "output": {"type": "string"},
                    "fps": {"type": "integer", "minimum": 1, "maximum": 30, "default": 8},
                    "size": {"type": "integer", "minimum": 64, "maximum": 1024, "default": 256},
                    "format": {"type": "string", "enum": ["png", "webp"], "default": "webp"},
                    "chroma_key": {"type": "boolean", "default": false}
                },
                "required": ["input", "output"]
            }
        }),
        json!({
            "name": "list_presets",
            "description": "List saved processing presets.",
            "inputSchema": {"type": "object", "properties": {}}
        }),
    ]
}

/// Dispatch a `tools/call` request to the matching handler.
pub fn call(params: Option<&Value>) -> Result<Value, ToolError> {
    let params = params.ok_or_else(|| ToolError::invalid_params("Missing params"))?;
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::invalid_params("Missing tool name"))?;
    let args = params.get("arguments").cloned().unwrap_or(json!({}));

    match name {
        "bg_remove" => bg_remove_handler(&args),
        "video_to_sprite" => video_to_sprite_handler(&args),
        "vectorize" => Err(ToolError::internal(
            "Vectorize tool will be available after Phase 3 merge. Use CLI `pixiekit-cli vectorize` for now.",
        )),
        "list_presets" => list_presets_handler(),
        other => Err(ToolError::unknown_tool(other)),
    }
}

// ---------- bg_remove ----------

fn bg_remove_handler(args: &Value) -> Result<Value, ToolError> {
    let input = require_string(args, "input")?;
    let output = require_string(args, "output")?;

    let target_color_str = args
        .get("target_color")
        .and_then(|v| v.as_str())
        .unwrap_or("#00FF00");
    let target_color = parse_hex_color(target_color_str)
        .map_err(|e| ToolError::invalid_params(format!("target_color: {e}")))?;

    let fuzz = args.get("fuzz").and_then(|v| v.as_f64()).unwrap_or(0.35) as f32;
    let despill = args
        .get("despill")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let erode = args
        .get("erode")
        .and_then(|v| v.as_u64())
        .unwrap_or(1)
        .min(5) as u8;
    let format = args
        .get("format")
        .and_then(|v| v.as_str())
        .unwrap_or("png")
        .to_ascii_lowercase();
    if format != "png" && format != "webp" {
        return Err(ToolError::invalid_params(format!(
            "format: expected png|webp, got {format}"
        )));
    }

    let opts = bg_remove::Options {
        target_color,
        fuzz,
        despill,
        erode,
    };

    let input_path = PathBuf::from(input);
    let output_path = PathBuf::from(output);

    let files = batch::list_images(&input_path, false, &["png", "jpg", "jpeg", "webp"])
        .map_err(|e| ToolError::internal(format!("Listing input: {e}")))?;

    if files.is_empty() {
        return Ok(tool_text_result(
            format!("No images found in {}", input_path.display()),
            json!({
                "processed": 0,
                "failed": 0,
                "duration_ms": 0,
                "output_dir": output_path.to_string_lossy(),
                "files": [],
            }),
        ));
    }

    std::fs::create_dir_all(&output_path)
        .map_err(|e| ToolError::internal(format!("Creating output dir: {e}")))?;

    let start = Instant::now();
    let results: Vec<FileResult> = files
        .par_iter()
        .map(|p| process_one_bg(p, &output_path, &opts, &format))
        .collect();
    let duration_ms = start.elapsed().as_millis() as u64;

    let processed = results.iter().filter(|r| r.error.is_none()).count();
    let failed = results.len() - processed;

    let text = format!(
        "Processed {processed}/{total} files in {duration_ms}ms. Output: {out}",
        total = results.len(),
        out = output_path.display()
    );
    let structured = json!({
        "processed": processed,
        "failed": failed,
        "duration_ms": duration_ms,
        "output_dir": output_path.to_string_lossy(),
        "files": results.iter().map(|r| json!({
            "input": r.input.to_string_lossy(),
            "output": r.output.as_ref().map(|p| p.to_string_lossy().into_owned()),
            "status": if r.error.is_none() { "ok" } else { "failed" },
            "error": r.error,
        })).collect::<Vec<_>>(),
    });

    Ok(tool_text_result(text, structured))
}

struct FileResult {
    input: PathBuf,
    output: Option<PathBuf>,
    error: Option<String>,
}

fn process_one_bg(
    input_path: &Path,
    output_dir: &Path,
    opts: &bg_remove::Options,
    format: &str,
) -> FileResult {
    let mut result = FileResult {
        input: input_path.to_path_buf(),
        output: None,
        error: None,
    };

    let img = match image::open(input_path) {
        Ok(i) => i.into_rgba8(),
        Err(e) => {
            result.error = Some(format!("Reading {}: {e}", input_path.display()));
            return result;
        }
    };

    let processed = bg_remove::process(&img, opts);

    let stem = match input_path.file_stem() {
        Some(s) => s.to_string_lossy().into_owned(),
        None => {
            result.error = Some(format!("Invalid filename: {}", input_path.display()));
            return result;
        }
    };
    let output_path = output_dir.join(format!("{stem}.{format}"));

    let save_result = if format == "webp" {
        encode_webp(&processed, &output_path)
    } else {
        processed
            .save(&output_path)
            .map_err(|e| format!("Writing {}: {e}", output_path.display()))
    };

    match save_result {
        Ok(()) => result.output = Some(output_path),
        Err(e) => result.error = Some(e),
    }
    result
}

fn encode_webp(img: &image::RgbaImage, path: &Path) -> Result<(), String> {
    let encoder = webp::Encoder::from_rgba(img.as_raw(), img.width(), img.height());
    let data = encoder.encode(90.0);
    std::fs::write(path, &*data).map_err(|e| format!("Writing {}: {e}", path.display()))
}

// ---------- video_to_sprite ----------

fn video_to_sprite_handler(args: &Value) -> Result<Value, ToolError> {
    let input = require_string(args, "input")?;
    let output = require_string(args, "output")?;

    let fps = args
        .get("fps")
        .and_then(|v| v.as_u64())
        .unwrap_or(8)
        .clamp(1, 30) as u8;
    let size = args
        .get("size")
        .and_then(|v| v.as_u64())
        .unwrap_or(256)
        .clamp(64, 1024) as u32;
    let format = args
        .get("format")
        .and_then(|v| v.as_str())
        .unwrap_or("webp")
        .to_ascii_lowercase();
    let output_format = match format.as_str() {
        "png" => video_to_sprite::OutputFormat::Png,
        "webp" => video_to_sprite::OutputFormat::Webp,
        other => {
            return Err(ToolError::invalid_params(format!(
                "format: expected png|webp, got {other}"
            )))
        }
    };
    let chroma_key_enabled = args
        .get("chroma_key")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let opts = video_to_sprite::Options {
        fps,
        frame_size: size,
        output_format,
        webp_quality: 90,
        chroma_key: if chroma_key_enabled {
            Some(bg_remove::Options::default())
        } else {
            None
        },
    };

    let input_path = PathBuf::from(input);
    let output_path = PathBuf::from(output);

    let videos = batch::list_images(&input_path, false, &["mp4", "mov", "webm"])
        .map_err(|e| ToolError::internal(format!("Listing input: {e}")))?;

    if videos.is_empty() {
        return Ok(tool_text_result(
            format!("No videos found in {}", input_path.display()),
            json!({
                "processed": 0,
                "failed": 0,
                "duration_ms": 0,
                "output_dir": output_path.to_string_lossy(),
                "files": [],
            }),
        ));
    }

    std::fs::create_dir_all(&output_path)
        .map_err(|e| ToolError::internal(format!("Creating output dir: {e}")))?;

    video_to_sprite::check_ffmpeg()
        .map_err(|e| ToolError::internal(format!("ffmpeg check: {e}")))?;

    let start = Instant::now();
    let results: Vec<VideoResult> = videos
        .par_iter()
        .map(|v| process_one_video(v, &output_path, &opts))
        .collect();
    let duration_ms = start.elapsed().as_millis() as u64;

    let processed = results.iter().filter(|r| r.error.is_none()).count();
    let failed = results.len() - processed;

    let text = format!(
        "Processed {processed}/{total} videos in {duration_ms}ms. Output: {out}",
        total = results.len(),
        out = output_path.display()
    );
    let structured = json!({
        "processed": processed,
        "failed": failed,
        "duration_ms": duration_ms,
        "output_dir": output_path.to_string_lossy(),
        "files": results.iter().map(|r| json!({
            "input": r.input.to_string_lossy(),
            "sprite": r.sprite_path.as_ref().map(|p| p.to_string_lossy().into_owned()),
            "metadata": r.metadata_path.as_ref().map(|p| p.to_string_lossy().into_owned()),
            "frame_count": r.frame_count,
            "frame_size": r.frame_size,
            "status": if r.error.is_none() { "ok" } else { "failed" },
            "error": r.error,
        })).collect::<Vec<_>>(),
    });

    Ok(tool_text_result(text, structured))
}

struct VideoResult {
    input: PathBuf,
    sprite_path: Option<PathBuf>,
    metadata_path: Option<PathBuf>,
    frame_count: Option<u32>,
    frame_size: Option<u32>,
    error: Option<String>,
}

fn process_one_video(
    video: &Path,
    output_dir: &Path,
    opts: &video_to_sprite::Options,
) -> VideoResult {
    match video_to_sprite::process(video, output_dir, opts) {
        Ok(report) => VideoResult {
            input: video.to_path_buf(),
            sprite_path: Some(report.sprite_path),
            metadata_path: Some(report.metadata_path),
            frame_count: Some(report.frame_count),
            frame_size: Some(report.frame_size),
            error: None,
        },
        Err(e) => VideoResult {
            input: video.to_path_buf(),
            sprite_path: None,
            metadata_path: None,
            frame_count: None,
            frame_size: None,
            error: Some(format!("{e}")),
        },
    }
}

// ---------- list_presets ----------

fn list_presets_handler() -> Result<Value, ToolError> {
    // Preset system is Phase 6 — return empty list per spec.
    Ok(tool_text_result(
        "No presets configured. Preset system arrives in Phase 6.",
        json!({ "presets": [] }),
    ))
}

// ---------- helpers ----------

fn tool_text_result(text: impl Into<String>, structured: Value) -> Value {
    json!({
        "content": [{"type": "text", "text": text.into()}],
        "structuredContent": structured,
    })
}

fn require_string(args: &Value, key: &str) -> Result<String, ToolError> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(str::to_owned)
        .ok_or_else(|| ToolError::invalid_params(format!("Missing required arg: {key}")))
}

fn parse_hex_color(s: &str) -> Result<[u8; 3], String> {
    let trimmed = s.trim_start_matches('#');
    if trimmed.len() != 6 {
        return Err(format!("hex color must be 6 chars, got {}", s));
    }
    let r =
        u8::from_str_radix(&trimmed[0..2], 16).map_err(|_| format!("invalid hex byte in {s}"))?;
    let g =
        u8::from_str_radix(&trimmed[2..4], 16).map_err(|_| format!("invalid hex byte in {s}"))?;
    let b =
        u8::from_str_radix(&trimmed[4..6], 16).map_err(|_| format!("invalid hex byte in {s}"))?;
    Ok([r, g, b])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_tools_returns_four_tools() {
        let tools = list_tools();
        assert_eq!(tools.len(), 4);
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"bg_remove"));
        assert!(names.contains(&"video_to_sprite"));
        assert!(names.contains(&"vectorize"));
        assert!(names.contains(&"list_presets"));
    }

    #[test]
    fn list_tools_have_required_fields() {
        for tool in list_tools() {
            assert!(tool["name"].is_string());
            assert!(tool["description"].is_string());
            assert!(tool["inputSchema"].is_object());
            assert_eq!(tool["inputSchema"]["type"], "object");
        }
    }

    #[test]
    fn call_unknown_tool_returns_method_not_found() {
        let params = json!({ "name": "nope", "arguments": {} });
        let err = call(Some(&params)).unwrap_err();
        assert_eq!(err.code, ERR_METHOD_NOT_FOUND);
    }

    #[test]
    fn call_missing_name_returns_invalid_params() {
        let params = json!({ "arguments": {} });
        let err = call(Some(&params)).unwrap_err();
        assert_eq!(err.code, ERR_INVALID_PARAMS);
    }

    #[test]
    fn call_missing_params_returns_invalid_params() {
        let err = call(None).unwrap_err();
        assert_eq!(err.code, ERR_INVALID_PARAMS);
    }

    #[test]
    fn bg_remove_missing_input_returns_invalid_params() {
        let params = json!({
            "name": "bg_remove",
            "arguments": { "output": "/tmp/out" }
        });
        let err = call(Some(&params)).unwrap_err();
        assert_eq!(err.code, ERR_INVALID_PARAMS);
        assert!(err.message.contains("input"), "msg was: {}", err.message);
    }

    #[test]
    fn bg_remove_missing_output_returns_invalid_params() {
        let params = json!({
            "name": "bg_remove",
            "arguments": { "input": "/tmp/in" }
        });
        let err = call(Some(&params)).unwrap_err();
        assert_eq!(err.code, ERR_INVALID_PARAMS);
        assert!(err.message.contains("output"));
    }

    #[test]
    fn bg_remove_invalid_target_color_returns_invalid_params() {
        let params = json!({
            "name": "bg_remove",
            "arguments": {
                "input": "/tmp/in",
                "output": "/tmp/out",
                "target_color": "not-hex"
            }
        });
        let err = call(Some(&params)).unwrap_err();
        assert_eq!(err.code, ERR_INVALID_PARAMS);
    }

    #[test]
    fn video_to_sprite_missing_input_returns_invalid_params() {
        let params = json!({
            "name": "video_to_sprite",
            "arguments": { "output": "/tmp/out" }
        });
        let err = call(Some(&params)).unwrap_err();
        assert_eq!(err.code, ERR_INVALID_PARAMS);
    }

    #[test]
    fn vectorize_returns_not_implemented_stub() {
        let params = json!({
            "name": "vectorize",
            "arguments": { "input": "/tmp/in.png", "output": "/tmp/out.svg" }
        });
        let err = call(Some(&params)).unwrap_err();
        assert_eq!(err.code, ERR_INTERNAL);
        assert!(
            err.message.to_lowercase().contains("phase 3")
                || err.message.to_lowercase().contains("not implemented")
                || err.message.to_lowercase().contains("vectorize"),
            "msg was: {}",
            err.message
        );
    }

    #[test]
    fn list_presets_returns_empty_list() {
        let params = json!({ "name": "list_presets", "arguments": {} });
        let result = call(Some(&params)).unwrap();
        assert!(result["structuredContent"]["presets"].is_array());
        assert_eq!(
            result["structuredContent"]["presets"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
    }

    #[test]
    fn parse_hex_color_with_hash() {
        assert_eq!(parse_hex_color("#00FF00").unwrap(), [0, 255, 0]);
    }

    #[test]
    fn parse_hex_color_without_hash() {
        assert_eq!(parse_hex_color("ff0080").unwrap(), [255, 0, 128]);
    }

    #[test]
    fn parse_hex_color_invalid_length() {
        assert!(parse_hex_color("#FFF").is_err());
    }

    #[test]
    fn parse_hex_color_invalid_chars() {
        assert!(parse_hex_color("#XXYYZZ").is_err());
    }
}
