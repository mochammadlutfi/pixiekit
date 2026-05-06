//! Trim transparent (or solid-color) borders, optionally pad uniform px,
//! optionally force a square output.
//!
//! Pipeline:
//!
//! 1. Decode the input as RGBA8.
//! 2. Compute the content bounding box. By default content is "any pixel with
//!    `alpha > alpha_threshold`". When [`Options::bg_color`] is set, content is
//!    instead "any pixel whose RGB Euclidean distance from `bg_color` exceeds
//!    `bg_tolerance × MAX_RGB_DIST`" (mirrors the [`bg_remove`] chroma-key
//!    distance constant).
//! 3. Crop the input to the bbox.
//! 4. Pad uniformly by `padding` pixels on every side (transparent fill).
//! 5. If `keep_square`, pad the shorter dimension symmetrically so the result
//!    is a square.
//!
//! [`bg_remove`]: crate::bg_remove

use std::path::Path;

use image::{GenericImage, Rgba, RgbaImage};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Maximum Euclidean distance in 8-bit RGB space.
/// `sqrt(255² + 255² + 255²) ≈ 441.673`. Mirrors `bg_remove::MAX_RGB_DIST`.
const MAX_RGB_DIST: f32 = 441.672_96;

/// Trim & pad options. See module docs for the algorithm.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Options {
    /// Alpha value above which a pixel counts as "content" (alpha-based mode).
    /// Default `1`.
    pub alpha_threshold: u8,

    /// Pixels to pad on every side after trimming. Default `0`.
    pub padding: u16,

    /// When true, pad the shorter side symmetrically to match the longer one.
    /// Default `false`.
    pub keep_square: bool,

    /// Optional background colour for non-alpha trimming. When `Some`, content
    /// is detected by RGB distance instead of alpha threshold.
    pub bg_color: Option<[u8; 3]>,

    /// Tolerance fraction (0.0..=1.0) of `MAX_RGB_DIST` used with `bg_color`.
    /// Default `0.05` (~22 in 8-bit RGB).
    pub bg_tolerance: f32,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            alpha_threshold: 1,
            padding: 0,
            keep_square: false,
            bg_color: None,
            bg_tolerance: 0.05,
        }
    }
}

/// Report describing one trim+pad pass.
#[derive(Debug, Clone, Serialize)]
pub struct TrimReport {
    /// Original `(width, height)` of the input image.
    pub input_size: (u32, u32),

    /// Final `(width, height)` after trim, pad, and optional squaring.
    pub output_size: (u32, u32),

    /// Content bbox in input image coordinates `(x, y, width, height)`.
    pub bbox: (u32, u32, u32, u32),
}

/// Trim & pad a single image file.
///
/// Reads `input`, computes content bbox, crops, pads, optionally squares, and
/// writes to `output`. The output extension determines the encoding (PNG by
/// default if missing — `image` infers from path).
///
/// # Errors
///
/// - [`Error::NotFound`] if `input` does not exist.
/// - [`Error::InvalidInput`] if the image has zero content (fully transparent
///   under the configured threshold).
/// - [`Error::Image`] / [`Error::Io`] on decode/encode/write failures.
pub fn process(input: &Path, output: &Path, opts: &Options) -> Result<TrimReport> {
    if !input.exists() {
        return Err(Error::NotFound(input.to_path_buf()));
    }

    let img = image::open(input)?.into_rgba8();
    let (in_w, in_h) = img.dimensions();

    let bbox = content_bbox(&img, opts).ok_or_else(|| {
        Error::InvalidInput(format!(
            "No content pixels found in {} (image is empty under the configured threshold)",
            input.display()
        ))
    })?;

    let cropped = crop(&img, bbox);
    let padded = pad(&cropped, opts.padding);
    let final_img = if opts.keep_square {
        square(&padded)
    } else {
        padded
    };

    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    final_img.save(output)?;

    Ok(TrimReport {
        input_size: (in_w, in_h),
        output_size: final_img.dimensions(),
        bbox,
    })
}

