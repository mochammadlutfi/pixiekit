//! Tool definitions and `tools/call` dispatcher.
//!
//! Each tool corresponds to a function in `pixiekit_core::*`. Schemas mirror
//! PRD §7.3.2. Handlers convert MCP arguments → core options, execute, and
//! shape the response per PRD §7.3.3.

use std::path::{Path, PathBuf};
use std::time::Instant;

use rayon::prelude::*;
use serde_json::{json, Value};

use pixiekit_core::{
    atlas_pack, audio, batch, bg_remove, optimize, preset, scale, svg_optimize, trim_pad,
    vectorize, video_to_sprite, nine_slice, anim_preview,
};

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
            "name": "atlas_pack",
            "description": "Pack a folder of PNG sprites into a texture atlas with Flame-compatible JSON metadata. Reduces draw calls in game runtime.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "input": {"type": "string", "description": "Folder of PNG sprites"},
                    "output": {"type": "string", "description": "Output folder for atlas + JSON"},
                    "name": {"type": "string", "default": "atlas"},
                    "max_size": {"type": "integer", "minimum": 256, "maximum": 8192, "default": 2048},
                    "padding": {"type": "integer", "minimum": 0, "maximum": 16, "default": 2},
                    "extrude": {"type": "integer", "minimum": 0, "maximum": 4, "default": 1},
                    "power_of_two": {"type": "boolean", "default": true},
                    "trim": {"type": "boolean", "default": true},
                    "format": {"type": "string", "enum": ["png", "webp"], "default": "png"}
                },
                "required": ["input", "output"]
            }
        }),
        json!({
            "name": "optimize_image",
            "description": "Optimize PNG/JPG/WebP file size (oxipng + re-encode). Batch process folder.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "input": {"type": "string", "description": "Path to image file or folder"},
                    "output": {"type": "string", "description": "Output folder path"},
                    "target_format": {"type": "string", "enum": ["png", "webp", "keep"], "default": "webp"},
                    "quality": {"type": "integer", "minimum": 0, "maximum": 100, "default": 90},
                    "lossless": {"type": "boolean", "default": false},
                    "strip_metadata": {"type": "boolean", "default": true},
                    "optimization_level": {"type": "integer", "minimum": 0, "maximum": 6, "default": 3}
                },
                "required": ["input", "output"]
            }
        }),
        json!({
            "name": "scale_image",
            "description": "Resample image to multiple density variants (Flutter @1x/@2x/@3x, iOS @suffix, or nested folders).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "input": {"type": "string"},
                    "output": {"type": "string"},
                    "base_scale": {"type": "number", "minimum": 0.1, "default": 4.0},
                    "target_scales": {
                        "type": "array",
                        "items": {"type": "number", "minimum": 0.1},
                        "default": [1.0, 1.5, 2.0, 3.0]
                    },
                    "naming": {"type": "string", "enum": ["flutter", "suffix", "nested"], "default": "flutter"},
                    "filter": {"type": "string", "enum": ["lanczos", "bilinear", "nearest"], "default": "lanczos"}
                },
                "required": ["input", "output"]
            }
        }),
        json!({
            "name": "audio_process",
            "description": "Normalize loudness (LUFS), trim silence, and convert audio to OGG/OPUS/MP3/WAV. Batch process folder.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "input": {"type": "string", "description": "Path to audio file or folder"},
                    "output": {"type": "string", "description": "Output folder path"},
                    "target_format": {"type": "string", "enum": ["ogg", "opus", "mp3", "wav"], "default": "ogg"},
                    "target_lufs": {"type": "number", "default": -16.0},
                    "normalize": {"type": "boolean", "default": true},
                    "trim_silence": {"type": "boolean", "default": true},
                    "silence_threshold_db": {"type": "number", "default": -50.0},
                    "sample_rate": {"type": "integer", "minimum": 8000, "maximum": 192000, "default": 44100},
                    "channels": {"type": "string", "enum": ["mono", "stereo", "keep"], "default": "keep"},
                    "bitrate_kbps": {"type": "integer", "minimum": 32, "maximum": 320, "default": 128}
                },
                "required": ["input", "output"]
            }
        }),
        json!({
            "name": "trim_pad",
            "description": "Auto-crop transparent (or solid-color) borders, optionally pad uniform px and force square output.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "input": {"type": "string", "description": "Path to image file or folder"},
                    "output": {"type": "string", "description": "Output folder path"},
                    "alpha_threshold": {"type": "integer", "minimum": 0, "maximum": 255, "default": 1},
                    "padding": {"type": "integer", "minimum": 0, "maximum": 4096, "default": 0},
                    "keep_square": {"type": "boolean", "default": false},
                    "bg_color": {"type": "string", "pattern": "^#[0-9a-fA-F]{6}$", "description": "Hex e.g. #00FF00 to trim a solid colour instead of alpha"},
                    "bg_tolerance": {"type": "number", "minimum": 0, "maximum": 1, "default": 0.05}
                },
                "required": ["input", "output"]
            }
        }),
        json!({
            "name": "svg_optimize",
            "description": "Minify SVG: parse via usvg, round path coords, strip metadata, drop hidden elements.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "input": {"type": "string", "description": "Path to SVG file or folder"},
                    "output": {"type": "string", "description": "Output folder path"},
                    "precision": {"type": "integer", "minimum": 0, "maximum": 8, "default": 3},
                    "remove_metadata": {"type": "boolean", "default": true},
                    "remove_hidden": {"type": "boolean", "default": true},
                    "merge_paths": {"type": "boolean", "default": true},
                    "pretty": {"type": "boolean", "default": false}
                },
                "required": ["input", "output"]
            }
        }),
        json!({
            "name": "list_presets",
            "description": "List saved processing presets (names only). Use `get_preset` to fetch options.",
            "inputSchema": {"type": "object", "properties": {}}
        }),
        json!({
            "name": "get_preset",
            "description": "Fetch a saved preset by name. Returns the wrapped preset (tool, version, options).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": {"type": "string", "description": "Preset name (as returned by list_presets)"}
                },
                "required": ["name"]
            }
        }),
        json!({
            "name": "nine_slice",
            "description": "Slice an image into 9 parts for UI scaling or generate Flame-compatible JSON metadata.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "input": {"type": "string", "description": "Path to image file or folder"},
                    "output": {"type": "string", "description": "Output folder path"},
                    "mode": {"type": "string", "enum": ["split", "metadata"], "default": "metadata"},
                    "left": {"type": "integer", "minimum": 0, "default": 0},
                    "right": {"type": "integer", "minimum": 0, "default": 0},
                    "top": {"type": "integer", "minimum": 0, "default": 0},
                    "bottom": {"type": "integer", "minimum": 0, "default": 0}
                },
                "required": ["input", "output", "left", "right", "top", "bottom"]
            }
        }),
        json!({
            "name": "anim_preview",
            "description": "Generate high-quality GIF/MP4/WebM animation previews from sprite sheets or frame folders.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "input": {"type": "string", "description": "Path to sprite sheet image or folder of frames"},
                    "output": {"type": "string", "description": "Output folder path"},
                    "fps": {"type": "integer", "minimum": 1, "maximum": 60, "default": 12},
                    "format": {"type": "string", "enum": ["gif", "mp4", "webm"], "default": "gif"},
                    "scale": {"type": "integer", "minimum": 1, "maximum": 8, "default": 1},
                    "sheet_cols": {"type": "integer", "minimum": 1, "description": "Required if input is a sprite sheet"},
                    "sheet_rows": {"type": "integer", "minimum": 1, "description": "Required if input is a sprite sheet"}
                },
                "required": ["input", "output"]
            }
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
        "vectorize" => vectorize_handler(&args),
        "atlas_pack" => atlas_pack_handler(&args),
        "optimize_image" => optimize_image_handler(&args),
        "scale_image" => scale_image_handler(&args),
        "audio_process" => audio_process_handler(&args),
        "trim_pad" => trim_pad_handler(&args),
        "svg_optimize" => svg_optimize_handler(&args),
        "list_presets" => list_presets_handler(),
        "get_preset" => get_preset_handler(&args),
        "nine_slice" => nine_slice_handler(&args),
        "anim_preview" => anim_preview_handler(&args),
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

