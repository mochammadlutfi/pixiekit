//! Background removal — chroma key + despill + alpha erode.
//!
//! Algorithm matches the reference Python script `clean-bg.py` (ImageMagick
//! pipeline) on a per-pixel basis:
//!
//! 1. **Chroma key** — Euclidean RGB distance from `target_color`. Pixels within
//!    `fuzz × max_dist` are made transparent.
//! 2. **Despill** — clamp green channel to `min(g, max(r, b))` to remove green
//!    halo around character edges.
//! 3. **Erode** — Diamond:1 morphology on the alpha channel, N iterations.
//!    Center + 4 cardinal neighbors, take min.
//!
//! `max_dist = sqrt(255² × 3) ≈ 441.67` for 8-bit RGB, matching ImageMagick's
//! `-fuzz` semantics.

use image::RgbaImage;
use serde::{Deserialize, Serialize};

/// Maximum Euclidean distance in 8-bit RGB space.
/// `sqrt(255² + 255² + 255²) ≈ 441.673`.
const MAX_RGB_DIST: f32 = 441.672_96;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Options {
    /// Target background RGB (default pure green `[0, 255, 0]`).
    pub target_color: [u8; 3],

    /// Fuzz threshold as fraction of max RGB distance (0.0 - 1.0). Default 0.35.
    pub fuzz: f32,

    /// Apply despill pass (clamp green channel). Default true.
    pub despill: bool,

    /// Number of erode iterations on alpha (Diamond:1). Default 1, max 5.
    pub erode: u8,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            target_color: [0, 255, 0],
            fuzz: 0.35,
            despill: true,
            erode: 1,
        }
    }
}

/// Process an image with the BG-remove pipeline. Returns a new image; input is
/// not mutated.
pub fn process(img: &RgbaImage, opts: &Options) -> RgbaImage {
    let mut out = img.clone();
    chroma_key(&mut out, opts);
    if opts.despill {
        despill(&mut out);
    }
    let iters = opts.erode.min(5);
    for _ in 0..iters {
        erode_alpha(&mut out);
    }
    out
}

fn chroma_key(img: &mut RgbaImage, opts: &Options) {
    let threshold = opts.fuzz.clamp(0.0, 1.0) * MAX_RGB_DIST;
    let threshold_sq = threshold * threshold;
    let [tr, tg, tb] = opts.target_color;
    let (tr, tg, tb) = (tr as f32, tg as f32, tb as f32);

    for pixel in img.pixels_mut() {
        let dr = pixel[0] as f32 - tr;
        let dg = pixel[1] as f32 - tg;
        let db = pixel[2] as f32 - tb;
        let dist_sq = dr * dr + dg * dg + db * db;
        if dist_sq <= threshold_sq {
            pixel[3] = 0;
        }
    }
}

fn despill(img: &mut RgbaImage) {
    for pixel in img.pixels_mut() {
        if pixel[3] == 0 {
            continue;
        }
        let max_rb = pixel[0].max(pixel[2]);
        if pixel[1] > max_rb {
            pixel[1] = max_rb;
        }
    }
}

