//! Raster → SVG vectorization via [`vtracer`].
//!
//! Wraps the [`vtracer`](https://crates.io/crates/vtracer) crate (MIT/Apache),
//! which itself wraps `visioncortex` for path tracing. Pipeline stages handled
//! by vtracer:
//!
//! 1. Color quantization (palette reduction)
//! 2. Pixel grouping (connected component clustering)
//! 3. Path tracing (polygon outline per cluster)
//! 4. Curve fitting (smooth Bezier curves)
//! 5. SVG generation
//!
//! This module exposes a stable [`Options`] surface that mirrors PRD §6.2.4 and
//! a [`process`] function that vectorizes a single image to an `.svg` file. It
//! does not own batch orchestration — frontends compose [`process`] with
//! [`crate::batch::list_images`] and rayon for parallelism.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Color mode for vectorization output.
///
/// Mirrors `vtracer::ColorMode` but lives in `core` so frontends do not need a
/// direct vtracer dependency to construct [`Options`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Preserve color palette (recommended for character art / illustrations).
    #[default]
    Color,
    /// Binary black & white (recommended for line art / silhouettes).
    Binary,
}

/// Vectorize options. Defaults match vtracer's documented defaults
/// (PRD §6.2.4) so a bare `Options::default()` produces a balanced cartoon
/// trace.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Options {
    pub mode: Mode,
    /// Discard speckle clusters smaller than this (px²). 0 - 128.
    pub filter_speckle: u32,
    /// Color quantization: bits per channel. 1 - 8.
    pub color_precision: u8,
    /// Min color difference between layers. 0 - 128.
    pub layer_difference: u8,
    /// Corner detection angle threshold (degrees). 0 - 180.
    pub corner_threshold: u8,
    /// Min segment length (px). 0.0 - 10.0.
    pub length_threshold: f64,
    /// Splice angle threshold (degrees). 0 - 180.
    pub splice_threshold: u8,
    /// Decimal places for SVG path coordinates. 0 - 16.
    pub path_precision: u8,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            mode: Mode::Color,
            filter_speckle: 4,
            color_precision: 6,
            layer_difference: 16,
            corner_threshold: 60,
            length_threshold: 4.0,
            splice_threshold: 45,
            path_precision: 8,
        }
    }
}

/// Map a 0-10 simple "smoothness" slider to advanced parameters
/// `(corner_threshold, length_threshold, splice_threshold)`.
///
/// Per PRD §14.4 cheat sheet:
///
/// | Slider | corner | length | splice |
/// |:------:|:------:|:------:|:------:|
/// | 0      | 30     | 1.0    | 20     |
/// | 4      | 60     | 4.0    | 45     |
/// | 7      | 120    | 8.0    | 90     |
/// | 10     | 180    | 10.0   | 180    |
///
/// Values between table entries are linearly interpolated. Values >10 saturate
/// at 10.
pub fn smooth_to_params(smooth: u8) -> (u8, f64, u8) {
    // Anchor points from PRD §14.4
    const POINTS: &[(u8, f64, f64, f64)] = &[
        (0, 30.0, 1.0, 20.0),
        (4, 60.0, 4.0, 45.0),
        (7, 120.0, 8.0, 90.0),
        (10, 180.0, 10.0, 180.0),
    ];

    let s = smooth.min(10);

    // Find bracketing anchor points
    let (lo, hi) = POINTS
        .windows(2)
        .find(|w| s >= w[0].0 && s <= w[1].0)
        .map(|w| (w[0], w[1]))
        // Saturation: s == 10 hits last point exactly; fall back to last pair.
        .unwrap_or((POINTS[POINTS.len() - 2], POINTS[POINTS.len() - 1]));

    if lo.0 == hi.0 {
        return (lo.1 as u8, lo.2, lo.3 as u8);
    }

    let t = (s - lo.0) as f64 / (hi.0 - lo.0) as f64;
    let corner = lo.1 + t * (hi.1 - lo.1);
    let length = lo.2 + t * (hi.2 - lo.2);
    let splice = lo.3 + t * (hi.3 - lo.3);

    (corner.round() as u8, length, splice.round() as u8)
}

