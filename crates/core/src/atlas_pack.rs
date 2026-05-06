//! Sprite atlas packer — pack many PNG sprites into a single texture atlas
//! plus Flame-compatible TexturePacker JSON Hash metadata.
//!
//! Pipeline:
//!
//! 1. **Decode** — load each PNG into [`RgbaImage`]
//! 2. **Trim** (optional) — record the alpha bounding box per sprite
//! 3. **Sort** — descending by trimmed area for better packing efficiency
//! 4. **Pack** — skyline bin packing (`texture_packer` crate) into a square bin
//! 5. **Compose** — blit each sprite into the atlas; replicate edge pixels for
//!    `extrude` to prevent texture bleed at GPU sampling time
//! 6. **Encode** — PNG (lossless) or WebP (alpha lossless, color quality knob)
//! 7. **Metadata** — JSON sibling matching the TexturePacker JSON Hash schema
//!
//! See PRD §6.4 for the full specification.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use image::{GenericImageView, Rgba, RgbaImage};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Output container for the atlas image.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    #[default]
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

/// Atlas pack options. Defaults match PRD §6.4.4.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Options {
    /// Atlas basename (without extension). Default: `atlas`.
    pub name: String,
    /// Max bin dimension in pixels (256 - 8192). Default: 2048.
    pub max_size: u16,
    /// Padding between sprites in pixels (0 - 16). Default: 2.
    pub padding: u8,
    /// Edge replication width to prevent texture bleed (0 - 4). Default: 1.
    pub extrude: u8,
    /// Force the atlas dimensions to a power of two for mobile GPU friendliness.
    pub power_of_two: bool,
    /// Auto-trim transparent borders before packing.
    pub trim: bool,
    /// Output container format.
    pub format: OutputFormat,
    /// WebP color quality 0-100 (alpha is always lossless). Default: 90.
    pub webp_quality: u8,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            name: "atlas".to_string(),
            max_size: 2048,
            padding: 2,
            extrude: 1,
            power_of_two: true,
            trim: true,
            format: OutputFormat::Png,
            webp_quality: 90,
        }
    }
}

