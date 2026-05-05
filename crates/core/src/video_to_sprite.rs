//! Video → horizontal sprite sheet pipeline.
//!
//! Pipeline stages:
//!
//! 1. **Extract** — `ffmpeg` decodes video to PNG frames at target fps + size
//! 2. **Chroma key** (optional) — apply [`crate::bg_remove`] per frame
//! 3. **Stitch** — concatenate frames horizontally into a single image
//! 4. **Encode** — write PNG (lossless) or WebP (lossy q=90, alpha lossless)
//! 5. **Metadata** — write JSON sibling with frame count, fps, duration
//!
//! Replaces the legacy bash pipeline:
//! `extract-sprites-smooth.sh` + `stitch-sprites-smooth.sh`.

use std::path::{Path, PathBuf};
use std::process::Command;

use image::{GenericImage, RgbaImage};
use serde::{Deserialize, Serialize};

use crate::bg_remove;
use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    Png,
    Webp,
}

impl OutputFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            OutputFormat::Png => "png",
            OutputFormat::Webp => "webp",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Options {
    /// Target output FPS (1 - 30). Default 8.
    pub fps: u8,

    /// Output frame size, square (64 - 1024). Default 256.
    pub frame_size: u32,

    /// Output container format.
    pub output_format: OutputFormat,

    /// WebP quality 0-100 (ignored for PNG). Alpha is always lossless. Default 90.
    pub webp_quality: u8,

    /// If `Some`, apply BG remove per frame before stitching.
    pub chroma_key: Option<bg_remove::Options>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            fps: 8,
            frame_size: 256,
            output_format: OutputFormat::Webp,
            webp_quality: 90,
            chroma_key: None,
        }
    }
}

/// Metadata describing a generated sprite sheet — written as JSON sibling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub frame_count: u32,
    pub frame_size: u32,
    pub fps: u8,
    pub total_duration_ms: u32,
    pub source_video: String,
}

/// Result of processing a single video.
#[derive(Debug, Clone)]
pub struct ProcessReport {
    pub sprite_path: PathBuf,
    pub metadata_path: PathBuf,
    pub frame_count: u32,
    pub frame_size: u32,
}

/// Process a single video into a sprite sheet + metadata JSON.
///
/// Output filenames are derived from `input_video` stem:
/// - `<stem>.png` or `<stem>.webp`
/// - `<stem>.json`
///
/// Created in `output_dir`. Caller is responsible for `output_dir` existing.
pub fn process(input_video: &Path, output_dir: &Path, opts: &Options) -> Result<ProcessReport> {
    if !input_video.exists() {
        return Err(Error::NotFound(input_video.to_path_buf()));
    }
    check_ffmpeg()?;

    let tmp = tempfile::Builder::new().prefix("pixiekit-v2s-").tempdir()?;

    // 1. Extract frames via ffmpeg
    let frame_paths = extract_frames(input_video, tmp.path(), opts.fps, opts.frame_size)?;
    if frame_paths.is_empty() {
        return Err(Error::NoFrames(input_video.to_path_buf()));
    }

    // 2-3. Read, optionally chroma-key, and stitch
    let frames = read_and_process_frames(&frame_paths, opts)?;
    let sheet = stitch_horizontal(&frames, opts.frame_size)?;

    // 4. Encode + write
    let stem = input_video
        .file_stem()
        .ok_or_else(|| Error::InvalidInput(format!("Bad filename: {}", input_video.display())))?
        .to_string_lossy();
    let sprite_path = output_dir.join(format!("{}.{}", stem, opts.output_format.extension()));
    let metadata_path = output_dir.join(format!("{}.json", stem));

    encode_sprite_sheet(&sheet, &sprite_path, opts)?;

    // 5. Write metadata
    let frame_count = frames.len() as u32;
    let metadata = Metadata {
        frame_count,
        frame_size: opts.frame_size,
        fps: opts.fps,
        total_duration_ms: (frame_count * 1000) / opts.fps.max(1) as u32,
        source_video: input_video.to_string_lossy().into_owned(),
    };
    let json = serde_json::to_string_pretty(&metadata)?;
    std::fs::write(&metadata_path, json)?;

    Ok(ProcessReport {
        sprite_path,
        metadata_path,
        frame_count,
        frame_size: opts.frame_size,
    })
}

/// Verify `ffmpeg` is callable. Errors with [`Error::FfmpegMissing`] otherwise.
pub fn check_ffmpeg() -> Result<()> {
    let result = Command::new("ffmpeg").arg("-version").output();
    match result {
        Ok(output) if output.status.success() => Ok(()),
        _ => Err(Error::FfmpegMissing),
    }
}