fn erode_alpha(img: &mut RgbaImage) {
    let (w, h) = img.dimensions();
    let total = (w as usize) * (h as usize);
    let mut alpha = vec![0u8; total];

    // Snapshot alpha channel
    for (i, pixel) in img.pixels().enumerate() {
        alpha[i] = pixel[3];
    }

    // Diamond:1 — center + 4 cardinal neighbors, take min
    let w_us = w as usize;
    for y in 0..h as usize {
        for x in 0..w as usize {
            let idx = y * w_us + x;
            let mut min_a = alpha[idx];
            if y > 0 {
                min_a = min_a.min(alpha[idx - w_us]);
            }
            if y + 1 < h as usize {
                min_a = min_a.min(alpha[idx + w_us]);
            }
            if x > 0 {
                min_a = min_a.min(alpha[idx - 1]);
            }
            if x + 1 < w as usize {
                min_a = min_a.min(alpha[idx + 1]);
            }
            img.get_pixel_mut(x as u32, y as u32)[3] = min_a;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn solid_image(w: u32, h: u32, color: [u8; 4]) -> RgbaImage {
        let mut img = RgbaImage::new(w, h);
        for pixel in img.pixels_mut() {
            *pixel = Rgba(color);
        }
        img
    }

    #[test]
    fn pure_green_becomes_transparent_with_default_opts() {
        let img = solid_image(4, 4, [0, 255, 0, 255]);
        let out = process(&img, &Options::default());
        for pixel in out.pixels() {
            assert_eq!(pixel[3], 0, "expected fully transparent, got {:?}", pixel);
        }
    }

    #[test]
    fn pure_red_stays_opaque() {
        let img = solid_image(4, 4, [255, 0, 0, 255]);
        let out = process(&img, &Options::default());
        for pixel in out.pixels() {
            assert_eq!(pixel[3], 255);
        }
    }

    #[test]
    fn fuzz_zero_only_exact_match_transparent() {
        let img = solid_image(4, 4, [10, 240, 10, 255]);
        let opts = Options {
            fuzz: 0.0,
            ..Default::default()
        };
        let out = process(&img, &opts);
        // 10,240,10 is not exactly 0,255,0 — distance ≈ 22.6 > 0
        for pixel in out.pixels() {
            assert_eq!(pixel[3], 255);
        }
    }

    #[test]
    fn fuzz_high_kills_anything_remotely_green() {
        let img = solid_image(4, 4, [50, 200, 50, 255]);
        let opts = Options {
            fuzz: 0.5,
            ..Default::default()
        };
        let out = process(&img, &opts);
        for pixel in out.pixels() {
            assert_eq!(pixel[3], 0);
        }
    }

    #[test]
    fn despill_clamps_green_above_max_rb() {
        // Pixel: r=100, g=200, b=50 → max(r,b)=100 → g should clamp to 100
        let img = solid_image(2, 2, [100, 200, 50, 255]);
        let opts = Options {
            fuzz: 0.0, // no chroma key
            despill: true,
            erode: 0,
            ..Default::default()
        };
        let out = process(&img, &opts);
        for pixel in out.pixels() {
            assert_eq!(pixel[1], 100, "green should clamp to max(r,b)=100");
        }
    }

    #[test]
    fn despill_skips_when_green_below_max_rb() {
        // Pixel: r=200, g=50, b=100 → max(r,b)=200, g=50 < 200 → no change
        let img = solid_image(2, 2, [200, 50, 100, 255]);
        let opts = Options {
            fuzz: 0.0,
            despill: true,
            erode: 0,
            ..Default::default()
        };
        let out = process(&img, &opts);
        for pixel in out.pixels() {
            assert_eq!(pixel[1], 50);
        }
    }

    #[test]
    fn erode_shrinks_alpha_at_edge() {
        // 3x3 image: center opaque, edges transparent
        let mut img = RgbaImage::new(3, 3);
        for y in 0..3 {
            for x in 0..3 {
                let alpha = if x == 1 && y == 1 { 255 } else { 0 };
                img.put_pixel(x, y, Rgba([255, 0, 0, alpha]));
            }
        }
        // 1 erode iteration → center pixel should become 0 (neighbors are 0)
        let opts = Options {
            fuzz: 0.0,
            despill: false,
            erode: 1,
            ..Default::default()
        };
        let out = process(&img, &opts);
        assert_eq!(
            out.get_pixel(1, 1)[3],
            0,
            "center should be eroded by neighbors"
        );
    }

    #[test]
    fn erode_zero_iterations_preserves_alpha() {
        let img = solid_image(3, 3, [255, 0, 0, 200]);
        let opts = Options {
            fuzz: 0.0,
            despill: false,
            erode: 0,
            ..Default::default()
        };
        let out = process(&img, &opts);
        assert!(out.pixels().all(|p| p[3] == 200));
    }

    #[test]
    fn erode_clamped_to_max_5() {
        // erode > 5 should be clamped, not panic
        let img = solid_image(10, 10, [255, 0, 0, 255]);
        let opts = Options {
            fuzz: 0.0,
            despill: false,
            erode: 100,
            ..Default::default()
        };
        let _out = process(&img, &opts); // should not panic
    }

    #[test]
    fn process_does_not_mutate_input() {
        let img = solid_image(4, 4, [0, 255, 0, 255]);
        let copy = img.clone();
        let _out = process(&img, &Options::default());
        // input image should remain unchanged
        for (a, b) in img.pixels().zip(copy.pixels()) {
            assert_eq!(a, b);
        }
    }
}
