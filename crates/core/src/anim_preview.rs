//! Animation Preview — converts sprite sheets or frame folders into GIF/MP4/WebM.
//!
//! Reuses `ffmpeg` for encoding (mirrors [`crate::video_to_sprite`] and [`crate::audio`]).
//! Supports:
//! - Sprite sheet mode: Splits a single horizontal PNG into frames.
//! - Folder mode: Reads a directory of PNG frames.
//! - Nearest-neighbor upscaling (M11.3).
//! - Two-pass palette generation for high-quality GIFs (M11.5).

use std::path::{Path, PathBuf};
use std::process::Command;
use serde::{Deserialize, Serialize};
use image::{GenericImageView, imageops::FilterType};

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PreviewFormat {
    Gif,
    Mp4,
    Webm,
}

impl PreviewFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            PreviewFormat::Gif => "gif",
            PreviewFormat::Mp4 => "mp4",
            PreviewFormat::Webm => "webm",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Options {
    /// Frames per second (1 - 30). Default 8.
    pub fps: u8,
    /// Output container format.
    pub output_format: PreviewFormat,
    /// Loop the animation (only affects GIF and metadata).
    pub loop_anim: bool,
    /// Upscale factor (1, 2, 4). Default 1. Uses nearest-neighbor.
    pub upscale: u8,
    /// Size of each square frame in a sprite sheet. If None, auto-detected from sibling JSON.
    pub frame_size: Option<u32>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            fps: 8,
            output_format: PreviewFormat::Gif,
            loop_anim: true,
            upscale: 1,
            frame_size: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimPreviewReport {
    pub output_path: PathBuf,
    pub frame_count: u32,
    pub frame_size: u32,
    pub format: PreviewFormat,
}

/// Process a sprite sheet (file) or frame folder (directory) into an animation preview.
pub fn process(input: &Path, output_dir: &Path, opts: &Options) -> Result<AnimPreviewReport> {
    if !input.exists() {
        return Err(Error::NotFound(input.to_path_buf()));
    }
    check_ffmpeg()?;

    let tmp = tempfile::Builder::new().prefix("pixiekit-ap-").tempdir()?;
    let frames_dir = tmp.path();

    let mut frame_count = 0;
    let mut frame_size = 0;

    if input.is_file() {
        // Sprite sheet mode
        let img = image::open(input).map_err(Error::Image)?;
        let (w, h) = img.dimensions();

        let size = if let Some(s) = opts.frame_size {
            s
        } else {
            // Attempt auto-detect from sibling JSON
            try_detect_frame_size(input).unwrap_or(h) // Fallback to height (assume square)
        };

        frame_size = size;
        frame_count = w / size;

        for i in 0..frame_count {
            let mut frame = img.view(i * size, 0, size, size).to_image();
            
            // Optional upscale
            if opts.upscale > 1 {
                let new_size = size * opts.upscale as u32;
                frame = image::imageops::resize(&frame, new_size, new_size, FilterType::Nearest);
            }

            let frame_path = frames_dir.join(format!("frame_{:04}.png", i));
            frame.save(frame_path).map_err(Error::Image)?;
        }
    } else if input.is_dir() {
        // Folder mode
        let mut frames: Vec<_> = std::fs::read_dir(input)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("png")))
            .collect();
        frames.sort();

        frame_count = frames.len() as u32;
        if frame_count == 0 {
            return Err(Error::NoFrames(input.to_path_buf()));
        }

        for (i, p) in frames.iter().enumerate() {
            let mut frame = image::open(p).map_err(Error::Image)?.into_rgba8();
            if i == 0 {
                frame_size = frame.width();
            }

            // Optional upscale
            if opts.upscale > 1 {
                let new_size = frame.width() * opts.upscale as u32;
                frame = image::imageops::resize(&frame, new_size, new_size, FilterType::Nearest);
            }

            let frame_path = frames_dir.join(format!("frame_{:04}.png", i));
            frame.save(frame_path).map_err(Error::Image)?;
        }
    }

    let stem = input.file_stem().unwrap().to_string_lossy();
    let output_path = output_dir.join(format!("{}.{}", stem, opts.output_format.extension()));

    encode_animation(frames_dir, &output_path, opts)?;

    Ok(AnimPreviewReport {
        output_path,
        frame_count,
        frame_size,
        format: opts.output_format,
    })
}

