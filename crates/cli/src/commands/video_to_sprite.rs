use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{anyhow, Context, Result};
use clap::Args as ClapArgs;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;

use pixiekit_core::{batch, bg_remove, video_to_sprite};

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Input video file or folder of videos
    #[arg(short, long)]
    pub input: PathBuf,

    /// Output folder (created if missing)
    #[arg(short, long)]
    pub output: PathBuf,

    /// Target output FPS (1 - 30)
    #[arg(long, default_value_t = 8)]
    pub fps: u8,

    /// Frame size, square (64 - 1024)
    #[arg(long, default_value_t = 256)]
    pub size: u32,

    /// Output format (png | webp)
    #[arg(long, default_value = "webp")]
    pub format: String,

    /// WebP quality 0-100 (alpha is always lossless)
    #[arg(long, default_value_t = 90)]
    pub webp_quality: u8,

    /// Apply BG remove (chroma key) per frame
    #[arg(long)]
    pub chroma_key: bool,

    /// Chroma key target color (only with --chroma-key)
    #[arg(long, default_value = "#00FF00")]
    pub chroma_target: String,

    /// Chroma key fuzz threshold (only with --chroma-key)
    #[arg(long, default_value_t = 0.35)]
    pub chroma_fuzz: f32,

    /// Disable despill (default: enabled when --chroma-key)
    #[arg(long)]
    pub no_despill: bool,

    /// Chroma key alpha erode iterations
    #[arg(long, default_value_t = 1)]
    pub chroma_erode: u8,

    /// Recursive folder scan
    #[arg(short, long)]
    pub recursive: bool,

    /// Overwrite existing output files
    #[arg(long)]
    pub overwrite: bool,

    /// JSON output (for AI / scripting)
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: Args) -> Result<()> {
    let format =
        parse_format(&args.format).with_context(|| format!("Invalid --format: {}", args.format))?;

    let chroma_key = if args.chroma_key {
        Some(bg_remove::Options {
            target_color: parse_hex_color(&args.chroma_target)
                .with_context(|| format!("Invalid --chroma-target: {}", args.chroma_target))?,
            fuzz: args.chroma_fuzz,
            despill: !args.no_despill,
            erode: args.chroma_erode,
        })
    } else {
        None
    };

    let opts = video_to_sprite::Options {
        fps: args.fps,
        frame_size: args.size,
        output_format: format,
        webp_quality: args.webp_quality,
        chroma_key,
    };

    let videos = batch::list_images(&args.input, args.recursive, &["mp4", "mov", "webm"])
        .context("Listing input videos")?;

    if videos.is_empty() {
        if args.json {
            println!(
                "{}",
                serde_json::json!({
                    "tool": "video-to-sprite",
                    "processed": 0,
                    "failed": 0,
                    "files": [],
                    "warning": "No videos found"
                })
            );
        } else {
            eprintln!("No videos found in {}", args.input.display());
        }
        return Ok(());
    }

    std::fs::create_dir_all(&args.output)
        .with_context(|| format!("Creating output dir {}", args.output.display()))?;

    // ffmpeg early check — fail fast with clear error
    video_to_sprite::check_ffmpeg().context("ffmpeg check")?;

    let pb = if args.json {
        ProgressBar::hidden()
    } else {
        let bar = ProgressBar::new(videos.len() as u64);
        bar.set_style(
            ProgressStyle::with_template("{bar:40.cyan/blue} {pos}/{len} {wide_msg}")
                .unwrap()
                .progress_chars("█▓░"),
        );
        bar
    };

    // Parallel per video — rayon spawns one task per logical CPU core.
    // Each video has its own ffmpeg subprocess + temp dir, no shared state.
    // ProgressBar is internally synchronized, safe across threads.
    let start = Instant::now();
    let results: Vec<VideoResult> = videos
        .par_iter()
        .map(|video_path| {
            let result = process_one(video_path, &args.output, &opts, args.overwrite);
            pb.inc(1);
            if let Some(name) = video_path.file_name() {
                pb.set_message(name.to_string_lossy().into_owned());
            }
            VideoResult {
                input: video_path.clone(),
                report: result.as_ref().ok().cloned(),
                error: result.err().map(|e| format!("{:#}", e)),
            }
        })
        .collect();
    pb.finish_and_clear();

    let processed = results.iter().filter(|r| r.error.is_none()).count();
    let failed = results.len() - processed;
    let duration_ms = start.elapsed().as_millis();

    if args.json {
        let json = serde_json::json!({
            "tool": "video-to-sprite",
            "processed": processed,
            "failed": failed,
            "duration_ms": duration_ms,
            "files": results.iter().map(|r| {
                serde_json::json!({
                    "input": r.input,
                    "sprite": r.report.as_ref().map(|x| &x.sprite_path),
                    "metadata": r.report.as_ref().map(|x| &x.metadata_path),
                    "frame_count": r.report.as_ref().map(|x| x.frame_count),
                    "frame_size": r.report.as_ref().map(|x| x.frame_size),
                    "status": if r.error.is_none() { "ok" } else { "failed" },
                    "error": r.error,
                })
            }).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        println!(
            "✓ Processed {}/{} videos in {}ms",
            processed,
            videos.len(),
            duration_ms
        );
        for r in &results {
            match &r.report {
                Some(rep) => println!(
                    "  ✓ {} → {} ({} frames)",
                    r.input.file_name().unwrap_or_default().to_string_lossy(),
                    rep.sprite_path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy(),
                    rep.frame_count
                ),
                None => eprintln!(
                    "  ✗ {}: {}",
                    r.input.file_name().unwrap_or_default().to_string_lossy(),
                    r.error.as_deref().unwrap_or("unknown")
                ),
            }
        }
    }

    if failed > 0 {
        std::process::exit(1);
    }
    Ok(())
}

struct VideoResult {
    input: PathBuf,
    report: Option<video_to_sprite::ProcessReport>,
    error: Option<String>,
}

fn process_one(
    video_path: &Path,
    output_dir: &Path,
    opts: &video_to_sprite::Options,
    overwrite: bool,
) -> Result<video_to_sprite::ProcessReport> {
    // Pre-flight overwrite check
    if !overwrite {
        let stem = video_path
            .file_stem()
            .ok_or_else(|| anyhow!("Invalid filename: {}", video_path.display()))?;
        let sprite = output_dir.join(format!(
            "{}.{}",
            stem.to_string_lossy(),
            opts.output_format.extension()
        ));
        if sprite.exists() {
            return Err(anyhow!(
                "Output exists (use --overwrite): {}",
                sprite.display()
            ));
        }
    }
    Ok(video_to_sprite::process(video_path, output_dir, opts)?)
}

fn parse_format(s: &str) -> Result<video_to_sprite::OutputFormat> {
    match s.to_ascii_lowercase().as_str() {
        "png" => Ok(video_to_sprite::OutputFormat::Png),
        "webp" => Ok(video_to_sprite::OutputFormat::Webp),
        other => Err(anyhow!("Unsupported format: {} (expected png|webp)", other)),
    }
}

fn parse_hex_color(s: &str) -> Result<[u8; 3]> {
    let trimmed = s.trim_start_matches('#');
    if trimmed.len() != 6 {
        return Err(anyhow!("Hex color must be 6 chars (e.g., #00FF00)"));
    }
    let r = u8::from_str_radix(&trimmed[0..2], 16)?;
    let g = u8::from_str_radix(&trimmed[2..4], 16)?;
    let b = u8::from_str_radix(&trimmed[4..6], 16)?;
    Ok([r, g, b])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_format_lowercase() {
        assert_eq!(
            parse_format("png").unwrap(),
            video_to_sprite::OutputFormat::Png
        );
        assert_eq!(
            parse_format("webp").unwrap(),
            video_to_sprite::OutputFormat::Webp
        );
    }

    #[test]
    fn parse_format_uppercase() {
        assert_eq!(
            parse_format("WEBP").unwrap(),
            video_to_sprite::OutputFormat::Webp
        );
    }

    #[test]
    fn parse_format_unknown() {
        assert!(parse_format("gif").is_err());
    }
}
