//! Image Optimizer — PNG/JPG/WebP byte-size reduction.
//!
//! Pipeline (selected per-input by `target_format` and the source extension):
//! 1. **PNG → PNG**: run `oxipng` at the configured optimization level. Strips
//!    metadata (`StripChunks::Safe`) when [`Options::strip_metadata`] is true.
//! 2. **PNG → WebP**: decode via `image`, encode via `webp` (lossy at
//!    `quality`, or lossless when [`Options::lossless`] is true).
//! 3. **JPEG → JPEG**: decode + re-encode via `image`'s built-in JPEG
//!    encoder at `quality`.
//! 4. **WebP → WebP**: decode + re-encode via the `webp` crate at the same
//!    quality / lossless setting as PNG → WebP.
//! 5. **Keep mode**: forces the source format; otherwise the user-selected
//!    `target_format` wins.
//!
//! Module is pure logic: callers are responsible for batching and progress UI.
//! Returns an [`OptimizeReport`] describing the byte ratio so frontends can
//! print human-readable savings.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Output container — `Keep` re-encodes into the input format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TargetFormat {
    Png,
    #[default]
    Webp,
    Keep,
}

/// Per-call optimizer options. Defaults match PRD §6.5.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Options {
    /// Output container.
    pub target_format: TargetFormat,
    /// Lossy quality 0-100 (ignored when `lossless` is true). Default 90.
    pub quality: u8,
    /// Use lossless WebP encoding. Default false.
    pub lossless: bool,
    /// Strip safe metadata chunks (e.g. EXIF/text on PNG). Default true.
    pub strip_metadata: bool,
    /// `oxipng` optimization preset 0-6 (higher = slower, smaller). Default 3.
    pub optimization_level: u8,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            target_format: TargetFormat::Webp,
            quality: 90,
            lossless: false,
            strip_metadata: true,
            optimization_level: 3,
        }
    }
}

/// Report describing one file's optimization result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizeReport {
    /// Final output path (extension may differ from input).
    pub output_path: PathBuf,
    /// Bytes on disk for the source file.
    pub input_size: u64,
    /// Bytes on disk for the output file.
    pub output_size: u64,
    /// `output_size / input_size` (1.0 == no change, 0.5 == 50% smaller).
    pub ratio: f32,
}

/// Optimize a single file. The output extension is derived from the resolved
/// target format; callers should not pre-attach an extension to `output`.
///
/// `output` is treated as a path stem when [`Options::target_format`] is
/// [`TargetFormat::Keep`] OR when its extension already matches the resolved
/// format. In both cases we just copy the resolved extension on top.
pub fn process(input: &Path, output: &Path, opts: &Options) -> Result<OptimizeReport> {
    if !input.exists() {
        return Err(Error::NotFound(input.to_path_buf()));
    }

    let input_ext = input
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .ok_or_else(|| {
            Error::InvalidInput(format!("Input has no extension: {}", input.display()))
        })?;

    let resolved_ext = match opts.target_format {
        TargetFormat::Png => "png".to_string(),
        TargetFormat::Webp => "webp".to_string(),
        TargetFormat::Keep => match input_ext.as_str() {
            "jpg" | "jpeg" => "jpg".to_string(),
            other => other.to_string(),
        },
    };

    let output_path = with_extension(output, &resolved_ext);

    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let bytes = encode_for_target(input, &input_ext, &resolved_ext, opts)?;
    std::fs::write(&output_path, &bytes)?;

    let input_size = std::fs::metadata(input)?.len();
    let output_size = bytes.len() as u64;
    let ratio = if input_size == 0 {
        0.0
    } else {
        output_size as f32 / input_size as f32
    };

    Ok(OptimizeReport {
        output_path,
        input_size,
        output_size,
        ratio,
    })
}

