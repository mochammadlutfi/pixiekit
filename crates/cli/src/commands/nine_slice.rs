use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result};
use clap::{Args as ClapArgs, ValueEnum};
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use serde::Serialize;

use pixiekit_core::{batch, nine_slice, preset};

#[derive(ValueEnum, Clone, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputMode {
    Split,
    Metadata,
}

impl From<OutputMode> for nine_slice::OutputMode {
    fn from(m: OutputMode) -> Self {
        match m {
            OutputMode::Split => nine_slice::OutputMode::Split,
            OutputMode::Metadata => nine_slice::OutputMode::Metadata,
        }
    }
}

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Input image file or folder
    #[arg(short, long)]
    pub input: PathBuf,

    /// Output folder (created if missing)
    #[arg(short, long)]
    pub output: PathBuf,

    /// Top inset (pixels)
    #[arg(long)]
    pub top: u32,

    /// Right inset (pixels)
    #[arg(long)]
    pub right: u32,

    /// Bottom inset (pixels)
    #[arg(long)]
    pub bottom: u32,

    /// Left inset (pixels)
    #[arg(long)]
    pub left: u32,

    /// Output mode: split (9 files) or metadata (JSON sibling)
    #[arg(long = "output-mode", value_enum, default_value_t = OutputMode::Metadata)]
    pub mode: OutputMode,

    /// Recursive folder scan
    #[arg(short, long)]
    pub recursive: bool,

    /// Overwrite existing output files
    #[arg(long)]
    pub overwrite: bool,

    /// Print plan, do not write
    #[arg(long)]
    pub dry_run: bool,

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
        None => nine_slice::Options {
            top: args.top,
            right: args.right,
            bottom: args.bottom,
            left: args.left,
            output_mode: args.mode.into(),
        },
    };

    let files = batch::list_images(&args.input, args.recursive, &["png", "jpg", "jpeg", "webp"])
        .context("Listing input files")?;

    if files.is_empty() {
        if args.json {
            println!(
                "{}",
                serde_json::json!({
                    "tool": "nine-slice",
                    "processed": 0,
                    "failed": 0,
                    "files": [],
                    "warning": "No images found"
                })
            );
        } else {
            println!("No images found.");
        }
        return Ok(());
    }

    if !args.output.exists() && !args.dry_run {
        std::fs::create_dir_all(&args.output)?;
    }

    if !args.json {
        println!(
            "Processing {} files using nine-slice ({:?})...",
            files.len(),
            opts.output_mode
        );
    }

    let start = Instant::now();
    let pb = if args.json || files.len() < 2 {
        None
    } else {
        let p = ProgressBar::new(files.len() as u64);
        p.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")?
                .progress_chars("#>-"),
        );
        Some(p)
    };

    let results: Vec<_> = files
        .par_iter()
        .map(|path| {
            let res = if args.dry_run {
                Ok(nine_slice::NineSliceReport {
                    mode: opts.output_mode,
                    output_files: vec![],
                    image_size: (0, 0),
                })
            } else {
                nine_slice::process(path, &args.output, &opts)
            };

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
                        "mode": report.mode,
                        "size": report.image_size,
                        "output": report.output_files,
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
                "tool": "nine-slice",
                "processed": success_count,
                "failed": failed_count,
                "duration_ms": start.elapsed().as_millis(),
                "results": reports
            })
        );
    } else {
        println!(
            "Done! Processed {} files ({} failed) in {:.2?}",
            success_count,
            failed_count,
            start.elapsed()
        );
    }

    Ok(())
}

fn load_options_from_config(path: &Path) -> Result<nine_slice::Options> {
    let p = preset::load_from_path(path).context("Loading preset")?;
    preset::ensure_tool(&p, preset::TOOL_NINE_SLICE)?;
    serde_json::from_value(p.options).context("Parsing nine-slice options from preset")
}
