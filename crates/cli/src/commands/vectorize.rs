use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{anyhow, Context, Result};
use clap::Args as ClapArgs;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;

use pixiekit_core::{batch, preset, vectorize};

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Input image file or folder
    #[arg(short, long)]
    pub input: PathBuf,

    /// Output folder (created if missing)
    #[arg(short, long)]
    pub output: PathBuf,

    /// Color mode (color | binary)
    #[arg(long, default_value = "color")]
    pub mode: String,

    /// 0-10 simple smoothness slider — overrides --corner-threshold,
    /// --length-threshold, and --splice-threshold when set
    #[arg(long)]
    pub smooth: Option<u8>,

    /// Discard speckle clusters smaller than this (px²)
    #[arg(long, default_value_t = 4)]
    pub filter_speckle: u32,

    /// Color quantization bits per channel (1 - 8)
    #[arg(long, default_value_t = 6)]
    pub color_precision: u8,

    /// Min color difference between layers (0 - 128)
    #[arg(long, default_value_t = 16)]
    pub layer_difference: u8,

    /// Corner detection angle threshold, degrees (0 - 180)
    #[arg(long, default_value_t = 60)]
    pub corner_threshold: u8,

    /// Min segment length in px (0.0 - 10.0)
    #[arg(long, default_value_t = 4.0)]
    pub length_threshold: f64,

    /// Splice angle threshold, degrees (0 - 180)
    #[arg(long, default_value_t = 45)]
    pub splice_threshold: u8,

    /// Decimal places for SVG path coordinates (0 - 16)
    #[arg(long, default_value_t = 8)]
    pub path_precision: u8,

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
        None => {
            let mode = parse_mode(&args.mode).with_context(|| {
                format!("Invalid --mode: {} (expected color|binary)", args.mode)
            })?;
            // If --smooth is provided, derive corner/length/splice from the slider;
            // otherwise honour the individual flags.
            let (corner_threshold, length_threshold, splice_threshold) = match args.smooth {
                Some(s) => vectorize::smooth_to_params(s),
                None => (
                    args.corner_threshold,
                    args.length_threshold,
                    args.splice_threshold,
                ),
            };
            vectorize::Options {
                mode,
                filter_speckle: args.filter_speckle,
                color_precision: args.color_precision,
                layer_difference: args.layer_difference,
                corner_threshold,
                length_threshold,
                splice_threshold,
                path_precision: args.path_precision,
            }
        }
    };

    let files = batch::list_images(&args.input, args.recursive, &["png", "jpg", "jpeg", "webp"])
        .context("Listing input files")?;

    if files.is_empty() {
        if args.json {
            println!(
                "{}",
                serde_json::json!({
                    "tool": "vectorize",
                    "processed": 0,
                    "failed": 0,
                    "files": [],
                    "warning": "No images found"
                })
            );
        } else {
            eprintln!("No images found in {}", args.input.display());
        }
        return Ok(());
    }

    if args.dry_run {
        for f in &files {
            println!("[dry-run] would vectorize: {}", f.display());
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

    // Parallel per file — vtracer is CPU-bound and re-entrant.
    let start = Instant::now();
    let results: Vec<FileResult> = files
        .par_iter()
        .map(|input_path| {
            let result = process_one(input_path, &args.output, &opts, args.overwrite);
            pb.inc(1);
            if let Some(name) = input_path.file_name() {
                pb.set_message(name.to_string_lossy().into_owned());
            }
            FileResult {
                input: input_path.clone(),
                output: result.as_ref().ok().cloned(),
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
            "tool": "vectorize",
            "processed": processed,
            "failed": failed,
            "duration_ms": duration_ms,
            "files": results.iter().map(|r| {
                serde_json::json!({
                    "input": r.input,
                    "output": r.output,
                    "status": if r.error.is_none() { "ok" } else { "failed" },
                    "error": r.error,
                })
            }).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        println!(
            "✓ Processed {}/{} files in {}ms",
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
    error: Option<String>,
}

fn process_one(
    input_path: &Path,
    output_dir: &Path,
    opts: &vectorize::Options,
    overwrite: bool,
) -> Result<PathBuf> {
    let stem = input_path
        .file_stem()
        .ok_or_else(|| anyhow!("Invalid filename: {}", input_path.display()))?;
    let output_path = output_dir.join(format!("{}.svg", stem.to_string_lossy()));

    if !overwrite && output_path.exists() {
        return Err(anyhow!(
            "Output file exists (use --overwrite): {}",
            output_path.display()
        ));
    }

    vectorize::process(input_path, &output_path, opts)
        .with_context(|| format!("Vectorizing {}", input_path.display()))?;

    Ok(output_path)
}

fn load_options_from_config(path: &Path) -> Result<vectorize::Options> {
    let preset = preset::load_from_path(path)
        .with_context(|| format!("Loading preset {}", path.display()))?;
    preset::ensure_tool(&preset, preset::TOOL_VECTORIZE)
        .with_context(|| format!("Preset {} is not a vectorize preset", path.display()))?;
    serde_json::from_value(preset.options)
        .with_context(|| format!("Decoding vectorize options from {}", path.display()))
}

fn parse_mode(s: &str) -> Result<vectorize::Mode> {
    match s.to_ascii_lowercase().as_str() {
        "color" => Ok(vectorize::Mode::Color),
        "binary" => Ok(vectorize::Mode::Binary),
        other => Err(anyhow!(
            "Unsupported mode: {} (expected color|binary)",
            other
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_mode_color() {
        assert_eq!(parse_mode("color").unwrap(), vectorize::Mode::Color);
    }

    #[test]
    fn parse_mode_binary() {
        assert_eq!(parse_mode("binary").unwrap(), vectorize::Mode::Binary);
    }

    #[test]
    fn parse_mode_uppercase() {
        assert_eq!(parse_mode("COLOR").unwrap(), vectorize::Mode::Color);
    }

    #[test]
    fn parse_mode_unknown() {
        assert!(parse_mode("vector").is_err());
    }
}
