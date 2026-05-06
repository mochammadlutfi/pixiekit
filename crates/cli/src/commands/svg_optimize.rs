use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{anyhow, Context, Result};
use clap::Args as ClapArgs;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;

use pixiekit_core::{batch, preset, svg_optimize};

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Input SVG file or folder
    #[arg(short, long)]
    pub input: PathBuf,

    /// Output folder (created if missing)
    #[arg(short, long)]
    pub output: PathBuf,

    /// Decimal places for path coordinates / transforms (0 - 8)
    #[arg(long, default_value_t = 3)]
    pub precision: u8,

    /// Keep <title>, <desc>, and XML comments
    #[arg(long)]
    pub keep_metadata: bool,

    /// Keep elements with display="none" / visibility="hidden"
    #[arg(long)]
    pub keep_hidden: bool,

    /// Skip path-merging post passes (currently disables metadata/hidden strip)
    #[arg(long)]
    pub no_merge_paths: bool,

    /// Pretty-print output (default: minified)
    #[arg(long)]
    pub pretty: bool,

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

    /// Load tool options from a preset JSON file (overrides individual flags)
    #[arg(long, value_name = "PATH")]
    pub config: Option<PathBuf>,
}

pub fn run(args: Args) -> Result<()> {
    crate::commands::preflight_input(&args.input)?;

    let opts = match &args.config {
        Some(path) => load_options_from_config(path)?,
        None => svg_optimize::Options {
            precision: args.precision,
            remove_metadata: !args.keep_metadata,
            remove_hidden: !args.keep_hidden,
            merge_paths: !args.no_merge_paths,
            pretty: args.pretty,
        },
    };

    let files =
        batch::list_images(&args.input, args.recursive, &["svg"]).context("Listing input files")?;

    if files.is_empty() {
        if args.json {
            println!(
                "{}",
                serde_json::json!({
                    "tool": "svg-optimize",
                    "processed": 0,
                    "failed": 0,
                    "files": [],
                    "warning": "No SVG files found"
                })
            );
        } else {
            eprintln!("No SVG files found in {}", args.input.display());
        }
        return Ok(());
    }

    if args.dry_run {
        for f in &files {
            println!("[dry-run] would optimize: {}", f.display());
        }
        return Ok(());
    }

    std::fs::create_dir_all(&args.output)
        .with_context(|| format!("Creating output dir {}", args.output.display()))?;

    let pb = if args.json {
        ProgressBar::hidden()
    } else {
        let bar = ProgressBar::new(files.len() as u64);
        bar.set_style(
            ProgressStyle::with_template("{bar:40.cyan/blue} {pos}/{len} {wide_msg}")
                .unwrap()
                .progress_chars("█▓░"),
        );
        bar
    };

    let start = Instant::now();
    let results: Vec<FileResult> = files
        .par_iter()
        .map(|input_path| {
            let result = process_one(input_path, &args.output, &opts, args.overwrite);
            pb.inc(1);
            if let Some(name) = input_path.file_name() {
                pb.set_message(name.to_string_lossy().into_owned());
            }
            match result {
                Ok((path, report)) => FileResult {
                    input: input_path.clone(),
                    output: Some(path),
                    input_size: Some(report.input_size),
                    output_size: Some(report.output_size),
                    ratio: Some(report.ratio),
                    error: None,
                },
                Err(e) => FileResult {
                    input: input_path.clone(),
                    output: None,
                    input_size: None,
                    output_size: None,
                    ratio: None,
                    error: Some(format!("{:#}", e)),
                },
            }
        })
        .collect();
    pb.finish_and_clear();

    let processed = results.iter().filter(|r| r.error.is_none()).count();
    let failed = results.len() - processed;
    let duration_ms = start.elapsed().as_millis();

    if args.json {
        let json = serde_json::json!({
            "tool": "svg-optimize",
            "processed": processed,
            "failed": failed,
            "duration_ms": duration_ms,
            "files": results.iter().map(|r| {
                serde_json::json!({
                    "input": r.input,
                    "output": r.output,
                    "input_size": r.input_size,
                    "output_size": r.output_size,
                    "ratio": r.ratio,
                    "status": if r.error.is_none() { "ok" } else { "failed" },
                    "error": r.error,
                })
            }).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        println!(
            "✓ Optimized {}/{} files in {}ms",
            processed,
            files.len(),
            duration_ms
        );
        for r in &results {
            if let Some(err) = &r.error {
                eprintln!(
                    "  ✗ {}: {}",
                    r.input.file_name().unwrap_or_default().to_string_lossy(),
                    err
                );
            }
        }
    }

    if failed > 0 {
        std::process::exit(1);
    }
    Ok(())
}

struct FileResult {
    input: PathBuf,
    output: Option<PathBuf>,
    input_size: Option<u64>,
    output_size: Option<u64>,
    ratio: Option<f32>,
    error: Option<String>,
}

fn process_one(
    input_path: &Path,
    output_dir: &Path,
    opts: &svg_optimize::Options,
    overwrite: bool,
) -> Result<(PathBuf, svg_optimize::SvgReport)> {
    let file_name = input_path
        .file_name()
        .ok_or_else(|| anyhow!("Invalid filename: {}", input_path.display()))?;
    let output_path = output_dir.join(file_name);

    if !overwrite && output_path.exists() {
        return Err(anyhow!(
            "Output file exists (use --overwrite): {}",
            output_path.display()
        ));
    }

    let report = svg_optimize::process(input_path, &output_path, opts)
        .with_context(|| format!("Optimizing {}", input_path.display()))?;
    Ok((output_path, report))
}

fn load_options_from_config(path: &Path) -> Result<svg_optimize::Options> {
    let preset = preset::load_from_path(path)
        .with_context(|| format!("Loading preset {}", path.display()))?;
    preset::ensure_tool(&preset, preset::TOOL_SVG_OPTIMIZE)
        .with_context(|| format!("Preset {} is not an svg-optimize preset", path.display()))?;
    serde_json::from_value(preset.options)
        .with_context(|| format!("Decoding svg-optimize options from {}", path.display()))
}