fn try_detect_frame_size(png_path: &Path) -> Option<u32> {
    let json_path = png_path.with_extension("json");
    if !json_path.exists() {
        return None;
    }

    let content = std::fs::read_to_string(json_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;

    // Try a few common spots:
    // 1. "frame_size" (our own metadata)
    // 2. "frameSize"
    // 3. "size"
    if let Some(s) = json.get("frame_size").and_then(|v| v.as_u64()) {
        return Some(s as u32);
    }
    if let Some(s) = json.get("frameSize").and_then(|v| v.as_u64()) {
        return Some(s as u32);
    }

    None
}

fn encode_animation(frames_dir: &Path, output: &Path, opts: &Options) -> Result<()> {
    let pattern = frames_dir.join("frame_%04d.png");
    let fps = opts.fps.clamp(1, 30).to_string();

    match opts.output_format {
        PreviewFormat::Gif => {
            // Two-pass palette generation for high quality
            let palette_path = frames_dir.join("palette.png");
            
            // Pass 1: Palette gen
            let res = Command::new("ffmpeg")
                .args(["-y", "-framerate", &fps, "-i"])
                .arg(&pattern)
                .args(["-vf", "palettegen", "-f", "image2"])
                .arg(&palette_path)
                .output()?;

            if !res.status.success() {
                return Err(Error::FfmpegFailed {
                    code: res.status.code().unwrap_or(-1),
                    stderr: String::from_utf8_lossy(&res.stderr).into_owned(),
                });
            }

            // Pass 2: Palette use
            let loop_val = if opts.loop_anim { "0" } else { "-1" };
            let res = Command::new("ffmpeg")
                .args(["-y", "-framerate", &fps, "-i"])
                .arg(&pattern)
                .args(["-i"])
                .arg(&palette_path)
                .args(["-filter_complex", "paletteuse", "-loop", loop_val])
                .arg(output)
                .output()?;

            if !res.status.success() {
                return Err(Error::FfmpegFailed {
                    code: res.status.code().unwrap_or(-1),
                    stderr: String::from_utf8_lossy(&res.stderr).into_owned(),
                });
            }
        }
        PreviewFormat::Mp4 => {
            let res = Command::new("ffmpeg")
                .args(["-y", "-framerate", &fps, "-i"])
                .arg(&pattern)
                .args(["-c:v", "libx264", "-pix_fmt", "yuv420p", "-movflags", "faststart"])
                .arg(output)
                .output()?;

            if !res.status.success() {
                return Err(Error::FfmpegFailed {
                    code: res.status.code().unwrap_or(-1),
                    stderr: String::from_utf8_lossy(&res.stderr).into_owned(),
                });
            }
        }
        PreviewFormat::Webm => {
            let res = Command::new("ffmpeg")
                .args(["-y", "-framerate", &fps, "-i"])
                .arg(&pattern)
                .args(["-c:v", "libvpx-vp9", "-pix_fmt", "yuva420p"])
                .arg(output)
                .output()?;

            if !res.status.success() {
                return Err(Error::FfmpegFailed {
                    code: res.status.code().unwrap_or(-1),
                    stderr: String::from_utf8_lossy(&res.stderr).into_owned(),
                });
            }
        }
    }

    Ok(())
}

pub fn check_ffmpeg() -> Result<()> {
    let result = Command::new("ffmpeg").arg("-version").output();
    match result {
        Ok(output) if output.status.success() => Ok(()),
        _ => Err(Error::FfmpegMissing),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use image::RgbaImage;

    fn create_test_sprite(width: u32, height: u32, frames: u32) -> PathBuf {
        let dir = std::env::temp_dir().join("pixiekit-anim-test");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("sprite_{}x{}_{}.png", width, height, frames));
        let mut img = RgbaImage::new(width * frames, height);
        // Draw something simple
        for i in 0..frames {
            let color = [ (i * 20) as u8, 0, 0, 255 ];
            for x in 0..width {
                for y in 0..height {
                    img.put_pixel(i * width + x, y, image::Rgba(color));
                }
            }
        }
        img.save(&path).unwrap();
        path
    }

    #[test]
    fn sprite_sheet_split_works() {
        let path = create_test_sprite(32, 32, 4);
        let out_dir = path.parent().unwrap();
        let opts = Options {
            frame_size: Some(32),
            ..Options::default()
        };

        // We can't easily test the ffmpeg part in unit tests without ffmpeg on CI,
        // but we can test the pre-processing logic.
        // Actually, process() calls check_ffmpeg() and encode_animation().
        // Let's mock it or skip if ffmpeg missing.
        if check_ffmpeg().is_err() { return; }

        let report = process(&path, out_dir, &opts).unwrap();
        assert_eq!(report.frame_count, 4);
        assert_eq!(report.frame_size, 32);
        assert!(report.output_path.exists());
    }

    #[test]
    fn frame_size_auto_detect_works() {
        let path = create_test_sprite(32, 32, 4);
        let json_path = path.with_extension("json");
        fs::write(&json_path, r#"{"frame_size": 32}"#).unwrap();

        let detected = try_detect_frame_size(&path);
        assert_eq!(detected, Some(32));
    }
}
