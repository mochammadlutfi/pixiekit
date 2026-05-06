use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result};
use clap::{Args as ClapArgs, ValueEnum};
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use serde::Serialize;

use pixiekit_core::{anim_preview, preset};

#[derive(ValueEnum, Clone, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PreviewFormat {
    Gif,
    Mp4,
    Webm,
}

impl From<PreviewFormat> for anim_preview::PreviewFormat {
    fn from(f: PreviewFormat) -> Self {
        match f {
            PreviewFormat::Gif => anim_preview::PreviewFormat::Gif,
            PreviewFormat::Mp4 => anim_preview::PreviewFormat::Mp4,
            PreviewFormat::Webm => anim_preview::PreviewFormat::Webm,
        }
    }
}

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Input sprite sheet PNG or folder of PNG frames
    #[arg(short, long)]
    pub input: PathBuf,

    /// Output folder (created if missing)
    #[arg(short, long)]
    pub output: PathBuf,

    /// Target output FPS (1 - 30)
    #[arg(long, default_value_t = 8)]
    pub fps: u8,

    /// Output format (gif | mp4 | webm)
    #[arg(long, value_enum, default_value_t = PreviewFormat::Gif)]
    pub format: PreviewFormat,

    /// Loop the animation (GIF only)
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    pub loop_anim: bool,

    /// Nearest-neighbor upscale factor (1, 2, 4)
    #[arg(long, default_value_t = 1)]
    pub upscale: u8,

    /// Size of each square frame (pixels). If omitted, auto-detected from sibling JSON.
    #[arg(long)]
    pub frame_size: Option<u32>,

    /// Recursive folder scan
    #[arg(short, long)]
    pub recursive: bool,

    /// Overwrite existing output files
    #[arg(long)]
    pub overwrite: bool,

    /// JSON output (for AI / scripting)
    #[arg(long)]
    pub json: bool,

    /// Load tool options from a preset JSON file
    #[arg(long, value_name = "PATH")]
    pub config: Option<PathBuf>,
}

pub fn run(args: Args) -> Result<()> {
    crate::commands::preflight_input(&args.input)?;

    let opts = match &args.config {
        Some(path) => load_options_from_config(path)?,
        None => anim_preview::Options {
            fps: args.fps,
            output_format: args.format.into(),
            loop_anim: args.loop_anim,
            upscale: args.upscale,
            frame_size: args.frame_size,
        },
    };

    // Input can be a file (sprite sheet) or a directory (frame folder).
    // If it's a directory, we check if it contains PNGs.
    let inputs = if args.input.is_dir() {
        // If it's a directory, we might be processing one animation (folder mode)
        // OR a folder of sprite sheets (batch mode).
        // If the folder has PNGs at root, it's likely "folder mode".
        // Otherwise, we look for sprite sheets (PNG files) or subfolders.
        let pngs_at_root = std::fs::read_dir(&args.input)?
            .filter_map(|e| e.ok())
            .any(|e| e.path().extension().is_some_and(|ext| ext == "png"));

        if pngs_at_root {
            // Folder mode — process this directory as ONE animation
            vec![args.input.clone()]
        } else {
            // Batch mode — treat subfolders or PNGs as separate animations
            let mut all = Vec::new();
            // Subdirectories (each is a frame folder)
            for entry in std::fs::read_dir(&args.input)? {
                let entry = entry?;
                if entry.file_type()?.is_dir() || entry.path().extension().is_some_and(|ext| ext == "png") {
                    all.push(entry.path());
                }
            }
            all.sort();
            all
        }
    } else {
        // Single sprite sheet
        vec![args.input.clone()]
    };

    if inputs.is_empty() {
        if args.json {
            println!(
                "{}",
                serde_json::json!({
                    "tool": "anim-preview",
                    "processed": 0,
                    "failed": 0,
                    "files": [],
                    "warning": "No animations found"
                })
            );
        } else {
            println!("No animations found.");
        }
        return Ok(());
    }

    if !args.output.exists() {
        std::fs::create_dir_all(&args.output)?;
    }

    anim_preview::check_ffmpeg().context("ffmpeg check")?;

    if !args.json {
        println!(
            "Processing {} animations into {:?} ({} FPS)...",
            inputs.len(),
            opts.output_format,
            opts.fps
        );
    }

    let start = Instant::now();
    let pb = if args.json || inputs.len() < 2 {
        None
    } else {
        let p = ProgressBar::new(inputs.len() as u64);
        p.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")?
                .progress_chars("#>-"),
        );
        Some(p)
    };

    let results: Vec<_> = inputs
        .par_iter()
        .map(|path| {
            let res = anim_preview::process(path, &args.output, &opts);
            if let Some(p) = &pb {
                p.inc(1);
            }
            (path, res)
        })
        .collect();

    if let Some(p) = pb {
        p.finish_and_clear();
    }

    let mut success_count = 0;
    let mut failed_count = 0;
    let mut reports = Vec::new();

    for (path, res) in results {
        match res {
            Ok(report) => {
                success_count += 1;
                if args.json {
                    reports.push(serde_json::json!({
                        "input": path,
                        "status": "success",
                        "output": report.output_path,
                        "frames": report.frame_count,
                    }));
                }
            }
            Err(e) => {
                failed_count += 1;
                if args.json {
                    reports.push(serde_json::json!({
                        "input": path,
                        "status": "error",
                        "error": e.to_string()
                    }));
                } else {
                    eprintln!("Error processing {}: {}", path.display(), e);
                }
            }
        }
    }

    if args.json {
        println!(
            "{}",
            serde_json::json!({
                "tool": "anim-preview",
                "processed": success_count,
                "failed": failed_count,
                "duration_ms": start.elapsed().as_millis(),
                "results": reports
            })
        );
    } else {
        println!(
            "Done! Processed {} animations ({} failed) in {:.2?}",
            success_count,
            failed_count,
            start.elapsed()
        );
    }

    Ok(())
}

fn load_options_from_config(path: &Path) -> Result<anim_preview::Options> {
    let p = preset::load_from_path(path).context("Loading preset")?;
    preset::ensure_tool(&p, preset::TOOL_ANIM_PREVIEW)?;
    serde_json::from_value(p.options).context("Parsing anim-preview options from preset")
}