/// Vectorize a single image file to SVG.
///
/// Reads `input_path`, traces it via vtracer, and writes the resulting SVG to
/// `output_path`. The caller is responsible for ensuring the output directory
/// exists.
///
/// # Errors
///
/// - [`Error::NotFound`] if `input_path` does not exist.
/// - [`Error::VtracerFailed`] if vtracer reports a tracing/encoding failure
///   (e.g., unsupported image format, IO error writing SVG).
pub fn process(input_path: &Path, output_path: &Path, opts: &Options) -> Result<()> {
    if !input_path.exists() {
        return Err(Error::NotFound(input_path.to_path_buf()));
    }

    let config = vtracer::Config {
        color_mode: match opts.mode {
            Mode::Color => vtracer::ColorMode::Color,
            Mode::Binary => vtracer::ColorMode::Binary,
        },
        // Stacked is vtracer's default and matches PRD expectations.
        hierarchical: vtracer::Hierarchical::Stacked,
        filter_speckle: opts.filter_speckle as usize,
        color_precision: opts.color_precision.clamp(1, 8) as i32,
        layer_difference: opts.layer_difference as i32,
        // Spline is vtracer's default; produces Bezier curves.
        mode: visioncortex::PathSimplifyMode::Spline,
        corner_threshold: opts.corner_threshold.clamp(0, 180) as i32,
        length_threshold: opts.length_threshold.clamp(0.0, 10.0),
        max_iterations: 10,
        splice_threshold: opts.splice_threshold.clamp(0, 180) as i32,
        path_precision: Some(opts.path_precision.clamp(0, 16) as u32),
    };

    vtracer::convert_image_to_svg(input_path, output_path, config).map_err(Error::VtracerFailed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};
    use std::path::PathBuf;

    fn tmpdir(test_name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "pixiekit-vec-test-{}-{}",
            std::process::id(),
            test_name
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn mode_default_is_color() {
        assert_eq!(Mode::default(), Mode::Color);
    }

    #[test]
    fn options_default_values() {
        let opts = Options::default();
        assert_eq!(opts.mode, Mode::Color);
        assert_eq!(opts.filter_speckle, 4);
        assert_eq!(opts.color_precision, 6);
        assert_eq!(opts.layer_difference, 16);
        assert_eq!(opts.corner_threshold, 60);
        assert_eq!(opts.length_threshold, 4.0);
        assert_eq!(opts.splice_threshold, 45);
        assert_eq!(opts.path_precision, 8);
    }

    #[test]
    fn smooth_zero_is_sharp() {
        let (c, l, s) = smooth_to_params(0);
        assert_eq!(c, 30);
        assert_eq!(l, 1.0);
        assert_eq!(s, 20);
    }

    #[test]
    fn smooth_four_matches_default() {
        // Sanity: slider at 4 should reproduce the default Options values.
        let (c, l, s) = smooth_to_params(4);
        let defaults = Options::default();
        assert_eq!(c, defaults.corner_threshold);
        assert_eq!(l, defaults.length_threshold);
        assert_eq!(s, defaults.splice_threshold);
    }

    #[test]
    fn smooth_middle_produces_sane_defaults() {
        // Slider at 5 should be between the 4-anchor (60, 4.0, 45) and the
        // 7-anchor (120, 8.0, 90) — strictly bracketed.
        let (c, l, s) = smooth_to_params(5);
        assert!(c > 60 && c < 120, "corner {} out of bracket", c);
        assert!(l > 4.0 && l < 8.0, "length {} out of bracket", l);
        assert!(s > 45 && s < 90, "splice {} out of bracket", s);
    }

    #[test]
    fn smooth_ten_is_max() {
        let (c, l, s) = smooth_to_params(10);
        assert_eq!(c, 180);
        assert_eq!(l, 10.0);
        assert_eq!(s, 180);
    }

    #[test]
    fn smooth_above_ten_saturates() {
        let a = smooth_to_params(10);
        let b = smooth_to_params(255);
        assert_eq!(a, b);
    }

    #[test]
    fn process_errors_on_missing_input() {
        let dir = tmpdir("missing_input");
        let input = dir.join("does-not-exist.png");
        let output = dir.join("out.svg");
        let result = process(&input, &output, &Options::default());
        assert!(matches!(result, Err(Error::NotFound(_))));
    }

    #[test]
    fn process_emits_svg_for_simple_png() {
        // Synthesise a 32×32 PNG with two colour blocks so vtracer has
        // something to trace, then assert the output starts with `<?xml`/`<svg`.
        let dir = tmpdir("emits_svg");
        let input = dir.join("in.png");
        let output = dir.join("out.svg");

        let mut img = RgbaImage::new(32, 32);
        for y in 0..32 {
            for x in 0..32 {
                let color = if x < 16 {
                    [255, 0, 0, 255]
                } else {
                    [0, 0, 255, 255]
                };
                img.put_pixel(x, y, Rgba(color));
            }
        }
        img.save(&input).unwrap();

        process(&input, &output, &Options::default()).expect("vectorize failed");

        assert!(output.exists(), "output svg not created");
        let content = std::fs::read_to_string(&output).unwrap();
        assert!(
            content.contains("<svg"),
            "expected <svg> root tag, got: {}",
            &content[..content.len().min(120)]
        );
    }
}
