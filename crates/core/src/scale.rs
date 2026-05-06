//! Multi-Resolution Scaler — produce multiple density variants of an image.
//!
//! Three layout conventions, all writing into a user-supplied output dir:
//!
//! - [`NamingMode::Flutter`] — `<scale>x/<filename>` (e.g. `1.0x/foo.png`).
//! - [`NamingMode::Suffix`] — `<stem>@<scale>x.<ext>`. The 1.0x variant drops
//!   the suffix entirely so iOS asset catalogs see `foo.png` for the base.
//! - [`NamingMode::Nested`] — `<scale>/<filename>` (no `x` suffix on the dir).
//!
//! Resampling uses [`image::imageops::resize`] with the requested filter. The
//! defaults assume artwork is authored at 4× and downsampled — pass a custom
//! `base_scale` if your source is e.g. 3×.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Output naming convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NamingMode {
    /// Flutter `<scale>x/<filename>` (e.g. `2.0x/foo.png`).
    #[default]
    Flutter,
    /// iOS `<stem>@<scale>x.<ext>` (1.0x → no suffix).
    Suffix,
    /// Plain `<scale>/<filename>` (e.g. `2/foo.png`).
    Nested,
}

/// Resampling filter — maps directly to [`image::imageops::FilterType`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Filter {
    #[default]
    Lanczos,
    Bilinear,
    Nearest,
}

