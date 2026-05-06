//! Color posterization via median-cut quantization.
//!
//! Reduces an image to a small palette of N colors using the classic median-cut
//! algorithm (Heckbert, 1982). Used as a pre-processing step before
//! [`crate::vectorize`] to drastically reduce the number of distinct colors
//! vtracer must trace, which produces cleaner SVG output that more closely
//! resembles AI vectorizers like Recraft / Vectorizer.ai for cartoon and
//! illustration input.
//!
//! Pipeline:
//!
//! 1. Collect all visible (alpha > 0) pixels as RGB samples.
//! 2. Recursively split the bounding box of the sample set along its longest
//!    axis at the median, until `n_colors` buckets exist.
//! 3. Compute the average color of each bucket — these become the palette.
//! 4. Map every pixel to its nearest palette entry (Euclidean RGB distance).
//!
//! Transparent pixels (alpha == 0) are preserved unchanged. Alpha values for
//! visible pixels are also preserved — only RGB is quantized.

use image::{Rgba, RgbaImage};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Posterize options.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Options {
    /// Number of palette colors. Must be a power of two between 2 and 256.
    /// Typical values: 4 (very aggressive), 8 (clean cartoon), 16 (rich
    /// illustration), 32 (subtle).
    pub n_colors: u16,
}

impl Default for Options {
    fn default() -> Self {
        Self { n_colors: 8 }
    }
}

/// Apply median-cut posterization. Returns a new image; input is not mutated.
pub fn process(img: &RgbaImage, opts: &Options) -> Result<RgbaImage> {
    let n = opts.n_colors as usize;
    if !(2..=256).contains(&n) || !n.is_power_of_two() {
        return Err(Error::InvalidInput(format!(
            "n_colors must be a power of two between 2 and 256 (got {n})"
        )));
    }

    let samples: Vec<[u8; 3]> = img
        .pixels()
        .filter(|p| p[3] > 0)
        .map(|p| [p[0], p[1], p[2]])
        .collect();

    if samples.is_empty() {
        return Ok(img.clone());
    }

    let palette = median_cut(samples, n);
    Ok(remap(img, &palette))
}

/// Run median-cut quantization on `samples` until `n` buckets are produced.
/// Returns the average color of each bucket as the final palette.
fn median_cut(samples: Vec<[u8; 3]>, n: usize) -> Vec<[u8; 3]> {
    let mut buckets: Vec<Vec<[u8; 3]>> = vec![samples];

    while buckets.len() < n {
        // Split the bucket with the largest range on its longest axis.
        let split_idx = buckets
            .iter()
            .enumerate()
            .filter(|(_, b)| b.len() > 1)
            .max_by_key(|(_, b)| max_range(b))
            .map(|(i, _)| i);

        let Some(idx) = split_idx else {
            break; // All buckets are size 1 — cannot split further.
        };

        let bucket = buckets.swap_remove(idx);
        let axis = longest_axis(&bucket);
        let mut sorted = bucket;
        sorted.sort_unstable_by_key(|c| c[axis]);
        let mid = sorted.len() / 2;
        let right = sorted.split_off(mid);
        buckets.push(sorted);
        buckets.push(right);
    }

    buckets.into_iter().map(average).collect()
}

/// Index of the channel with the widest min-max range.
fn longest_axis(bucket: &[[u8; 3]]) -> usize {
    let mut min = [u8::MAX; 3];
    let mut max = [u8::MIN; 3];
    for &c in bucket {
        for i in 0..3 {
            min[i] = min[i].min(c[i]);
            max[i] = max[i].max(c[i]);
        }
    }
    let r = max[0] - min[0];
    let g = max[1] - min[1];
    let b = max[2] - min[2];
    if r >= g && r >= b {
        0
    } else if g >= b {
        1
    } else {
        2
    }
}

/// Largest channel range across the bucket — used to pick which bucket to
/// split next.
fn max_range(bucket: &[[u8; 3]]) -> u32 {
    let mut min = [u8::MAX; 3];
    let mut max = [u8::MIN; 3];
    for &c in bucket {
        for i in 0..3 {
            min[i] = min[i].min(c[i]);
            max[i] = max[i].max(c[i]);
        }
    }
    let r = (max[0] - min[0]) as u32;
    let g = (max[1] - min[1]) as u32;
    let b = (max[2] - min[2]) as u32;
    r.max(g).max(b)
}