// ---------- vectorize ----------

fn vectorize_handler(args: &Value) -> Result<Value, ToolError> {
    let input = require_string(args, "input")?;
    let output = require_string(args, "output")?;

    let mode_str = args
        .get("mode")
        .and_then(|v| v.as_str())
        .unwrap_or("color")
        .to_ascii_lowercase();
    let mode = match mode_str.as_str() {
        "color" => vectorize::Mode::Color,
        "binary" => vectorize::Mode::Binary,
        other => {
            return Err(ToolError::invalid_params(format!(
                "mode: expected color|binary, got {other}"
            )))
        }
    };

    let smooth = args
        .get("smooth")
        .and_then(|v| v.as_u64())
        .unwrap_or(4)
        .min(10) as u8;
    let (corner_threshold, length_threshold, splice_threshold) =
        vectorize::smooth_to_params(smooth);

    let opts = vectorize::Options {
        mode,
        corner_threshold,
        length_threshold,
        splice_threshold,
        ..Default::default()
    };

    let input_path = PathBuf::from(&input);
    let output_path = PathBuf::from(&output);

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
        .map(|input_path| {
            let stem = input_path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "out".into());
            let svg_path = output_path.join(format!("{stem}.svg"));
            match vectorize::process(input_path, &svg_path, &opts) {
                Ok(()) => FileResult {
                    input: input_path.clone(),
                    output: Some(svg_path),
                    error: None,
                },
                Err(e) => FileResult {
                    input: input_path.clone(),
                    output: None,
                    error: Some(format!("{e}")),
                },
            }
        })
        .collect();

    let processed = results.iter().filter(|r| r.error.is_none()).count();
    let failed = results.len() - processed;
    let duration_ms = start.elapsed().as_millis();

    Ok(tool_text_result(
        format!(
            "Vectorized {processed}/{} images in {duration_ms}ms (output: {})",
            results.len(),
            output_path.display()
        ),
        json!({
            "processed": processed,
            "failed": failed,
            "duration_ms": duration_ms,
            "output_dir": output_path.to_string_lossy(),
            "files": results.iter().map(|r| json!({
                "input": r.input,
                "output": r.output,
                "status": if r.error.is_none() { "ok" } else { "failed" },
                "error": r.error,
            })).collect::<Vec<_>>(),
        }),
    ))
}

// ---------- atlas_pack ----------

