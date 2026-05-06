//! 9-Slice Slicer — splits an image into 9 tiles or generates Flame-compatible metadata.
//!
//! Useful for UI elements that need to be stretched while preserving corner dimensions.
//! Supports two modes:
//! - `Metadata`: Writes a JSON sibling detailing insets (Flame `NineTileBoxComponent` compatible).
//! - `Split`: Physically crops the image into 9 separate PNG files.

use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use image::{GenericImageView, ImageBuffer, Rgba};

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputMode {
    Split,
    #[default]
    Metadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Options {
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
    pub left: u32,
    pub output_mode: OutputMode,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            top: 0,
            right: 0,
            bottom: 0,
            left: 0,
            output_mode: OutputMode::Metadata,
        }
    }
}

/// Flame-compatible nine-slice metadata (PRD §6.10.2).
///
/// JSON shape:
/// ```json
/// { "image": "button.png",
///   "size": {"w": 256, "h": 96},
///   "slices": {"top": 16, "right": 32, "bottom": 16, "left": 32} }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NineSliceMetadata {
    pub image: String,
    pub size: NineSliceSize,
    pub slices: NineSliceInsets,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct NineSliceSize {
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct NineSliceInsets {
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
    pub left: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NineSliceReport {
    pub mode: OutputMode,
    pub output_files: Vec<PathBuf>,
    pub image_size: (u32, u32),
}

/// Process a single image for 9-slice slicing.
pub fn process(input: &Path, output_dir: &Path, opts: &Options) -> Result<NineSliceReport> {
    if !input.exists() {
        return Err(Error::NotFound(input.to_path_buf()));
    }

    let img = image::open(input).map_err(Error::Image)?;
    let (width, height) = img.dimensions();

    // Inset validation
    if opts.top + opts.bottom >= height {
        return Err(Error::InvalidInput(format!(
            "Total vertical insets ({} + {}) exceed image height ({})",
            opts.top, opts.bottom, height
        )));
    }
    if opts.left + opts.right >= width {
        return Err(Error::InvalidInput(format!(
            "Total horizontal insets ({} + {}) exceed image width ({})",
            opts.left, opts.right, width
        )));
    }

    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| Error::InvalidInput("Invalid input filename".into()))?;

    let mut output_files = Vec::new();

    match opts.output_mode {
        OutputMode::Metadata => {
            let metadata = NineSliceMetadata {
                image: input.file_name().unwrap().to_string_lossy().into_owned(),
                size: NineSliceSize { w: width, h: height },
                slices: NineSliceInsets {
                    top: opts.top,
                    right: opts.right,
                    bottom: opts.bottom,
                    left: opts.left,
                },
            };

            let json_path = output_dir.join(format!("{}.9slice.json", stem));
            let f = std::fs::File::create(&json_path)?;
            serde_json::to_writer_pretty(f, &metadata)?;
            output_files.push(json_path);

            // Also copy the original image to output_dir if it's different
            let img_output_path = output_dir.join(input.file_name().unwrap());
            if input != img_output_path {
                std::fs::copy(input, &img_output_path)?;
                output_files.push(img_output_path);
            }
        }
        OutputMode::Split => {
            let slices = split_image(&img, opts)?;
            let names = [
                "top_left", "top", "top_right",
                "left", "center", "right",
                "bottom_left", "bottom", "bottom_right",
            ];

            for (i, slice) in slices.into_iter().enumerate() {
                let path = output_dir.join(format!("{}_{}.png", stem, names[i]));
                slice.save(&path).map_err(Error::Image)?;
                output_files.push(path);
            }
        }
    }

    Ok(NineSliceReport {
        mode: opts.output_mode,
        output_files,
        image_size: (width, height),
    })
}

fn split_image(
    img: &image::DynamicImage,
    opts: &Options,
) -> Result<Vec<ImageBuffer<Rgba<u8>, Vec<u8>>>> {
    let (w, h) = img.dimensions();
    let t = opts.top;
    let b = opts.bottom;
    let l = opts.left;
    let r = opts.right;

    let mid_w = w - l - r;
    let mid_h = h - t - b;

    let mut slices = Vec::with_capacity(9);

    // Coordinates: (x, y, width, height)
    let regions = [
        (0, 0, l, t),             // top_left
        (l, 0, mid_w, t),         // top
        (w - r, 0, r, t),         // top_right
        (0, t, l, mid_h),         // left
        (l, t, mid_w, mid_h),     // center
        (w - r, t, r, mid_h),     // right
        (0, h - b, l, b),         // bottom_left
        (l, h - b, mid_w, b),     // bottom
        (w - r, h - b, r, b),     // bottom_right
    ];

    for (rx, ry, rw, rh) in regions {
        let slice = img.view(rx, ry, rw, rh).to_image();
        slices.push(slice);
    }

    Ok(slices)
}

#[cfg(test)]
mod tests {
    use super::*;


    fn create_test_image(width: u32, height: u32) -> PathBuf {
        let dir = std::env::temp_dir().join("pixiekit-nine-slice-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("test_{}x{}.png", width, height));
        let img = ImageBuffer::from_pixel(width, height, Rgba([255u8, 0, 0, 255]));
        img.save(&path).unwrap();
        path
    }

    #[test]
    fn validates_insets() {
        let path = create_test_image(100, 100);
        let out_dir = path.parent().unwrap();

        // Valid
        let opts = Options {
            top: 10,
            bottom: 10,
            left: 10,
            right: 10,
            output_mode: OutputMode::Metadata,
        };
        assert!(process(&path, out_dir, &opts).is_ok());

        // Invalid height
        let opts = Options {
            top: 60,
            bottom: 60,
            ..opts
        };
        assert!(process(&path, out_dir, &opts).is_err());

        // Invalid width
        let opts = Options {
            left: 60,
            right: 60,
            ..opts
        };
        assert!(process(&path, out_dir, &opts).is_err());
    }

    #[test]
    fn metadata_mode_works() {
        let path = create_test_image(100, 100);
        let out_dir = path.parent().unwrap().join("output");
        std::fs::create_dir_all(&out_dir).unwrap();

        let opts = Options {
            top: 10,
            bottom: 20,
            left: 30,
            right: 40,
            output_mode: OutputMode::Metadata,
        };

        let report = process(&path, &out_dir, &opts).unwrap();
        assert_eq!(report.output_files.len(), 2); // json + image copy

        let json_path = out_dir.join("test_100x100.9slice.json");
        assert!(json_path.exists());

        let json_content = std::fs::read_to_string(json_path).unwrap();
        let metadata: NineSliceMetadata = serde_json::from_str(&json_content).unwrap();
        assert_eq!(metadata.slices.top, 10);
        assert_eq!(metadata.slices.bottom, 20);
        assert_eq!(metadata.slices.left, 30);
        assert_eq!(metadata.slices.right, 40);
        assert_eq!(metadata.size.w, 100);
        assert_eq!(metadata.size.h, 100);
    }

    #[test]
    fn split_mode_works() {
        let path = create_test_image(100, 100);
        let out_dir = path.parent().unwrap();
        let opts = Options {
            top: 25,
            bottom: 25,
            left: 25,
            right: 25,
            output_mode: OutputMode::Split,
        };

        let report = process(&path, out_dir, &opts).unwrap();
        assert_eq!(report.output_files.len(), 9);

        for p in report.output_files {
            let img = image::open(&p).unwrap();
            let name = p.file_name().unwrap().to_str().unwrap();
            
            if name.contains("top_left") || name.contains("top_right") || name.contains("bottom_left") || name.contains("bottom_right") {
                assert_eq!(img.width(), 25);
                assert_eq!(img.height(), 25);
            } else if name.contains("top") || name.contains("bottom") {
                assert_eq!(img.width(), 50); // 100 - 25 - 25
                assert_eq!(img.height(), 25);
            } else if name.contains("left") || name.contains("right") {
                assert_eq!(img.width(), 25);
                assert_eq!(img.height(), 50);
            } else if name.contains("center") {
                assert_eq!(img.width(), 50);
                assert_eq!(img.height(), 50);
            }
        }
    }
}