fn encode_for_target(
    input: &Path,
    input_ext: &str,
    output_ext: &str,
    opts: &Options,
) -> Result<Vec<u8>> {
    match (input_ext, output_ext) {
        ("png", "png") => optimize_png(input, opts),
        ("png", "webp") | ("jpg", "webp") | ("jpeg", "webp") | ("webp", "webp") => {
            encode_webp(input, opts)
        }
        ("jpg", "jpg") | ("jpeg", "jpg") | ("jpg", "jpeg") | ("jpeg", "jpeg") => {
            encode_jpeg(input, opts)
        }
        // Cross-decode fallbacks (e.g. JPEG → PNG or WebP → PNG): decode then
        // re-encode as PNG and (optionally) feed it through oxipng.
        (_, "png") => decode_and_optimize_to_png(input, opts),
        (other_in, other_out) => Err(Error::InvalidInput(format!(
            "Unsupported optimize conversion: {other_in} → {other_out}"
        ))),
    }
}

fn optimize_png(input: &Path, opts: &Options) -> Result<Vec<u8>> {
    let raw = std::fs::read(input)?;
    optimize_png_bytes(&raw, opts)
}

fn optimize_png_bytes(raw: &[u8], opts: &Options) -> Result<Vec<u8>> {
    let level = opts.optimization_level.min(6);
    let mut oxi_opts = oxipng::Options::from_preset(level);
    oxi_opts.strip = if opts.strip_metadata {
        oxipng::StripChunks::Safe
    } else {
        oxipng::StripChunks::None
    };
    oxipng::optimize_from_memory(raw, &oxi_opts).map_err(|e| Error::OxipngFailed(e.to_string()))
}

fn encode_webp(input: &Path, opts: &Options) -> Result<Vec<u8>> {
    let img = image::open(input)?.to_rgba8();
    let encoder = webp::Encoder::from_rgba(img.as_raw(), img.width(), img.height());
    let data = if opts.lossless {
        encoder.encode_lossless()
    } else {
        encoder.encode(opts.quality.min(100) as f32)
    };
    Ok(data.to_vec())
}

fn encode_jpeg(input: &Path, opts: &Options) -> Result<Vec<u8>> {
    let img = image::open(input)?.to_rgb8();
    let mut buf = std::io::Cursor::new(Vec::new());
    let encoder =
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, opts.quality.min(100));
    let mut encoder = encoder;
    encoder.encode(
        img.as_raw(),
        img.width(),
        img.height(),
        image::ExtendedColorType::Rgb8,
    )?;
    Ok(buf.into_inner())
}

fn decode_and_optimize_to_png(input: &Path, opts: &Options) -> Result<Vec<u8>> {
    let img = image::open(input)?.to_rgba8();
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)?;
    optimize_png_bytes(&buf.into_inner(), opts)
}