fn atlas_pack_handler(args: &Value) -> Result<Value, ToolError> {
    let input = require_string(args, "input")?;
    let output = require_string(args, "output")?;

    let name = args
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("atlas")
        .to_string();
    let max_size = args
        .get("max_size")
        .and_then(|v| v.as_u64())
        .unwrap_or(2048)
        .clamp(256, 8192) as u16;
    let padding = args
        .get("padding")
        .and_then(|v| v.as_u64())
        .unwrap_or(2)
        .min(16) as u8;
    let extrude = args
        .get("extrude")
        .and_then(|v| v.as_u64())
        .unwrap_or(1)
        .min(4) as u8;
    let power_of_two = args
        .get("power_of_two")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let trim = args.get("trim").and_then(|v| v.as_bool()).unwrap_or(true);
    let format_str = args
        .get("format")
        .and_then(|v| v.as_str())
        .unwrap_or("png")
        .to_ascii_lowercase();
    let format = match format_str.as_str() {
        "png" => atlas_pack::OutputFormat::Png,
        "webp" => atlas_pack::OutputFormat::Webp,
        other => {
            return Err(ToolError::invalid_params(format!(
                "format: expected png|webp, got {other}"
            )))
        }
    };
    let webp_quality = args
        .get("webp_quality")
        .and_then(|v| v.as_u64())
        .unwrap_or(90)
        .min(100) as u8;

    let opts = atlas_pack::Options {
        name,
        max_size,
        padding,
        extrude,
        power_of_two,
        trim,
        format,
        webp_quality,
    };

    let input_path = PathBuf::from(input);
    let output_path = PathBuf::from(output);

    let sprites = batch::list_images(&input_path, false, &["png"])
        .map_err(|e| ToolError::internal(format!("Listing sprites: {e}")))?;

    if sprites.is_empty() {
        return Ok(tool_text_result(
            format!("No PNG sprites found in {}", input_path.display()),
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
    let report = atlas_pack::process(&sprites, &output_path, &opts)
        .map_err(|e| ToolError::internal(format!("Atlas pack: {e}")))?;
    let duration_ms = start.elapsed().as_millis() as u64;

    let processed = report.packed as usize;
    let failed = (report.total - report.packed) as usize;
    let text = format!(
        "Packed {}/{} sprites into {}×{} atlas ({:.0}% efficiency) in {duration_ms}ms. Output: {}",
        report.packed,
        report.total,
        report.atlas_size.0,
        report.atlas_size.1,
        report.efficiency * 100.0,
        output_path.display()
    );
    let structured = json!({
        "processed": processed,
        "failed": failed,
        "duration_ms": duration_ms,
        "output_dir": output_path.to_string_lossy(),
        "atlas_path": report.atlas_path.to_string_lossy(),
        "metadata_path": report.metadata_path.to_string_lossy(),
        "atlas_size": { "w": report.atlas_size.0, "h": report.atlas_size.1 },
        "efficiency": report.efficiency,
        "files": [{
            "input": input_path.to_string_lossy(),
            "output": report.atlas_path.to_string_lossy(),
            "status": "ok",
        }],
    });
    Ok(tool_text_result(text, structured))
}

// ---------- optimize_image ----------

fn optimize_image_handler(args: &Value) -> Result<Value, ToolError> {
    let input = require_string(args, "input")?;
    let output = require_string(args, "output")?;

    let target_format = match args
        .get("target_format")
        .and_then(|v| v.as_str())
        .unwrap_or("webp")
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => optimize::TargetFormat::Png,
        "webp" => optimize::TargetFormat::Webp,
        "keep" => optimize::TargetFormat::Keep,
        other => {
            return Err(ToolError::invalid_params(format!(
                "target_format: expected png|webp|keep, got {other}"
            )))
        }
    };

    let quality = args
        .get("quality")
        .and_then(|v| v.as_u64())
        .unwrap_or(90)
        .min(100) as u8;
    let lossless = args
        .get("lossless")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let strip_metadata = args
        .get("strip_metadata")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let optimization_level = args
        .get("optimization_level")
        .and_then(|v| v.as_u64())
        .unwrap_or(3)
        .min(6) as u8;

    let opts = optimize::Options {
        target_format,
        quality,
        lossless,
        strip_metadata,
        optimization_level,
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
    let results: Vec<OptimizeResult> = files
        .par_iter()
        .map(|f| process_one_optimize(f, &output_path, &opts))
        .collect();
    let duration_ms = start.elapsed().as_millis();

    let processed = results.iter().filter(|r| r.error.is_none()).count();
    let failed = results.len() - processed;

    let text = format!(
        "Optimized {processed}/{} images in {duration_ms}ms (output: {})",
        results.len(),
        output_path.display()
    );
    let structured = json!({
        "processed": processed,
        "failed": failed,
        "duration_ms": duration_ms,
        "output_dir": output_path.to_string_lossy(),
        "files": results.iter().map(|r| json!({
            "input": r.input.to_string_lossy(),
            "output": r.output.as_ref().map(|p| p.to_string_lossy().into_owned()),
            "input_size": r.input_size,
            "output_size": r.output_size,
            "ratio": r.ratio,
            "status": if r.error.is_none() { "ok" } else { "failed" },
            "error": r.error,
        })).collect::<Vec<_>>(),
    });

    Ok(tool_text_result(text, structured))
}

struct OptimizeResult {
    input: PathBuf,
    output: Option<PathBuf>,
    input_size: Option<u64>,
    output_size: Option<u64>,
    ratio: Option<f32>,
    error: Option<String>,
}

fn process_one_optimize(
    input_path: &Path,
    output_dir: &Path,
    opts: &optimize::Options,
) -> OptimizeResult {
    let stem = match input_path.file_stem() {
        Some(s) => s.to_string_lossy().into_owned(),
        None => {
            return OptimizeResult {
                input: input_path.to_path_buf(),
                output: None,
                input_size: None,
                output_size: None,
                ratio: None,
                error: Some(format!("Invalid filename: {}", input_path.display())),
            }
        }
    };
    let target_stub = output_dir.join(stem);
    match optimize::process(input_path, &target_stub, opts) {
        Ok(report) => OptimizeResult {
            input: input_path.to_path_buf(),
            output: Some(report.output_path),
            input_size: Some(report.input_size),
            output_size: Some(report.output_size),
            ratio: Some(report.ratio),
            error: None,
        },
        Err(e) => OptimizeResult {
            input: input_path.to_path_buf(),
            output: None,
            input_size: None,
            output_size: None,
            ratio: None,
            error: Some(format!("{e}")),
        },
    }
}

// ---------- scale_image ----------

fn scale_image_handler(args: &Value) -> Result<Value, ToolError> {
    let input = require_string(args, "input")?;
    let output = require_string(args, "output")?;

    let base_scale = args
        .get("base_scale")
        .and_then(|v| v.as_f64())
        .unwrap_or(4.0) as f32;

    let target_scales: Vec<f32> = match args.get("target_scales") {
        Some(Value::Array(arr)) => arr
            .iter()
            .filter_map(|v| v.as_f64().map(|f| f as f32))
            .collect(),
        Some(_) => {
            return Err(ToolError::invalid_params(
                "target_scales: expected array of numbers",
            ))
        }
        None => vec![1.0, 1.5, 2.0, 3.0],
    };
    if target_scales.is_empty() {
        return Err(ToolError::invalid_params(
            "target_scales must list at least one density",
        ));
    }

    let naming = match args
        .get("naming")
        .and_then(|v| v.as_str())
        .unwrap_or("flutter")
        .to_ascii_lowercase()
        .as_str()
    {
        "flutter" => scale::NamingMode::Flutter,
        "suffix" => scale::NamingMode::Suffix,
        "nested" => scale::NamingMode::Nested,
        other => {
            return Err(ToolError::invalid_params(format!(
                "naming: expected flutter|suffix|nested, got {other}"
            )))
        }
    };

    let filter = match args
        .get("filter")
        .and_then(|v| v.as_str())
        .unwrap_or("lanczos")
        .to_ascii_lowercase()
        .as_str()
    {
        "lanczos" => scale::Filter::Lanczos,
        "bilinear" => scale::Filter::Bilinear,
        "nearest" => scale::Filter::Nearest,
        other => {
            return Err(ToolError::invalid_params(format!(
                "filter: expected lanczos|bilinear|nearest, got {other}"
            )))
        }
    };

    let opts = scale::Options {
        base_scale,
        target_scales,
        naming,
        filter,
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
    let results: Vec<ScaleResult> = files
        .par_iter()
        .map(|f| match scale::process(f, &output_path, &opts) {
            Ok(report) => ScaleResult {
                input: f.clone(),
                variants: report.variants,
                error: None,
            },
            Err(e) => ScaleResult {
                input: f.clone(),
                variants: Vec::new(),
                error: Some(format!("{e}")),
            },
        })
        .collect();
    let duration_ms = start.elapsed().as_millis();

    let processed = results.iter().filter(|r| r.error.is_none()).count();
    let failed = results.len() - processed;
    let total_variants: usize = results.iter().map(|r| r.variants.len()).sum();

    let text = format!(
        "Scaled {processed}/{} images into {total_variants} variants in {duration_ms}ms",
        results.len()
    );
    let structured = json!({
        "processed": processed,
        "failed": failed,
        "duration_ms": duration_ms,
        "output_dir": output_path.to_string_lossy(),
        "files": results.iter().map(|r| json!({
            "input": r.input.to_string_lossy(),
            "variants": r.variants.iter().map(|p| p.to_string_lossy().into_owned()).collect::<Vec<_>>(),
            "status": if r.error.is_none() { "ok" } else { "failed" },
            "error": r.error,
        })).collect::<Vec<_>>(),
    });

    Ok(tool_text_result(text, structured))
}

struct ScaleResult {
    input: PathBuf,
    variants: Vec<PathBuf>,
    error: Option<String>,
}

// ---------- audio_process ----------

fn audio_process_handler(args: &Value) -> Result<Value, ToolError> {
    let input = require_string(args, "input")?;
    let output = require_string(args, "output")?;

    let target_format_str = args
        .get("target_format")
        .and_then(|v| v.as_str())
        .unwrap_or("ogg")
        .to_ascii_lowercase();
    let target_format = match target_format_str.as_str() {
        "ogg" => audio::TargetFormat::Ogg,
        "opus" => audio::TargetFormat::Opus,
        "mp3" => audio::TargetFormat::Mp3,
        "wav" => audio::TargetFormat::Wav,
        other => {
            return Err(ToolError::invalid_params(format!(
                "target_format: expected ogg|opus|mp3|wav, got {other}"
            )))
        }
    };

    let target_lufs = args
        .get("target_lufs")
        .and_then(|v| v.as_f64())
        .unwrap_or(-16.0) as f32;
    let normalize = args
        .get("normalize")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let trim_silence = args
        .get("trim_silence")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let silence_threshold_db = args
        .get("silence_threshold_db")
        .and_then(|v| v.as_f64())
        .unwrap_or(-50.0) as f32;
    let sample_rate = args
        .get("sample_rate")
        .and_then(|v| v.as_u64())
        .unwrap_or(44_100)
        .clamp(8_000, 192_000) as u32;
    let channels_str = args
        .get("channels")
        .and_then(|v| v.as_str())
        .unwrap_or("keep")
        .to_ascii_lowercase();
    let channels = match channels_str.as_str() {
        "mono" => audio::Channels::Mono,
        "stereo" => audio::Channels::Stereo,
        "keep" => audio::Channels::Keep,
        other => {
            return Err(ToolError::invalid_params(format!(
                "channels: expected mono|stereo|keep, got {other}"
            )))
        }
    };
    let bitrate_kbps = args
        .get("bitrate_kbps")
        .and_then(|v| v.as_u64())
        .unwrap_or(128)
        .clamp(32, 320) as u16;

    let opts = audio::Options {
        target_format,
        target_lufs,
        normalize,
        trim_silence,
        silence_threshold_db,
        sample_rate,
        channels,
        bitrate_kbps,
    };

    let input_path = PathBuf::from(input);
    let output_path = PathBuf::from(output);

    let files = batch::list_images(
        &input_path,
        false,
        &["wav", "mp3", "ogg", "m4a", "flac", "opus"],
    )
    .map_err(|e| ToolError::internal(format!("Listing input: {e}")))?;

    if files.is_empty() {
        return Ok(tool_text_result(
            format!("No audio files found in {}", input_path.display()),
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

    audio::check_ffmpeg().map_err(|e| ToolError::internal(format!("ffmpeg check: {e}")))?;

    let start = Instant::now();
    let results: Vec<AudioCallResult> = files
        .par_iter()
        .map(|p| process_one_audio(p, &output_path, &opts))
        .collect();
    let duration_ms = start.elapsed().as_millis() as u64;

    let processed = results.iter().filter(|r| r.error.is_none()).count();
    let failed = results.len() - processed;

    let text = format!(
        "Processed {processed}/{total} audio file(s) in {duration_ms}ms. Output: {out}",
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
            "duration_ms_in": r.duration_ms_in,
            "duration_ms_out": r.duration_ms_out,
            "integrated_lufs": r.integrated_lufs,
            "status": if r.error.is_none() { "ok" } else { "failed" },
            "error": r.error,
        })).collect::<Vec<_>>(),
    });

    Ok(tool_text_result(text, structured))
}

struct AudioCallResult {
    input: PathBuf,
    output: Option<PathBuf>,
    duration_ms_in: Option<u32>,
    duration_ms_out: Option<u32>,
    integrated_lufs: Option<f32>,
    error: Option<String>,
}

fn process_one_audio(
    input_path: &Path,
    output_dir: &Path,
    opts: &audio::Options,
) -> AudioCallResult {
    let stem = match input_path.file_stem() {
        Some(s) => s.to_string_lossy().into_owned(),
        None => {
            return AudioCallResult {
                input: input_path.to_path_buf(),
                output: None,
                duration_ms_in: None,
                duration_ms_out: None,
                integrated_lufs: None,
                error: Some(format!("Invalid filename: {}", input_path.display())),
            };
        }
    };
    let out_path = output_dir.join(format!("{stem}.{}", opts.target_format.extension()));
    match audio::process(input_path, &out_path, opts) {
        Ok(report) => AudioCallResult {
            input: input_path.to_path_buf(),
            output: Some(out_path),
            duration_ms_in: Some(report.duration_ms_in),
            duration_ms_out: Some(report.duration_ms_out),
            integrated_lufs: report.integrated_lufs,
            error: None,
        },
        Err(e) => AudioCallResult {
            input: input_path.to_path_buf(),
            output: None,
            duration_ms_in: None,
            duration_ms_out: None,
            integrated_lufs: None,
            error: Some(format!("{e}")),
        },
    }
}

// ---------- trim_pad ----------

fn trim_pad_handler(args: &Value) -> Result<Value, ToolError> {
    let input = require_string(args, "input")?;
    let output = require_string(args, "output")?;

    let alpha_threshold = args
        .get("alpha_threshold")
        .and_then(|v| v.as_u64())
        .unwrap_or(1)
        .min(255) as u8;
    let padding = args
        .get("padding")
        .and_then(|v| v.as_u64())
        .unwrap_or(0)
        .min(u16::MAX as u64) as u16;
    let keep_square = args
        .get("keep_square")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let bg_color = match args.get("bg_color").and_then(|v| v.as_str()) {
        Some(s) => Some(
            parse_hex_color(s).map_err(|e| ToolError::invalid_params(format!("bg_color: {e}")))?,
        ),
        None => None,
    };
    let bg_tolerance = args
        .get("bg_tolerance")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.05) as f32;

    let opts = trim_pad::Options {
        alpha_threshold,
        padding,
        keep_square,
        bg_color,
        bg_tolerance,
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
    let results: Vec<TrimResult> = files
        .par_iter()
        .map(|input_file| {
            let file_name = match input_file.file_name() {
                Some(n) => n,
                None => {
                    return TrimResult {
                        input: input_file.clone(),
                        output: None,
                        output_size: None,
                        bbox: None,
                        error: Some(format!("Invalid filename: {}", input_file.display())),
                    }
                }
            };
            let out_path = output_path.join(file_name);
            match trim_pad::process(input_file, &out_path, &opts) {
                Ok(report) => TrimResult {
                    input: input_file.clone(),
                    output: Some(out_path),
                    output_size: Some(report.output_size),
                    bbox: Some(report.bbox),
                    error: None,
                },
                Err(e) => TrimResult {
                    input: input_file.clone(),
                    output: None,
                    output_size: None,
                    bbox: None,
                    error: Some(format!("{e}")),
                },
            }
        })
        .collect();
    let duration_ms = start.elapsed().as_millis() as u64;

    let processed = results.iter().filter(|r| r.error.is_none()).count();
    let failed = results.len() - processed;

    Ok(tool_text_result(
        format!(
            "Trimmed {processed}/{} images in {duration_ms}ms (output: {})",
            results.len(),
            output_path.display()
        ),
        json!({
            "processed": processed,
            "failed": failed,
            "duration_ms": duration_ms,
            "output_dir": output_path.to_string_lossy(),
            "files": results.iter().map(|r| json!({
                "input": r.input.to_string_lossy(),
                "output": r.output.as_ref().map(|p| p.to_string_lossy().into_owned()),
                "output_size": r.output_size,
                "bbox": r.bbox,
                "status": if r.error.is_none() { "ok" } else { "failed" },
                "error": r.error,
            })).collect::<Vec<_>>(),
        }),
    ))
}

struct TrimResult {
    input: PathBuf,
    output: Option<PathBuf>,
    output_size: Option<(u32, u32)>,
    bbox: Option<(u32, u32, u32, u32)>,
    error: Option<String>,
}

// ---------- svg_optimize ----------

fn svg_optimize_handler(args: &Value) -> Result<Value, ToolError> {
    let input = require_string(args, "input")?;
    let output = require_string(args, "output")?;

    let precision = args
        .get("precision")
        .and_then(|v| v.as_u64())
        .unwrap_or(3)
        .min(8) as u8;
    let remove_metadata = args
        .get("remove_metadata")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let remove_hidden = args
        .get("remove_hidden")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let merge_paths = args
        .get("merge_paths")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let pretty = args
        .get("pretty")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let opts = svg_optimize::Options {
        precision,
        remove_metadata,
        remove_hidden,
        merge_paths,
        pretty,
    };

    let input_path = PathBuf::from(input);
    let output_path = PathBuf::from(output);

    let files = batch::list_images(&input_path, false, &["svg"])
        .map_err(|e| ToolError::internal(format!("Listing input: {e}")))?;

    if files.is_empty() {
        return Ok(tool_text_result(
            format!("No SVG files found in {}", input_path.display()),
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
    let results: Vec<SvgResult> = files
        .par_iter()
        .map(|input_file| {
            let file_name = match input_file.file_name() {
                Some(n) => n,
                None => {
                    return SvgResult {
                        input: input_file.clone(),
                        output: None,
                        input_size: None,
                        output_size: None,
                        ratio: None,
                        error: Some(format!("Invalid filename: {}", input_file.display())),
                    }
                }
            };
            let out_path = output_path.join(file_name);
            match svg_optimize::process(input_file, &out_path, &opts) {
                Ok(report) => SvgResult {
                    input: input_file.clone(),
                    output: Some(out_path),
                    input_size: Some(report.input_size),
                    output_size: Some(report.output_size),
                    ratio: Some(report.ratio),
                    error: None,
                },
                Err(e) => SvgResult {
                    input: input_file.clone(),
                    output: None,
                    input_size: None,
                    output_size: None,
                    ratio: None,
                    error: Some(format!("{e}")),
                },
            }
        })
        .collect();
    let duration_ms = start.elapsed().as_millis() as u64;

    let processed = results.iter().filter(|r| r.error.is_none()).count();
    let failed = results.len() - processed;

    Ok(tool_text_result(
        format!(
            "Optimized {processed}/{} SVGs in {duration_ms}ms (output: {})",
            results.len(),
            output_path.display()
        ),
        json!({
            "processed": processed,
            "failed": failed,
            "duration_ms": duration_ms,
            "output_dir": output_path.to_string_lossy(),
            "files": results.iter().map(|r| json!({
                "input": r.input.to_string_lossy(),
                "output": r.output.as_ref().map(|p| p.to_string_lossy().into_owned()),
                "input_size": r.input_size,
                "output_size": r.output_size,
                "ratio": r.ratio,
                "status": if r.error.is_none() { "ok" } else { "failed" },
                "error": r.error,
            })).collect::<Vec<_>>(),
        }),
    ))
}

struct SvgResult {
    input: PathBuf,
    output: Option<PathBuf>,
    input_size: Option<u64>,
    output_size: Option<u64>,
    ratio: Option<f32>,
    error: Option<String>,
}

// ---------- list_presets / get_preset ----------

fn list_presets_handler() -> Result<Value, ToolError> {
    let names = preset::list().map_err(|e| ToolError::internal(format!("Listing presets: {e}")))?;
    let dir = preset::presets_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    let text = if names.is_empty() {
        format!("No presets saved in {dir}")
    } else {
        format!(
            "Found {} preset(s) in {dir}:\n  {}",
            names.len(),
            names.join("\n  ")
        )
    };
    Ok(tool_text_result(
        text,
        json!({
            "presets": names,
            "presets_dir": dir,
        }),
    ))
}

fn get_preset_handler(args: &Value) -> Result<Value, ToolError> {
    let name = require_string(args, "name")?;
    let preset = preset::load(&name).map_err(|e| match e {
        pixiekit_core::Error::PresetNotFound { .. } => ToolError {
            code: ERR_INVALID_PARAMS,
            message: format!("{e}"),
        },
        pixiekit_core::Error::InvalidPresetName(_) => ToolError {
            code: ERR_INVALID_PARAMS,
            message: format!("{e}"),
        },
        other => ToolError::internal(format!("Loading preset: {other}")),
    })?;
    let text = format!(
        "Preset '{}' (tool: {}, version: {})",
        preset.name, preset.tool, preset.version
    );
    let structured = json!({
        "name": preset.name,
        "tool": preset.tool,
        "version": preset.version,
        "options": preset.options,
    });
    Ok(tool_text_result(text, structured))
}

// ---------- nine_slice ----------

fn nine_slice_handler(args: &Value) -> Result<Value, ToolError> {
    let input = require_string(args, "input")?;
    let output = require_string(args, "output")?;

    let mode_str = args
        .get("mode")
        .and_then(|v| v.as_str())
        .unwrap_or("metadata")
        .to_ascii_lowercase();
    let output_mode = match mode_str.as_str() {
        "split" => nine_slice::OutputMode::Split,
        "metadata" => nine_slice::OutputMode::Metadata,
        other => {
            return Err(ToolError::invalid_params(format!(
                "mode: expected split|metadata, got {other}"
            )))
        }
    };

    let left = args.get("left").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let right = args.get("right").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let top = args.get("top").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let bottom = args.get("bottom").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

    let opts = nine_slice::Options {
        left,
        right,
        top,
        bottom,
        output_mode,
    };

    let input_path = PathBuf::from(&input);
    let output_path = PathBuf::from(&output);

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
    let results: Vec<NineSliceResult> = files
        .par_iter()
        .map(|p| match nine_slice::process(p, &output_path, &opts) {
            Ok(report) => NineSliceResult {
                input: p.clone(),
                output_files: report.output_files,
                error: None,
            },
            Err(e) => NineSliceResult {
                input: p.clone(),
                output_files: vec![],
                error: Some(format!("{e}")),
            },
        })
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
            "outputs": r.output_files.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>(),
            "status": if r.error.is_none() { "ok" } else { "failed" },
            "error": r.error,
        })).collect::<Vec<_>>(),
    });

    Ok(tool_text_result(text, structured))
}

struct NineSliceResult {
    input: PathBuf,
    output_files: Vec<PathBuf>,
    error: Option<String>,
}

// ---------- anim_preview ----------

fn anim_preview_handler(args: &Value) -> Result<Value, ToolError> {
    let input = require_string(args, "input")?;
    let output = require_string(args, "output")?;

    let fps = args
        .get("fps")
        .and_then(|v| v.as_u64())
        .unwrap_or(12)
        .clamp(1, 60) as u8;

    let format_str = args
        .get("format")
        .and_then(|v| v.as_str())
        .unwrap_or("gif")
        .to_ascii_lowercase();
    let output_format = match format_str.as_str() {
        "gif" => anim_preview::PreviewFormat::Gif,
        "mp4" => anim_preview::PreviewFormat::Mp4,
        "webm" => anim_preview::PreviewFormat::Webm,
        other => {
            return Err(ToolError::invalid_params(format!(
                "format: expected gif|mp4|webm, got {other}"
            )))
        }
    };

    let upscale = args
        .get("scale")
        .and_then(|v| v.as_u64())
        .unwrap_or(1)
        .clamp(1, 8) as u8;

    let mut frame_size = args
        .get("frame_size")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);

    // Helper: calculate frame_size from cols if provided
    if frame_size.is_none() {
        if let Some(cols) = args.get("sheet_cols").and_then(|v| v.as_u64()) {
            let input_path = Path::new(&input);
            if input_path.is_file() {
                if let Ok(img) = image::open(input_path) {
                    frame_size = Some(img.width() / cols as u32);
                }
            }
        }
    }

    let opts = anim_preview::Options {
        fps,
        output_format,
        loop_anim: true,
        upscale,
        frame_size,
    };

    let input_path = PathBuf::from(&input);
    let output_path = PathBuf::from(&output);

    let is_frame_folder = input_path.is_dir()
        && std::fs::read_dir(&input_path).map(|mut entries| {
            entries.any(|e| {
                e.ok()
                    .map(|e| {
                        e.path()
                            .extension()
                            .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
                    })
                    .unwrap_or(false)
            })
        }).unwrap_or(false);

    if input_path.is_file() || is_frame_folder {
        std::fs::create_dir_all(&output_path)
            .map_err(|e| ToolError::internal(format!("Creating output dir: {e}")))?;

        let report = anim_preview::process(&input_path, &output_path, &opts)
            .map_err(|e| ToolError::internal(format!("Anim preview: {e}")))?;

        let text = format!(
            "Generated {} preview: {} ({} frames, {}px)",
            format_str,
            report.output_path.display(),
            report.frame_count,
            report.frame_size
        );
        let structured = json!({
            "output": report.output_path.to_string_lossy(),
            "frame_count": report.frame_count,
            "frame_size": report.frame_size,
            "format": format_str,
            "status": "ok",
        });
        Ok(tool_text_result(text, structured))
    } else {
        let files = batch::list_images(&input_path, false, &["png"])
            .map_err(|e| ToolError::internal(format!("Listing input: {e}")))?;

        if files.is_empty() {
            return Ok(tool_text_result(
                format!("No sprite sheets found in {}", input_path.display()),
                json!({ "processed": 0, "failed": 0 }),
            ));
        }

        std::fs::create_dir_all(&output_path)
            .map_err(|e| ToolError::internal(format!("Creating output dir: {e}")))?;

        let start = Instant::now();
        let results: Vec<Value> = files
            .par_iter()
            .map(|p| match anim_preview::process(p, &output_path, &opts) {
                Ok(report) => json!({
                    "input": p.to_string_lossy(),
                    "output": report.output_path.to_string_lossy(),
                    "status": "ok"
                }),
                Err(e) => json!({
                    "input": p.to_string_lossy(),
                    "status": "failed",
                    "error": format!("{e}")
                }),
            })
            .collect();
        let duration_ms = start.elapsed().as_millis();

        let processed = results.iter().filter(|v| v["status"] == "ok").count();
        let failed = results.len() - processed;

        Ok(tool_text_result(
            format!(
                "Processed {processed}/{} animations in {duration_ms}ms",
                results.len()
            ),
            json!({
                "processed": processed,
                "failed": failed,
                "results": results
            }),
        ))
    }
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
    use std::sync::{Mutex, MutexGuard, OnceLock};

    /// Serialize tests that mutate `PIXIEKIT_CONFIG_DIR`. Mirrors the pattern
    /// in `pixiekit_core::preset::tests`.
    fn env_lock() -> MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|p| p.into_inner())
    }

    struct ScopedConfigDir {
        _tmp: tempfile::TempDir,
        _guard: MutexGuard<'static, ()>,
    }

    impl ScopedConfigDir {
        fn new() -> Self {
            let guard = env_lock();
            let tmp = tempfile::Builder::new()
                .prefix("pixiekit-mcp-preset-test-")
                .tempdir()
                .unwrap();
            std::env::set_var("PIXIEKIT_CONFIG_DIR", tmp.path());
            Self {
                _tmp: tmp,
                _guard: guard,
            }
        }
    }

    impl Drop for ScopedConfigDir {
        fn drop(&mut self) {
            std::env::remove_var("PIXIEKIT_CONFIG_DIR");
        }
    }

    #[test]
    fn list_tools_returns_thirteen_tools() {
        let tools = list_tools();
        assert_eq!(tools.len(), 13);
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"bg_remove"));
        assert!(names.contains(&"video_to_sprite"));
        assert!(names.contains(&"vectorize"));
        assert!(names.contains(&"atlas_pack"));
        assert!(names.contains(&"optimize_image"));
        assert!(names.contains(&"scale_image"));
        assert!(names.contains(&"audio_process"));
        assert!(names.contains(&"trim_pad"));
        assert!(names.contains(&"svg_optimize"));
        assert!(names.contains(&"list_presets"));
        assert!(names.contains(&"get_preset"));
        assert!(names.contains(&"nine_slice"));
        assert!(names.contains(&"anim_preview"));
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
    fn vectorize_missing_input_returns_invalid_params() {
        let params = json!({
            "name": "vectorize",
            "arguments": { "output": "/tmp/out" }
        });
        let err = call(Some(&params)).unwrap_err();
        assert_eq!(err.code, ERR_INVALID_PARAMS);
    }

    #[test]
    fn vectorize_invalid_mode_returns_invalid_params() {
        let params = json!({
            "name": "vectorize",
            "arguments": { "input": "/tmp/in", "output": "/tmp/out", "mode": "rainbow" }
        });
        let err = call(Some(&params)).unwrap_err();
        assert_eq!(err.code, ERR_INVALID_PARAMS);
    }

    #[test]
    fn vectorize_empty_input_dir_returns_zero() {
        let dir =
            std::env::temp_dir().join(format!("pixiekit-mcp-vec-empty-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let params = json!({
            "name": "vectorize",
            "arguments": { "input": dir.to_string_lossy(), "output": dir.to_string_lossy() }
        });
        let result = call(Some(&params)).unwrap();
        assert_eq!(result["structuredContent"]["processed"], 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn atlas_pack_missing_input_returns_invalid_params() {
        let params = json!({
            "name": "atlas_pack",
            "arguments": { "output": "/tmp/out" }
        });
        let err = call(Some(&params)).unwrap_err();
        assert_eq!(err.code, ERR_INVALID_PARAMS);
    }

    #[test]
    fn atlas_pack_invalid_format_returns_invalid_params() {
        let params = json!({
            "name": "atlas_pack",
            "arguments": {
                "input": "/tmp/in",
                "output": "/tmp/out",
                "format": "gif"
            }
        });
        let err = call(Some(&params)).unwrap_err();
        assert_eq!(err.code, ERR_INVALID_PARAMS);
    }

    #[test]
    fn atlas_pack_empty_input_dir_returns_zero() {
        let dir =
            std::env::temp_dir().join(format!("pixiekit-mcp-atlas-empty-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let params = json!({
            "name": "atlas_pack",
            "arguments": { "input": dir.to_string_lossy(), "output": dir.to_string_lossy() }
        });
        let result = call(Some(&params)).unwrap();
        assert_eq!(result["structuredContent"]["processed"], 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn optimize_image_missing_input_returns_invalid_params() {
        let params = json!({
            "name": "optimize_image",
            "arguments": { "output": "/tmp/out" }
        });
        let err = call(Some(&params)).unwrap_err();
        assert_eq!(err.code, ERR_INVALID_PARAMS);
    }

    #[test]
    fn optimize_image_invalid_format_returns_invalid_params() {
        let params = json!({
            "name": "optimize_image",
            "arguments": { "input": "/tmp/in", "output": "/tmp/out", "target_format": "avif" }
        });
        let err = call(Some(&params)).unwrap_err();
        assert_eq!(err.code, ERR_INVALID_PARAMS);
    }

    #[test]
    fn optimize_image_empty_input_dir_returns_zero() {
        let dir = std::env::temp_dir().join(format!(
            "pixiekit-mcp-optimize-empty-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let params = json!({
            "name": "optimize_image",
            "arguments": { "input": dir.to_string_lossy(), "output": dir.to_string_lossy() }
        });
        let result = call(Some(&params)).unwrap();
        assert_eq!(result["structuredContent"]["processed"], 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn scale_image_missing_input_returns_invalid_params() {
        let params = json!({
            "name": "scale_image",
            "arguments": { "output": "/tmp/out" }
        });
        let err = call(Some(&params)).unwrap_err();
        assert_eq!(err.code, ERR_INVALID_PARAMS);
    }

    #[test]
    fn scale_image_invalid_naming_returns_invalid_params() {
        let params = json!({
            "name": "scale_image",
            "arguments": { "input": "/tmp/in", "output": "/tmp/out", "naming": "xcode" }
        });
        let err = call(Some(&params)).unwrap_err();
        assert_eq!(err.code, ERR_INVALID_PARAMS);
    }

    #[test]
    fn scale_image_empty_target_scales_returns_invalid_params() {
        let params = json!({
            "name": "scale_image",
            "arguments": {
                "input": "/tmp/in",
                "output": "/tmp/out",
                "target_scales": []
            }
        });
        let err = call(Some(&params)).unwrap_err();
        assert_eq!(err.code, ERR_INVALID_PARAMS);
    }

    #[test]
    fn scale_image_empty_input_dir_returns_zero() {
        let dir =
            std::env::temp_dir().join(format!("pixiekit-mcp-scale-empty-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let params = json!({
            "name": "scale_image",
            "arguments": { "input": dir.to_string_lossy(), "output": dir.to_string_lossy() }
        });
        let result = call(Some(&params)).unwrap();
        assert_eq!(result["structuredContent"]["processed"], 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn audio_process_missing_input_returns_invalid_params() {
        let params = json!({
            "name": "audio_process",
            "arguments": { "output": "/tmp/out" }
        });
        let err = call(Some(&params)).unwrap_err();
        assert_eq!(err.code, ERR_INVALID_PARAMS);
        assert!(err.message.contains("input"));
    }

    #[test]
    fn audio_process_invalid_target_format_returns_invalid_params() {
        let params = json!({
            "name": "audio_process",
            "arguments": {
                "input": "/tmp/in",
                "output": "/tmp/out",
                "target_format": "flac"
            }
        });
        let err = call(Some(&params)).unwrap_err();
        assert_eq!(err.code, ERR_INVALID_PARAMS);
    }

    #[test]
    fn audio_process_invalid_channels_returns_invalid_params() {
        let dir =
            std::env::temp_dir().join(format!("pixiekit-mcp-audio-bad-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // Need files to reach the channels parse — use a fake file
        std::fs::write(dir.join("a.wav"), b"x").unwrap();
        let params = json!({
            "name": "audio_process",
            "arguments": {
                "input": dir.to_string_lossy(),
                "output": dir.to_string_lossy(),
                "channels": "surround"
            }
        });
        let err = call(Some(&params)).unwrap_err();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(err.code, ERR_INVALID_PARAMS);
    }

    #[test]
    fn list_presets_returns_empty_list_for_fresh_config_dir() {
        let _scope = ScopedConfigDir::new();
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
    fn list_presets_returns_saved_names_sorted() {
        let _scope = ScopedConfigDir::new();
        preset::save("zeta", preset::TOOL_BG_REMOVE, json!({})).unwrap();
        preset::save("alpha", preset::TOOL_VECTORIZE, json!({})).unwrap();
        let params = json!({ "name": "list_presets", "arguments": {} });
        let result = call(Some(&params)).unwrap();
        let names = result["structuredContent"]["presets"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["alpha", "zeta"]);
    }

    #[test]
    fn get_preset_returns_saved_options() {
        let _scope = ScopedConfigDir::new();
        let opts = json!({"fuzz": 0.5, "erode": 2});
        preset::save("clean", preset::TOOL_BG_REMOVE, opts.clone()).unwrap();
        let params = json!({
            "name": "get_preset",
            "arguments": { "name": "clean" }
        });
        let result = call(Some(&params)).unwrap();
        assert_eq!(result["structuredContent"]["name"], "clean");
        assert_eq!(result["structuredContent"]["tool"], preset::TOOL_BG_REMOVE);
        assert_eq!(result["structuredContent"]["options"], opts);
    }

    #[test]
    fn get_preset_missing_returns_invalid_params() {
        let _scope = ScopedConfigDir::new();
        let params = json!({
            "name": "get_preset",
            "arguments": { "name": "ghost" }
        });
        let err = call(Some(&params)).unwrap_err();
        assert_eq!(err.code, ERR_INVALID_PARAMS);
        assert!(err.message.contains("ghost"));
    }

    #[test]
    fn get_preset_missing_name_arg_returns_invalid_params() {
        let _scope = ScopedConfigDir::new();
        let params = json!({ "name": "get_preset", "arguments": {} });
        let err = call(Some(&params)).unwrap_err();
        assert_eq!(err.code, ERR_INVALID_PARAMS);
    }

    #[test]
    fn get_preset_invalid_name_returns_invalid_params() {
        let _scope = ScopedConfigDir::new();
        let params = json!({
            "name": "get_preset",
            "arguments": { "name": "../etc/passwd" }
        });
        let err = call(Some(&params)).unwrap_err();
        assert_eq!(err.code, ERR_INVALID_PARAMS);
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