/// Per-sprite frame entry in the TexturePacker JSON Hash output.
#[derive(Debug, Clone, Serialize)]
pub struct FrameEntry {
    pub frame: FrameRect,
    pub rotated: bool,
    pub trimmed: bool,
    #[serde(rename = "spriteSourceSize")]
    pub sprite_source_size: FrameRect,
    #[serde(rename = "sourceSize")]
    pub source_size: SourceSize,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct FrameRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct SourceSize {
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct AtlasSize {
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct AtlasMeta {
    pub image: String,
    pub size: AtlasSize,
    pub format: String,
    pub scale: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AtlasJson {
    pub frames: BTreeMap<String, FrameEntry>,
    pub meta: AtlasMeta,
}

/// Result of [`process`] — paths plus packing statistics.
#[derive(Debug, Clone)]
pub struct Report {
    pub atlas_path: PathBuf,
    pub metadata_path: PathBuf,
    pub packed: u32,
    pub total: u32,
    pub atlas_size: (u32, u32),
    /// Fraction of the atlas pixel area covered by sprite content (0.0 - 1.0).
    pub efficiency: f32,
}

/// Pack a slice of sprite paths into a single atlas image plus JSON metadata.
///
/// Output filenames are derived from `opts.name`:
/// - atlas image: `<name>.<png|webp>`
/// - metadata: `<name>.json`
///
/// `output_dir` must already exist; create with `std::fs::create_dir_all` if
/// callers need that behavior.
pub fn process(sprite_paths: &[PathBuf], output_dir: &Path, opts: &Options) -> Result<Report> {
    if sprite_paths.is_empty() {
        let atlas_path = output_dir.join(format!("{}.{}", opts.name, opts.format.extension()));
        let metadata_path = output_dir.join(format!("{}.json", opts.name));
        // Empty input → no atlas written. Caller decides what to surface.
        return Ok(Report {
            atlas_path,
            metadata_path,
            packed: 0,
            total: 0,
            atlas_size: (0, 0),
            efficiency: 0.0,
        });
    }

    let max_size = (opts.max_size as u32).clamp(256, 8192);
    let padding = opts.padding.min(16) as u32;
    let extrude = opts.extrude.min(4) as u32;

    // 1. Decode + (optional) trim
    let sprites = load_sprites(sprite_paths, opts.trim, max_size)?;

    // 2. Sort by trimmed area, descending (skyline does best with biggest first)
    let mut order: Vec<usize> = (0..sprites.len()).collect();
    order.sort_by(|a, b| {
        let area_b = sprites[*b].trimmed.width() * sprites[*b].trimmed.height();
        let area_a = sprites[*a].trimmed.width() * sprites[*a].trimmed.height();
        area_b.cmp(&area_a)
    });

    // 3. Pack via texture_packer (skyline best-fit, no rotation)
    use texture_packer::{TexturePacker, TexturePackerConfig};
    let cfg = TexturePackerConfig {
        max_width: max_size,
        max_height: max_size,
        allow_rotation: false,
        force_max_dimensions: false,
        border_padding: padding,
        texture_padding: padding,
        texture_extrusion: extrude,
        // We trim beforehand so the packer just lays out trimmed bitmaps.
        trim: false,
        texture_outlines: false,
    };
    let mut packer: TexturePacker<RgbaImage, String> = TexturePacker::new_skyline(cfg);

    let mut placed: Vec<(usize, texture_packer::Rect)> = Vec::with_capacity(sprites.len());
    let mut packed = 0u32;
    for idx in &order {
        let sprite = &sprites[*idx];
        let key = sprite.name.clone();
        // `texture_packer::PackError` is not re-exported at the crate root, so
        // we match on its Debug form. Variants are stable: `TextureEmpty` and
        // `TextureTooLargeToFitIntoAtlas`.
        match packer.pack_ref(key.clone(), &sprite.trimmed) {
            Ok(()) => {
                if let Some(frame) = packer.get_frame(&key) {
                    placed.push((*idx, frame.frame));
                    packed += 1;
                }
            }
            Err(e) => {
                let msg = format!("{e:?}");
                if msg.contains("TextureEmpty") {
                    return Err(Error::AtlasPackFailed(format!(
                        "Sprite '{}' is fully transparent",
                        sprite.name
                    )));
                }
                // Either the sprite is bigger than the bin, or remaining space
                // is too tight — both surface as overflow to the caller.
                return Err(Error::AtlasOverflow {
                    fits: packed,
                    total: sprites.len() as u32,
                });
            }
        }
    }

    // 4. Compute atlas dimensions (bbox of placed rects + extrude/padding)
    let used_w = placed
        .iter()
        .map(|(_, r)| r.x + r.w + extrude + padding)
        .max()
        .unwrap_or(0);
    let used_h = placed
        .iter()
        .map(|(_, r)| r.y + r.h + extrude + padding)
        .max()
        .unwrap_or(0);

    let (atlas_w, atlas_h) = if opts.power_of_two {
        let w = next_power_of_two(used_w).min(max_size);
        let h = next_power_of_two(used_h).min(max_size);
        (w, h)
    } else {
        (used_w.min(max_size), used_h.min(max_size))
    };

    if atlas_w == 0 || atlas_h == 0 {
        return Err(Error::AtlasPackFailed(
            "Computed atlas size is zero — no sprites packed".into(),
        ));
    }

    // 5. Compose atlas image (own buffer, blit sprites + extrude)
    let mut atlas = RgbaImage::from_pixel(atlas_w, atlas_h, Rgba([0, 0, 0, 0]));
    for (idx, rect) in &placed {
        let sprite = &sprites[*idx];
        let dst_x = rect.x;
        let dst_y = rect.y;
        blit_with_extrude(&mut atlas, &sprite.trimmed, dst_x, dst_y, extrude);
    }

    // 6. Build metadata
    let atlas_filename = format!("{}.{}", opts.name, opts.format.extension());
    let mut frames: BTreeMap<String, FrameEntry> = BTreeMap::new();
    let mut content_area: u64 = 0;
    for (idx, rect) in &placed {
        let sprite = &sprites[*idx];
        let frame = FrameRect {
            x: rect.x,
            y: rect.y,
            w: rect.w,
            h: rect.h,
        };
        let sprite_source_size = FrameRect {
            x: sprite.trim_offset.0,
            y: sprite.trim_offset.1,
            w: rect.w,
            h: rect.h,
        };
        let source_size = SourceSize {
            w: sprite.original_size.0,
            h: sprite.original_size.1,
        };
        content_area += (rect.w as u64) * (rect.h as u64);
        frames.insert(
            sprite.name.clone(),
            FrameEntry {
                frame,
                rotated: false,
                trimmed: opts.trim && sprite.was_trimmed,
                sprite_source_size,
                source_size,
            },
        );
    }
    let total_area = (atlas_w as u64) * (atlas_h as u64);
    let efficiency = if total_area > 0 {
        (content_area as f64 / total_area as f64) as f32
    } else {
        0.0
    };
    let json = AtlasJson {
        frames,
        meta: AtlasMeta {
            image: atlas_filename.clone(),
            size: AtlasSize {
                w: atlas_w,
                h: atlas_h,
            },
            format: "RGBA8888".to_string(),
            scale: "1".to_string(),
        },
    };

    // 7. Encode + write
    let atlas_path = output_dir.join(&atlas_filename);
    let metadata_path = output_dir.join(format!("{}.json", opts.name));
    encode_atlas(&atlas, &atlas_path, opts)?;
    let json_str = serde_json::to_string_pretty(&json)?;
    std::fs::write(&metadata_path, json_str)?;

    Ok(Report {
        atlas_path,
        metadata_path,
        packed,
        total: sprites.len() as u32,
        atlas_size: (atlas_w, atlas_h),
        efficiency,
    })
}

/// Internal sprite representation — original buffer is dropped after trim, we
/// hold only the trimmed sub-image and the offset inside the original frame.
struct LoadedSprite {
    name: String,
    trimmed: RgbaImage,
    /// (x, y) of the trimmed bbox inside the original sprite.
    trim_offset: (u32, u32),
    /// (w, h) of the original (untrimmed) sprite.
    original_size: (u32, u32),
    /// True if the trim actually shrunk the sprite.
    was_trimmed: bool,
}

fn load_sprites(paths: &[PathBuf], do_trim: bool, max_size: u32) -> Result<Vec<LoadedSprite>> {
    let mut sprites = Vec::with_capacity(paths.len());
    for path in paths {
        let file_name = path
            .file_name()
            .ok_or_else(|| Error::InvalidInput(format!("bad path: {}", path.display())))?
            .to_string_lossy()
            .into_owned();
        let img = image::open(path)?.into_rgba8();
        let (w, h) = (img.width(), img.height());
        if w > max_size || h > max_size {
            return Err(Error::AtlasPackFailed(format!(
                "Sprite '{file_name}' ({w}x{h}) exceeds max_size {max_size}"
            )));
        }

        let (trimmed, offset, was_trimmed) = if do_trim {
            match alpha_bbox(&img) {
                Some((x0, y0, x1, y1)) => {
                    let tw = x1 - x0 + 1;
                    let th = y1 - y0 + 1;
                    let sub = img.view(x0, y0, tw, th).to_image();
                    let was = (tw, th) != (w, h);
                    (sub, (x0, y0), was)
                }
                None => {
                    return Err(Error::AtlasPackFailed(format!(
                        "Sprite '{file_name}' is fully transparent"
                    )));
                }
            }
        } else {
            (img, (0, 0), false)
        };

        sprites.push(LoadedSprite {
            name: file_name,
            trimmed,
            trim_offset: offset,
            original_size: (w, h),
            was_trimmed,
        });
    }
    Ok(sprites)
}

/// Tight alpha bounding box: smallest rect containing all pixels with alpha>0.
/// Returns `None` for fully transparent images.
fn alpha_bbox(img: &RgbaImage) -> Option<(u32, u32, u32, u32)> {
    let (w, h) = (img.width(), img.height());
    let mut min_x = w;
    let mut min_y = h;
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    let mut any = false;
    for y in 0..h {
        for x in 0..w {
            let p = img.get_pixel(x, y);
            if p[3] != 0 {
                any = true;
                if x < min_x {
                    min_x = x;
                }
                if y < min_y {
                    min_y = y;
                }
                if x > max_x {
                    max_x = x;
                }
                if y > max_y {
                    max_y = y;
                }
            }
        }
    }
    if any {
        Some((min_x, min_y, max_x, max_y))
    } else {
        None
    }
}

/// Copy `src` into `atlas` at (`dst_x`, `dst_y`) and replicate edge pixels
/// `extrude` pixels outward to prevent GPU bilinear sampling from bleeding the
/// neighboring sprite. Edges are clamped to atlas bounds.
fn blit_with_extrude(atlas: &mut RgbaImage, src: &RgbaImage, dst_x: u32, dst_y: u32, extrude: u32) {
    let (sw, sh) = (src.width(), src.height());
    let (aw, ah) = (atlas.width(), atlas.height());

    // Main copy
    for sy in 0..sh {
        for sx in 0..sw {
            let p = src.get_pixel(sx, sy);
            let ax = dst_x + sx;
            let ay = dst_y + sy;
            if ax < aw && ay < ah {
                atlas.put_pixel(ax, ay, *p);
            }
        }
    }

    if extrude == 0 {
        return;
    }

    // Extrude top / bottom rows
    for k in 1..=extrude {
        for sx in 0..sw {
            let top = src.get_pixel(sx, 0);
            let bot = src.get_pixel(sx, sh - 1);
            let ax = dst_x + sx;
            if ax < aw {
                if dst_y >= k {
                    atlas.put_pixel(ax, dst_y - k, *top);
                }
                let by = dst_y + sh + k - 1;
                if by < ah {
                    atlas.put_pixel(ax, by, *bot);
                }
            }
        }
    }
    // Extrude left / right columns
    for k in 1..=extrude {
        for sy in 0..sh {
            let left = src.get_pixel(0, sy);
            let right = src.get_pixel(sw - 1, sy);
            let ay = dst_y + sy;
            if ay < ah {
                if dst_x >= k {
                    atlas.put_pixel(dst_x - k, ay, *left);
                }
                let rx = dst_x + sw + k - 1;
                if rx < aw {
                    atlas.put_pixel(rx, ay, *right);
                }
            }
        }
    }
    // Extrude corner cells (replicate the four corner pixels into the diagonal pad)
    let tl = *src.get_pixel(0, 0);
    let tr = *src.get_pixel(sw - 1, 0);
    let bl = *src.get_pixel(0, sh - 1);
    let br = *src.get_pixel(sw - 1, sh - 1);
    for ky in 1..=extrude {
        for kx in 1..=extrude {
            // top-left
            if dst_x >= kx && dst_y >= ky {
                atlas.put_pixel(dst_x - kx, dst_y - ky, tl);
            }
            // top-right
            let rx = dst_x + sw + kx - 1;
            if rx < aw && dst_y >= ky {
                atlas.put_pixel(rx, dst_y - ky, tr);
            }
            // bottom-left
            let by = dst_y + sh + ky - 1;
            if dst_x >= kx && by < ah {
                atlas.put_pixel(dst_x - kx, by, bl);
            }
            // bottom-right
            if rx < aw && by < ah {
                atlas.put_pixel(rx, by, br);
            }
        }
    }
}

fn encode_atlas(atlas: &RgbaImage, path: &Path, opts: &Options) -> Result<()> {
    match opts.format {
        OutputFormat::Png => {
            atlas.save(path)?;
            Ok(())
        }
        OutputFormat::Webp => {
            let encoder = webp::Encoder::from_rgba(atlas.as_raw(), atlas.width(), atlas.height());
            let webp_data = encoder.encode(opts.webp_quality.clamp(0, 100) as f32);
            std::fs::write(path, &*webp_data)?;
            Ok(())
        }
    }
}

fn next_power_of_two(n: u32) -> u32 {
    if n <= 1 {
        return 1;
    }
    let mut p = 1u32;
    while p < n {
        p = p.saturating_mul(2);
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmpdir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("pixiekit-atlas-{}-{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_solid_png(dir: &Path, name: &str, w: u32, h: u32, color: [u8; 4]) -> PathBuf {
        let img = RgbaImage::from_pixel(w, h, Rgba(color));
        let path = dir.join(name);
        img.save(&path).unwrap();
        path
    }

    /// Sprite with a transparent border so trim has something to do.
    fn write_bordered_png(
        dir: &Path,
        name: &str,
        outer: u32,
        border: u32,
        color: [u8; 4],
    ) -> PathBuf {
        let mut img = RgbaImage::from_pixel(outer, outer, Rgba([0, 0, 0, 0]));
        for y in border..(outer - border) {
            for x in border..(outer - border) {
                img.put_pixel(x, y, Rgba(color));
            }
        }
        let path = dir.join(name);
        img.save(&path).unwrap();
        path
    }

    #[test]
    fn output_format_extensions() {
        assert_eq!(OutputFormat::Png.extension(), "png");
        assert_eq!(OutputFormat::Webp.extension(), "webp");
    }

    #[test]
    fn options_default_values() {
        let opts = Options::default();
        assert_eq!(opts.name, "atlas");
        assert_eq!(opts.max_size, 2048);
        assert_eq!(opts.padding, 2);
        assert_eq!(opts.extrude, 1);
        assert!(opts.power_of_two);
        assert!(opts.trim);
        assert_eq!(opts.format, OutputFormat::Png);
    }

    #[test]
    fn next_power_of_two_clamps() {
        assert_eq!(next_power_of_two(0), 1);
        assert_eq!(next_power_of_two(1), 1);
        assert_eq!(next_power_of_two(2), 2);
        assert_eq!(next_power_of_two(3), 4);
        assert_eq!(next_power_of_two(513), 1024);
    }

    #[test]
    fn alpha_bbox_finds_content() {
        let mut img = RgbaImage::from_pixel(10, 10, Rgba([0, 0, 0, 0]));
        img.put_pixel(3, 4, Rgba([255, 0, 0, 255]));
        img.put_pixel(6, 7, Rgba([0, 255, 0, 255]));
        let bbox = alpha_bbox(&img).unwrap();
        assert_eq!(bbox, (3, 4, 6, 7));
    }

    #[test]
    fn alpha_bbox_empty_for_fully_transparent() {
        let img = RgbaImage::from_pixel(8, 8, Rgba([0, 0, 0, 0]));
        assert!(alpha_bbox(&img).is_none());
    }

    #[test]
    fn process_packs_multiple_solid_frames() {
        let dir = tmpdir("packs_solid");
        let p1 = write_solid_png(&dir, "a.png", 32, 32, [255, 0, 0, 255]);
        let p2 = write_solid_png(&dir, "b.png", 32, 32, [0, 255, 0, 255]);
        let p3 = write_solid_png(&dir, "c.png", 32, 32, [0, 0, 255, 255]);

        let opts = Options {
            name: "atlas".into(),
            max_size: 256,
            padding: 2,
            extrude: 1,
            power_of_two: true,
            trim: false,
            format: OutputFormat::Png,
            webp_quality: 90,
        };
        let report = process(&[p1, p2, p3], &dir, &opts).unwrap();
        assert_eq!(report.packed, 3);
        assert_eq!(report.total, 3);
        assert!(report.atlas_path.exists());
        assert!(report.metadata_path.exists());
        // Atlas dimensions should be powers of two
        assert!(report.atlas_size.0.is_power_of_two());
        assert!(report.atlas_size.1.is_power_of_two());
    }

    #[test]
    fn process_marks_trimmed_when_alpha_border_present() {
        let dir = tmpdir("trimmed_flag");
        let p = write_bordered_png(&dir, "wave_01.png", 64, 16, [200, 100, 50, 255]);

        let opts = Options {
            name: "atlas".into(),
            max_size: 256,
            padding: 1,
            extrude: 0,
            power_of_two: false,
            trim: true,
            format: OutputFormat::Png,
            webp_quality: 90,
        };
        let report = process(&[p], &dir, &opts).unwrap();
        assert_eq!(report.packed, 1);

        let json_str = std::fs::read_to_string(&report.metadata_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        let frame = &parsed["frames"]["wave_01.png"];
        assert_eq!(frame["trimmed"], true);
        // sourceSize should preserve the original 64×64
        assert_eq!(frame["sourceSize"]["w"], 64);
        assert_eq!(frame["sourceSize"]["h"], 64);
        // spriteSourceSize should reflect the trim offset (16,16)
        assert_eq!(frame["spriteSourceSize"]["x"], 16);
        assert_eq!(frame["spriteSourceSize"]["y"], 16);
    }

    #[test]
    fn process_high_efficiency_uniform_sprites() {
        let dir = tmpdir("efficient");
        let mut paths = Vec::new();
        for i in 0..16 {
            let color = [(i * 16) as u8, 128, 64, 255];
            paths.push(write_solid_png(
                &dir,
                &format!("s_{i:02}.png"),
                64,
                64,
                color,
            ));
        }
        let opts = Options {
            name: "uniform".into(),
            max_size: 512,
            padding: 0,
            extrude: 0,
            power_of_two: true,
            trim: false,
            format: OutputFormat::Png,
            webp_quality: 90,
        };
        let report = process(&paths, &dir, &opts).unwrap();
        assert_eq!(report.packed, 16);
        // 16 × 64×64 = 65536 px content; 256×256 bin = 65536. > 75% efficiency.
        assert!(
            report.efficiency > 0.75,
            "efficiency {} should exceed 0.75",
            report.efficiency
        );
    }

    #[test]
    fn process_errors_when_sprite_exceeds_max_size() {
        let dir = tmpdir("oversized");
        let p = write_solid_png(&dir, "big.png", 512, 512, [255, 255, 255, 255]);

        let opts = Options {
            name: "atlas".into(),
            max_size: 256,
            padding: 0,
            extrude: 0,
            power_of_two: true,
            trim: false,
            format: OutputFormat::Png,
            webp_quality: 90,
        };
        let result = process(&[p], &dir, &opts);
        assert!(result.is_err());
    }

    #[test]
    fn process_writes_valid_json_schema() {
        let dir = tmpdir("schema");
        let p1 = write_solid_png(&dir, "x.png", 16, 16, [10, 20, 30, 255]);
        let p2 = write_solid_png(&dir, "y.png", 24, 24, [40, 50, 60, 255]);
        let opts = Options {
            name: "atlas".into(),
            max_size: 128,
            padding: 1,
            extrude: 0,
            power_of_two: true,
            trim: false,
            format: OutputFormat::Png,
            webp_quality: 90,
        };
        let report = process(&[p1, p2], &dir, &opts).unwrap();
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&report.metadata_path).unwrap()).unwrap();

        assert!(json["frames"].is_object());
        assert!(json["meta"].is_object());
        assert_eq!(json["meta"]["format"], "RGBA8888");
        assert_eq!(json["meta"]["scale"], "1");
        assert_eq!(json["meta"]["image"], "atlas.png");
        assert!(json["meta"]["size"]["w"].is_number());
        assert!(json["meta"]["size"]["h"].is_number());

        // Each frame has the required keys
        for (_, frame) in json["frames"].as_object().unwrap() {
            assert!(frame["frame"].is_object());
            assert_eq!(frame["rotated"], false);
            assert!(frame["trimmed"].is_boolean());
            assert!(frame["spriteSourceSize"].is_object());
            assert!(frame["sourceSize"].is_object());
        }
    }

    #[test]
    fn process_empty_input_returns_zero() {
        let dir = tmpdir("empty_input");
        let opts = Options::default();
        let report = process(&[], &dir, &opts).unwrap();
        assert_eq!(report.packed, 0);
        assert_eq!(report.total, 0);
    }

    #[test]
    fn process_webp_format_writes_webp_file() {
        let dir = tmpdir("webp_format");
        let p = write_solid_png(&dir, "a.png", 32, 32, [120, 30, 200, 255]);
        let opts = Options {
            name: "atlas".into(),
            max_size: 128,
            padding: 0,
            extrude: 0,
            power_of_two: true,
            trim: false,
            format: OutputFormat::Webp,
            webp_quality: 90,
        };
        let report = process(&[p], &dir, &opts).unwrap();
        assert!(report.atlas_path.exists());
        assert_eq!(
            report.atlas_path.extension().and_then(|s| s.to_str()),
            Some("webp")
        );

        let json_str = std::fs::read_to_string(&report.metadata_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["meta"]["image"], "atlas.webp");
    }
}