/// Average RGB of a bucket. Bucket is guaranteed non-empty by the caller.
fn average(bucket: Vec<[u8; 3]>) -> [u8; 3] {
    let n = bucket.len() as u64;
    let mut sum = [0u64; 3];
    for c in &bucket {
        sum[0] += c[0] as u64;
        sum[1] += c[1] as u64;
        sum[2] += c[2] as u64;
    }
    [(sum[0] / n) as u8, (sum[1] / n) as u8, (sum[2] / n) as u8]
}

/// Replace each visible pixel with its nearest palette entry.
fn remap(img: &RgbaImage, palette: &[[u8; 3]]) -> RgbaImage {
    let mut out = img.clone();
    for pixel in out.pixels_mut() {
        if pixel[3] == 0 {
            continue;
        }
        let nearest = nearest_palette(pixel, palette);
        *pixel = Rgba([nearest[0], nearest[1], nearest[2], pixel[3]]);
    }
    out
}

fn nearest_palette(pixel: &Rgba<u8>, palette: &[[u8; 3]]) -> [u8; 3] {
    let mut best = palette[0];
    let mut best_d = u32::MAX;
    for &c in palette {
        let dr = pixel[0] as i32 - c[0] as i32;
        let dg = pixel[1] as i32 - c[1] as i32;
        let db = pixel[2] as i32 - c[2] as i32;
        let d = (dr * dr + dg * dg + db * db) as u32;
        if d < best_d {
            best_d = d;
            best = c;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, color: [u8; 4]) -> RgbaImage {
        let mut img = RgbaImage::new(w, h);
        for p in img.pixels_mut() {
            *p = Rgba(color);
        }
        img
    }

    #[test]
    fn rejects_non_power_of_two() {
        let img = solid(2, 2, [255, 0, 0, 255]);
        let result = process(&img, &Options { n_colors: 7 });
        assert!(matches!(result, Err(Error::InvalidInput(_))));
    }

    #[test]
    fn rejects_too_few_colors() {
        let img = solid(2, 2, [255, 0, 0, 255]);
        let result = process(&img, &Options { n_colors: 1 });
        assert!(matches!(result, Err(Error::InvalidInput(_))));
    }

    #[test]
    fn solid_image_unchanged() {
        let img = solid(4, 4, [200, 100, 50, 255]);
        let out = process(&img, &Options { n_colors: 8 }).unwrap();
        for p in out.pixels() {
            assert_eq!(p[0], 200);
            assert_eq!(p[1], 100);
            assert_eq!(p[2], 50);
            assert_eq!(p[3], 255);
        }
    }

    #[test]
    fn preserves_alpha_zero_pixels() {
        let mut img = RgbaImage::new(4, 4);
        for (i, p) in img.pixels_mut().enumerate() {
            *p = if i % 2 == 0 {
                Rgba([255, 0, 0, 0]) // transparent — should pass through untouched
            } else {
                Rgba([100, 200, 50, 255])
            };
        }
        let out = process(&img, &Options { n_colors: 2 }).unwrap();
        for (i, p) in out.pixels().enumerate() {
            if i % 2 == 0 {
                // Transparent pixels keep their original RGBA bytes.
                assert_eq!(*p, Rgba([255, 0, 0, 0]));
            }
        }
    }

    #[test]
    fn reduces_to_n_distinct_colors() {
        // Build a 4-color image, ask for 2 colors → expect at most 2 distinct
        // RGB values among visible pixels.
        let colors = [
            [255, 0, 0, 255],
            [250, 5, 5, 255],
            [0, 0, 255, 255],
            [5, 5, 250, 255],
        ];
        let mut img = RgbaImage::new(4, 4);
        for (i, p) in img.pixels_mut().enumerate() {
            *p = Rgba(colors[i % 4]);
        }
        let out = process(&img, &Options { n_colors: 2 }).unwrap();
        let mut distinct = std::collections::HashSet::new();
        for p in out.pixels() {
            distinct.insert([p[0], p[1], p[2]]);
        }
        assert!(
            distinct.len() <= 2,
            "expected ≤2 distinct colors, got {}",
            distinct.len()
        );
    }

    #[test]
    fn fully_transparent_image_returns_unchanged() {
        let img = solid(4, 4, [123, 45, 67, 0]);
        let out = process(&img, &Options { n_colors: 4 }).unwrap();
        assert_eq!(img, out);
    }

    #[test]
    fn longest_axis_picks_widest_channel() {
        // Red range 100, green range 10, blue range 5 → axis 0
        let bucket = vec![[0, 50, 50], [100, 60, 55]];
        assert_eq!(longest_axis(&bucket), 0);
    }
}