/// Compute the content bounding box: `(x, y, width, height)`.
///
/// Returns `None` if no pixels qualify as content under the configured rule.
fn content_bbox(img: &RgbaImage, opts: &Options) -> Option<(u32, u32, u32, u32)> {
    let (w, h) = img.dimensions();
    let mut min_x = u32::MAX;
    let mut min_y = u32::MAX;
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    let mut found = false;

    let bg_threshold_sq = opts
        .bg_color
        .map(|_| (opts.bg_tolerance.clamp(0.0, 1.0) * MAX_RGB_DIST).powi(2));

    for y in 0..h {
        for x in 0..w {
            let pixel = img.get_pixel(x, y);
            let is_content = match (opts.bg_color, bg_threshold_sq) {
                (Some([br, bg_, bb]), Some(thresh_sq)) => {
                    let dr = pixel[0] as f32 - br as f32;
                    let dg = pixel[1] as f32 - bg_ as f32;
                    let db = pixel[2] as f32 - bb as f32;
                    let dist_sq = dr * dr + dg * dg + db * db;
                    dist_sq > thresh_sq
                }
                _ => pixel[3] > opts.alpha_threshold,
            };
            if is_content {
                if !found {
                    min_x = x;
                    min_y = y;
                    max_x = x;
                    max_y = y;
                    found = true;
                } else {
                    if x < min_x {
                        min_x = x;
                    }
                    if x > max_x {
                        max_x = x;
                    }
                    if y < min_y {
                        min_y = y;
                    }
                    if y > max_y {
                        max_y = y;
                    }
                }
            }
        }
    }

    if !found {
        return None;
    }
    Some((min_x, min_y, max_x - min_x + 1, max_y - min_y + 1))
}

/// Sub-image crop returning an owned [`RgbaImage`].
fn crop(img: &RgbaImage, bbox: (u32, u32, u32, u32)) -> RgbaImage {
    let (x, y, w, h) = bbox;
    let mut out = RgbaImage::new(w, h);
    for dy in 0..h {
        for dx in 0..w {
            let pixel = img.get_pixel(x + dx, y + dy);
            out.put_pixel(dx, dy, *pixel);
        }
    }
    out
}

/// Pad `padding` transparent pixels on every side.
fn pad(img: &RgbaImage, padding: u16) -> RgbaImage {
    if padding == 0 {
        return img.clone();
    }
    let (w, h) = img.dimensions();
    let pad = padding as u32;
    let new_w = w + 2 * pad;
    let new_h = h + 2 * pad;
    let mut out = RgbaImage::from_pixel(new_w, new_h, Rgba([0, 0, 0, 0]));
    out.copy_from(img, pad, pad)
        .expect("padded canvas always large enough for source image");
    out
}