/// Replace (or attach) the extension on a path without consuming it.
fn with_extension(path: &Path, ext: &str) -> PathBuf {
    let mut p = path.to_path_buf();
    p.set_extension(ext);
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    fn tmpdir(test_name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "pixiekit-optimize-test-{}-{}",
            std::process::id(),
            test_name
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_png(path: &Path, w: u32, h: u32) {
        let mut img = RgbaImage::new(w, h);
        // Use a varied gradient so PNG isn't already optimal-tiny.
        for y in 0..h {
            for x in 0..w {
                let r = ((x * 7) % 256) as u8;
                let g = ((y * 11) % 256) as u8;
                let b = ((x.wrapping_add(y) * 5) % 256) as u8;
                img.put_pixel(x, y, Rgba([r, g, b, 255]));
            }
        }
        img.save(path).unwrap();
    }

    #[test]
    fn defaults_match_prd() {
        let opts = Options::default();
        assert_eq!(opts.target_format, TargetFormat::Webp);
        assert_eq!(opts.quality, 90);
        assert!(!opts.lossless);
        assert!(opts.strip_metadata);
        assert_eq!(opts.optimization_level, 3);
    }

    #[test]
    fn missing_input_errors() {
        let dir = tmpdir("missing_input");
        let input = dir.join("ghost.png");
        let output = dir.join("out");
        let err = process(&input, &output, &Options::default()).unwrap_err();
        assert!(matches!(err, Error::NotFound(_)));
    }

    #[test]
    fn png_to_png_produces_smaller_or_equal() {
        let dir = tmpdir("png_to_png");
        let input = dir.join("in.png");
        let output = dir.join("out");
        write_png(&input, 64, 64);

        let opts = Options {
            target_format: TargetFormat::Png,
            optimization_level: 2,
            ..Default::default()
        };
        let report = process(&input, &output, &opts).unwrap();
        assert!(report.output_path.exists());
        assert_eq!(
            report.output_path.extension().and_then(|e| e.to_str()),
            Some("png")
        );
        assert!(report.input_size > 0);
        assert!(report.output_size > 0);
        assert!(
            report.output_size <= report.input_size,
            "oxipng should not enlarge: in={} out={}",
            report.input_size,
            report.output_size
        );
        assert!(report.ratio > 0.0);
    }

    #[test]
    fn png_to_webp_produces_webp_output() {
        let dir = tmpdir("png_to_webp");
        let input = dir.join("in.png");
        let output = dir.join("out");
        write_png(&input, 64, 64);

        let opts = Options {
            target_format: TargetFormat::Webp,
            quality: 80,
            ..Default::default()
        };
        let report = process(&input, &output, &opts).unwrap();
        assert_eq!(
            report.output_path.extension().and_then(|e| e.to_str()),
            Some("webp")
        );
        assert!(report.output_size > 0);
        // Lossy WebP at q=80 on a 64x64 noisy PNG is typically smaller.
        assert!(
            report.output_size < report.input_size,
            "expected WebP smaller than PNG: in={} out={}",
            report.input_size,
            report.output_size
        );
    }

    #[test]
    fn keep_format_keeps_png() {
        let dir = tmpdir("keep_png");
        let input = dir.join("in.png");
        let output = dir.join("out");
        write_png(&input, 32, 32);

        let opts = Options {
            target_format: TargetFormat::Keep,
            ..Default::default()
        };
        let report = process(&input, &output, &opts).unwrap();
        assert_eq!(
            report.output_path.extension().and_then(|e| e.to_str()),
            Some("png")
        );
    }

    #[test]
    fn lossless_webp_encodes_successfully() {
        let dir = tmpdir("lossless_webp");
        let input = dir.join("in.png");
        let output = dir.join("out");
        write_png(&input, 32, 32);

        let opts = Options {
            target_format: TargetFormat::Webp,
            lossless: true,
            ..Default::default()
        };
        let report = process(&input, &output, &opts).unwrap();
        assert!(report.output_size > 0);
    }

    #[test]
    fn optimization_level_clamped_to_six() {
        // Sanity: oxipng presets are 0-6. Calling with 100 must not panic.
        let dir = tmpdir("level_clamp");
        let input = dir.join("in.png");
        let output = dir.join("out");
        write_png(&input, 16, 16);

        let opts = Options {
            target_format: TargetFormat::Png,
            optimization_level: 100,
            ..Default::default()
        };
        let report = process(&input, &output, &opts).unwrap();
        assert!(report.output_size > 0);
    }

    #[test]
    fn ratio_computed_correctly() {
        let dir = tmpdir("ratio");
        let input = dir.join("in.png");
        let output = dir.join("out");
        write_png(&input, 32, 32);

        let opts = Options {
            target_format: TargetFormat::Png,
            ..Default::default()
        };
        let report = process(&input, &output, &opts).unwrap();
        let expected = report.output_size as f32 / report.input_size as f32;
        assert!((report.ratio - expected).abs() < f32::EPSILON);
    }

    #[test]
    fn strip_metadata_off_still_succeeds() {
        let dir = tmpdir("no_strip");
        let input = dir.join("in.png");
        let output = dir.join("out");
        write_png(&input, 32, 32);

        let opts = Options {
            target_format: TargetFormat::Png,
            strip_metadata: false,
            ..Default::default()
        };
        let report = process(&input, &output, &opts).unwrap();
        assert!(report.output_path.exists());
    }
}