impl Filter {
    fn to_imageops(self) -> image::imageops::FilterType {
        match self {
            Filter::Lanczos => image::imageops::FilterType::Lanczos3,
            Filter::Bilinear => image::imageops::FilterType::Triangle,
            Filter::Nearest => image::imageops::FilterType::Nearest,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Options {
    /// The density the source asset is authored at. Default 4.0 (Flutter convention).
    pub base_scale: f32,
    /// Densities to emit. Default `[1.0, 1.5, 2.0, 3.0]`.
    pub target_scales: Vec<f32>,
    /// Output naming convention.
    pub naming: NamingMode,
    /// Resampling filter.
    pub filter: Filter,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            base_scale: 4.0,
            target_scales: vec![1.0, 1.5, 2.0, 3.0],
            naming: NamingMode::default(),
            filter: Filter::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScaleReport {
    /// One entry per emitted variant, in the order of `target_scales`.
    pub variants: Vec<PathBuf>,
}

/// Scale a single image into N variants under `output_dir`.
///
/// Layout follows [`Options::naming`]; the caller does not need to pre-create
/// the per-scale subdirectories. Returns the list of emitted paths.
pub fn process(input: &Path, output_dir: &Path, opts: &Options) -> Result<ScaleReport> {
    if !input.exists() {
        return Err(Error::NotFound(input.to_path_buf()));
    }
    if opts.target_scales.is_empty() {
        return Err(Error::InvalidInput(
            "target_scales must not be empty".into(),
        ));
    }
    if !opts.base_scale.is_finite() || opts.base_scale <= 0.0 {
        return Err(Error::InvalidInput(format!(
            "base_scale must be > 0, got {}",
            opts.base_scale
        )));
    }

    let img = image::open(input)?.to_rgba8();
    let (src_w, src_h) = img.dimensions();
    if src_w == 0 || src_h == 0 {
        return Err(Error::InvalidInput(format!(
            "Input has zero dimension: {}",
            input.display()
        )));
    }

    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| Error::InvalidInput(format!("Invalid filename: {}", input.display())))?
        .to_string();
    let ext = input
        .extension()
        .and_then(|s| s.to_str())
        .ok_or_else(|| Error::InvalidInput(format!("Input has no extension: {}", input.display())))?
        .to_string();

    let filter = opts.filter.to_imageops();

    let mut variants = Vec::with_capacity(opts.target_scales.len());
    for &scale in &opts.target_scales {
        if !scale.is_finite() || scale <= 0.0 {
            return Err(Error::InvalidInput(format!(
                "target_scale must be > 0, got {scale}"
            )));
        }
        let factor = scale / opts.base_scale;
        let new_w = ((src_w as f32) * factor).round().max(1.0) as u32;
        let new_h = ((src_h as f32) * factor).round().max(1.0) as u32;

        let resized = if new_w == src_w && new_h == src_h {
            img.clone()
        } else {
            image::imageops::resize(&img, new_w, new_h, filter)
        };

        let out_path = build_variant_path(output_dir, &stem, &ext, scale, opts.naming);
        if let Some(parent) = out_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        resized.save(&out_path)?;
        variants.push(out_path);
    }

    Ok(ScaleReport { variants })
}

fn build_variant_path(
    output_dir: &Path,
    stem: &str,
    ext: &str,
    scale: f32,
    naming: NamingMode,
) -> PathBuf {
    let scale_label = format_scale(scale);
    match naming {
        NamingMode::Flutter => output_dir
            .join(format!("{scale_label}x"))
            .join(format!("{stem}.{ext}")),
        NamingMode::Suffix => {
            // 1.0x → drop the suffix so iOS asset catalogs see the bare name.
            if (scale - 1.0).abs() < f32::EPSILON {
                output_dir.join(format!("{stem}.{ext}"))
            } else {
                output_dir.join(format!("{stem}@{scale_label}x.{ext}"))
            }
        }
        NamingMode::Nested => output_dir.join(scale_label).join(format!("{stem}.{ext}")),
    }
}

/// Render a scale value as `1`, `1.5`, `2`, `3` — no trailing zeros.
fn format_scale(scale: f32) -> String {
    if scale.fract().abs() < f32::EPSILON {
        format!("{}", scale.round() as i32)
    } else {
        // 2 dp is enough for 1.5x / 1.25x conventions; trim trailing zero.
        let s = format!("{scale:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    fn tmpdir(test_name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "pixiekit-scale-test-{}-{}",
            std::process::id(),
            test_name
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_png(path: &Path, w: u32, h: u32) {
        let mut img = RgbaImage::new(w, h);
        for pixel in img.pixels_mut() {
            *pixel = Rgba([200, 100, 50, 255]);
        }
        img.save(path).unwrap();
    }

    #[test]
    fn defaults_are_flutter_lanczos_4x() {
        let opts = Options::default();
        assert!((opts.base_scale - 4.0).abs() < f32::EPSILON);
        assert_eq!(opts.target_scales, vec![1.0, 1.5, 2.0, 3.0]);
        assert_eq!(opts.naming, NamingMode::Flutter);
        assert_eq!(opts.filter, Filter::Lanczos);
    }

    #[test]
    fn missing_input_errors() {
        let dir = tmpdir("missing");
        let input = dir.join("ghost.png");
        let err = process(&input, &dir, &Options::default()).unwrap_err();
        assert!(matches!(err, Error::NotFound(_)));
    }

    #[test]
    fn empty_target_scales_errors() {
        let dir = tmpdir("empty_scales");
        let input = dir.join("in.png");
        write_png(&input, 32, 32);

        let opts = Options {
            target_scales: vec![],
            ..Default::default()
        };
        let err = process(&input, &dir, &opts).unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)));
    }

    #[test]
    fn flutter_layout_writes_scale_dirs() {
        let dir = tmpdir("flutter_layout");
        let input = dir.join("foo.png");
        let outdir = dir.join("out");
        write_png(&input, 32, 32);

        let opts = Options {
            base_scale: 4.0,
            target_scales: vec![1.0, 2.0],
            naming: NamingMode::Flutter,
            ..Default::default()
        };
        let report = process(&input, &outdir, &opts).unwrap();
        assert_eq!(report.variants.len(), 2);
        assert_eq!(report.variants[0], outdir.join("1x").join("foo.png"));
        assert_eq!(report.variants[1], outdir.join("2x").join("foo.png"));
        for v in &report.variants {
            assert!(v.exists(), "variant missing: {}", v.display());
        }
    }

    #[test]
    fn suffix_layout_drops_1x_suffix() {
        let dir = tmpdir("suffix_layout");
        let input = dir.join("foo.png");
        let outdir = dir.join("out");
        write_png(&input, 32, 32);

        let opts = Options {
            base_scale: 4.0,
            target_scales: vec![1.0, 2.0, 3.0],
            naming: NamingMode::Suffix,
            ..Default::default()
        };
        let report = process(&input, &outdir, &opts).unwrap();
        assert_eq!(report.variants[0], outdir.join("foo.png"));
        assert_eq!(report.variants[1], outdir.join("foo@2x.png"));
        assert_eq!(report.variants[2], outdir.join("foo@3x.png"));
    }

    #[test]
    fn nested_layout_uses_plain_scale_dirs() {
        let dir = tmpdir("nested_layout");
        let input = dir.join("foo.png");
        let outdir = dir.join("out");
        write_png(&input, 32, 32);

        let opts = Options {
            base_scale: 4.0,
            target_scales: vec![1.0, 2.0],
            naming: NamingMode::Nested,
            ..Default::default()
        };
        let report = process(&input, &outdir, &opts).unwrap();
        assert_eq!(report.variants[0], outdir.join("1").join("foo.png"));
        assert_eq!(report.variants[1], outdir.join("2").join("foo.png"));
    }

    #[test]
    fn dimensions_scale_proportionally() {
        let dir = tmpdir("dims");
        let input = dir.join("foo.png");
        let outdir = dir.join("out");
        write_png(&input, 64, 32);

        let opts = Options {
            base_scale: 4.0,
            target_scales: vec![2.0],
            naming: NamingMode::Flutter,
            ..Default::default()
        };
        let report = process(&input, &outdir, &opts).unwrap();
        let img = image::open(&report.variants[0]).unwrap();
        // factor = 2/4 = 0.5 → 32×16
        assert_eq!(img.width(), 32);
        assert_eq!(img.height(), 16);
    }

    #[test]
    fn fractional_scale_label() {
        assert_eq!(format_scale(1.0), "1");
        assert_eq!(format_scale(1.5), "1.5");
        assert_eq!(format_scale(2.0), "2");
        assert_eq!(format_scale(1.25), "1.25");
    }

    #[test]
    fn fractional_scale_in_flutter_layout() {
        let dir = tmpdir("frac_scale");
        let input = dir.join("foo.png");
        let outdir = dir.join("out");
        write_png(&input, 32, 32);

        let opts = Options {
            base_scale: 4.0,
            target_scales: vec![1.5],
            naming: NamingMode::Flutter,
            ..Default::default()
        };
        let report = process(&input, &outdir, &opts).unwrap();
        assert_eq!(report.variants[0], outdir.join("1.5x").join("foo.png"));
    }

    #[test]
    fn invalid_base_scale_errors() {
        let dir = tmpdir("bad_base");
        let input = dir.join("foo.png");
        write_png(&input, 32, 32);
        let opts = Options {
            base_scale: 0.0,
            ..Default::default()
        };
        let err = process(&input, &dir, &opts).unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)));
    }

    #[test]
    fn invalid_target_scale_errors() {
        let dir = tmpdir("bad_target");
        let input = dir.join("foo.png");
        write_png(&input, 32, 32);
        let opts = Options {
            target_scales: vec![1.0, -2.0],
            ..Default::default()
        };
        let err = process(&input, &dir, &opts).unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)));
    }
}