fn extract_frames(video: &Path, out_dir: &Path, fps: u8, size: u32) -> Result<Vec<PathBuf>> {
    let pattern = out_dir.join("frame_%04d.png");
    let vf = format!(
        "fps={},scale={size}:{size}:flags=lanczos",
        fps.clamp(1, 30),
        size = size.clamp(16, 4096)
    );

    let output = Command::new("ffmpeg")
        .args(["-y", "-i"])
        .arg(video)
        .args(["-vf", &vf])
        .arg(&pattern)
        .output()?;

    if !output.status.success() {
        return Err(Error::FfmpegFailed {
            code: output.status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }

    let mut frames: Vec<_> = std::fs::read_dir(out_dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "png"))
        .collect();
    frames.sort();
    Ok(frames)
}

fn read_and_process_frames(paths: &[PathBuf], opts: &Options) -> Result<Vec<RgbaImage>> {
    paths
        .iter()
        .map(|p| {
            let img = image::open(p)?.into_rgba8();
            Ok(if let Some(ck_opts) = &opts.chroma_key {
                bg_remove::process(&img, ck_opts)
            } else {
                img
            })
        })
        .collect()
}

fn stitch_horizontal(frames: &[RgbaImage], frame_size: u32) -> Result<RgbaImage> {
    // Validate uniform size before allocating
    for f in frames {
        if f.width() != frame_size || f.height() != frame_size {
            return Err(Error::InconsistentFrameSize {
                expected: frame_size,
                got_w: f.width(),
                got_h: f.height(),
            });
        }
    }

    let total_width = frame_size * frames.len() as u32;
    let mut sheet = RgbaImage::new(total_width, frame_size);
    for (i, frame) in frames.iter().enumerate() {
        let x_offset = (i as u32) * frame_size;
        // GenericImage::copy_from copies pixels with no alpha blending
        sheet.copy_from(frame, x_offset, 0)?;
    }
    Ok(sheet)
}

fn encode_sprite_sheet(sheet: &RgbaImage, path: &Path, opts: &Options) -> Result<()> {
    match opts.output_format {
        OutputFormat::Png => {
            sheet.save(path)?;
        }
        OutputFormat::Webp => {
            let encoder = webp::Encoder::from_rgba(sheet.as_raw(), sheet.width(), sheet.height());
            let webp_data = encoder.encode(opts.webp_quality.clamp(0, 100) as f32);
            std::fs::write(path, &*webp_data)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn solid_frame(size: u32, color: [u8; 4]) -> RgbaImage {
        let mut img = RgbaImage::new(size, size);
        for pixel in img.pixels_mut() {
            *pixel = Rgba(color);
        }
        img
    }

    #[test]
    fn output_format_extensions() {
        assert_eq!(OutputFormat::Png.extension(), "png");
        assert_eq!(OutputFormat::Webp.extension(), "webp");
    }

    #[test]
    fn options_default_values() {
        let opts = Options::default();
        assert_eq!(opts.fps, 8);
        assert_eq!(opts.frame_size, 256);
        assert_eq!(opts.output_format, OutputFormat::Webp);
        assert_eq!(opts.webp_quality, 90);
        assert!(opts.chroma_key.is_none());
    }

    #[test]
    fn stitch_three_frames_horizontal() {
        let frames = vec![
            solid_frame(64, [255, 0, 0, 255]),
            solid_frame(64, [0, 255, 0, 255]),
            solid_frame(64, [0, 0, 255, 255]),
        ];
        let sheet = stitch_horizontal(&frames, 64).unwrap();
        assert_eq!(sheet.dimensions(), (192, 64));

        // Spot-check: frame 0 is red at x=10, frame 1 is green at x=70, frame 2 blue at x=130
        assert_eq!(sheet.get_pixel(10, 32)[0], 255); // red
        assert_eq!(sheet.get_pixel(70, 32)[1], 255); // green
        assert_eq!(sheet.get_pixel(130, 32)[2], 255); // blue
    }

    #[test]
    fn stitch_rejects_inconsistent_size() {
        let frames = vec![
            solid_frame(64, [255, 0, 0, 255]),
            solid_frame(48, [0, 255, 0, 255]), // wrong size
        ];
        let result = stitch_horizontal(&frames, 64);
        assert!(matches!(
            result,
            Err(Error::InconsistentFrameSize { expected: 64, .. })
        ));
    }

    #[test]
    fn stitch_single_frame_works() {
        let frames = vec![solid_frame(32, [128, 128, 128, 255])];
        let sheet = stitch_horizontal(&frames, 32).unwrap();
        assert_eq!(sheet.dimensions(), (32, 32));
    }

    #[test]
    fn check_ffmpeg_works_when_installed() {
        // This test passes only if ffmpeg is in PATH (which is required for video-to-sprite).
        // Skip silently if not available — useful for CI without ffmpeg.
        if Command::new("ffmpeg").arg("-version").output().is_ok() {
            assert!(check_ffmpeg().is_ok());
        }
    }
}