/// Pad shorter dimension symmetrically (transparent) to match the longer one.
fn square(img: &RgbaImage) -> RgbaImage {
    let (w, h) = img.dimensions();
    if w == h {
        return img.clone();
    }
    let side = w.max(h);
    let off_x = (side - w) / 2;
    let off_y = (side - h) / 2;
    let mut out = RgbaImage::from_pixel(side, side, Rgba([0, 0, 0, 0]));
    out.copy_from(img, off_x, off_y)
        .expect("square canvas always large enough for source image");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmpdir(test_name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "pixiekit-trim-test-{}-{}",
            std::process::id(),
            test_name
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Build an RGBA image with a transparent border of `border` px on every
    /// side and an opaque red `inner_w × inner_h` block inside.
    fn alpha_bordered(border: u32, inner_w: u32, inner_h: u32) -> RgbaImage {
        let w = inner_w + 2 * border;
        let h = inner_h + 2 * border;
        let mut img = RgbaImage::from_pixel(w, h, Rgba([0, 0, 0, 0]));
        for y in border..border + inner_h {
            for x in border..border + inner_w {
                img.put_pixel(x, y, Rgba([255, 0, 0, 255]));
            }
        }
        img
    }

    #[test]
    fn options_defaults() {
        let opts = Options::default();
        assert_eq!(opts.alpha_threshold, 1);
        assert_eq!(opts.padding, 0);
        assert!(!opts.keep_square);
        assert!(opts.bg_color.is_none());
        assert!((opts.bg_tolerance - 0.05).abs() < f32::EPSILON);
    }

    #[test]
    fn bbox_alpha_finds_inner_block() {
        let img = alpha_bordered(3, 4, 6);
        let bbox = content_bbox(&img, &Options::default()).unwrap();
        assert_eq!(bbox, (3, 3, 4, 6));
    }

    #[test]
    fn bbox_returns_none_for_fully_transparent() {
        let img = RgbaImage::from_pixel(8, 8, Rgba([0, 0, 0, 0]));
        let bbox = content_bbox(&img, &Options::default());
        assert!(bbox.is_none());
    }

    #[test]
    fn bbox_bg_color_detects_red_inside_green_border() {
        let mut img = RgbaImage::from_pixel(10, 10, Rgba([0, 255, 0, 255]));
        // place a red block in (2..6, 3..7)
        for y in 3..7 {
            for x in 2..6 {
                img.put_pixel(x, y, Rgba([255, 0, 0, 255]));
            }
        }
        let opts = Options {
            bg_color: Some([0, 255, 0]),
            bg_tolerance: 0.05,
            ..Default::default()
        };
        let bbox = content_bbox(&img, &opts).unwrap();
        assert_eq!(bbox, (2, 3, 4, 4));
    }

    #[test]
    fn bbox_bg_color_high_tolerance_rejects_everything() {
        // tolerance 1.0 means no pixel can be far enough from bg to count.
        let mut img = RgbaImage::from_pixel(4, 4, Rgba([0, 255, 0, 255]));
        img.put_pixel(1, 1, Rgba([255, 0, 0, 255]));
        let opts = Options {
            bg_color: Some([0, 255, 0]),
            bg_tolerance: 1.0,
            ..Default::default()
        };
        assert!(content_bbox(&img, &opts).is_none());
    }

    #[test]
    fn pad_adds_n_pixels_each_side() {
        let img = RgbaImage::from_pixel(4, 6, Rgba([255, 0, 0, 255]));
        let padded = pad(&img, 2);
        assert_eq!(padded.dimensions(), (8, 10));
        // corners must be transparent, center must be red.
        assert_eq!(padded.get_pixel(0, 0)[3], 0);
        assert_eq!(padded.get_pixel(4, 4), &Rgba([255, 0, 0, 255]));
    }

    #[test]
    fn square_pads_shorter_dim() {
        let img = RgbaImage::from_pixel(4, 8, Rgba([255, 0, 0, 255]));
        let sq = square(&img);
        assert_eq!(sq.dimensions(), (8, 8));
        // center column has the red strip
        assert_eq!(sq.get_pixel(4, 0)[3], 255);
        // left edge is padding
        assert_eq!(sq.get_pixel(0, 0)[3], 0);
    }

    #[test]
    fn square_noop_when_already_square() {
        let img = RgbaImage::from_pixel(5, 5, Rgba([0, 0, 0, 255]));
        let sq = square(&img);
        assert_eq!(sq.dimensions(), (5, 5));
    }

    #[test]
    fn process_trims_alpha_border() {
        let dir = tmpdir("process_trims_alpha_border");
        let input = dir.join("in.png");
        let output = dir.join("out.png");
        alpha_bordered(5, 8, 6).save(&input).unwrap();

        let report = process(&input, &output, &Options::default()).unwrap();
        assert_eq!(report.input_size, (18, 16));
        assert_eq!(report.output_size, (8, 6));
        assert_eq!(report.bbox, (5, 5, 8, 6));
        assert!(output.exists());
    }

    #[test]
    fn process_pads_uniformly() {
        let dir = tmpdir("process_pads_uniformly");
        let input = dir.join("in.png");
        let output = dir.join("out.png");
        alpha_bordered(2, 4, 6).save(&input).unwrap();

        let opts = Options {
            padding: 3,
            ..Default::default()
        };
        let report = process(&input, &output, &opts).unwrap();
        assert_eq!(report.bbox, (2, 2, 4, 6));
        assert_eq!(report.output_size, (4 + 6, 6 + 6)); // (10, 12)
    }

    #[test]
    fn process_keep_square() {
        let dir = tmpdir("process_keep_square");
        let input = dir.join("in.png");
        let output = dir.join("out.png");
        alpha_bordered(0, 4, 8).save(&input).unwrap();

        let opts = Options {
            keep_square: true,
            ..Default::default()
        };
        let report = process(&input, &output, &opts).unwrap();
        assert_eq!(report.output_size, (8, 8));
    }

    #[test]
    fn process_errors_on_missing_input() {
        let dir = tmpdir("process_errors_on_missing_input");
        let input = dir.join("nope.png");
        let output = dir.join("out.png");
        let result = process(&input, &output, &Options::default());
        assert!(matches!(result, Err(Error::NotFound(_))));
    }

    #[test]
    fn process_errors_on_empty_image() {
        let dir = tmpdir("process_errors_on_empty_image");
        let input = dir.join("empty.png");
        let output = dir.join("out.png");
        RgbaImage::from_pixel(8, 8, Rgba([0, 0, 0, 0]))
            .save(&input)
            .unwrap();
        let result = process(&input, &output, &Options::default());
        assert!(matches!(result, Err(Error::InvalidInput(_))));
    }
}
