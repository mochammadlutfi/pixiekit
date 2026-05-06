use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{anyhow, Context, Result};
use clap::Args as ClapArgs;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;

use pixiekit_core::{batch, preset, trim_pad};

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Input image file or folder
    #[arg(short, long)]
    pub input: PathBuf,

    /// Output folder (created if missing)
    #[arg(short, long)]
    pub output: PathBuf,

    /// Alpha threshold: pixels with alpha > this count as content (0-255)
    #[arg(long, default_value_t = 1)]
    pub alpha_threshold: u8,

    /// Pad uniform N pixels (transparent) on every side after trim
    #[arg(long, default_value_t = 0)]
    pub padding: u16,

    /// Pad shorter dimension symmetrically to make a square output
    #[arg(long)]
    pub keep_square: bool,

    /// Background colour for non-alpha trimming, hex (e.g. #00FF00)
    #[arg(long)]
    pub bg_color: Option<String>,

    /// Tolerance fraction for --bg-color (0.0 - 1.0)
    #[arg(long, default_value_t = 0.05)]
    pub bg_tolerance: f32,

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
            let bg_color = match &args.bg_color {
                Some(s) => Some(
                    parse_hex_color(s)
                        .with_context(|| format!("Invalid --bg-color: {} (expected #RRGGBB)", s))?,
                ),
                None => None,
            };
            trim_pad::Options {
                alpha_threshold: args.alpha_threshold,
                padding: args.padding,
                keep_square: args.keep_square,
                bg_color,
                bg_tolerance: args.bg_tolerance,
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
                    "tool": "trim-pad",
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
            println!("[dry-run] would trim+pad: {}", f.display());
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
                    output_size: Some(report.output_size),
                    bbox: Some(report.bbox),
                    error: None,
                },
                Err(e) => FileResult {
                    input: input_path.clone(),
                    output: None,
                    output_size: None,
                    bbox: None,
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
            "tool": "trim-pad",
            "processed": processed,
            "failed": failed,
            "duration_ms": duration_ms,
            "files": results.iter().map(|r| {
                serde_json::json!({
                    "input": r.input,
                    "output": r.output,
                    "output_size": r.output_size,
                    "bbox": r.bbox,
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
    output_size: Option<(u32, u32)>,
    bbox: Option<(u32, u32, u32, u32)>,
    error: Option<String>,
}

fn process_one(
    input_path: &Path,
    output_dir: &Path,
    opts: &trim_pad::Options,
    overwrite: bool,
) -> Result<(PathBuf, trim_pad::TrimReport)> {
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

    let report = trim_pad::process(input_path, &output_path, opts)
        .with_context(|| format!("Trim+pad {}", input_path.display()))?;
    Ok((output_path, report))
}

fn load_options_from_config(path: &Path) -> Result<trim_pad::Options> {
    let preset = preset::load_from_path(path)
        .with_context(|| format!("Loading preset {}", path.display()))?;
    preset::ensure_tool(&preset, preset::TOOL_TRIM_PAD)
        .with_context(|| format!("Preset {} is not a trim-pad preset", path.display()))?;
    serde_json::from_value(preset.options)
        .with_context(|| format!("Decoding trim-pad options from {}", path.display()))
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
    fn parse_hex_color_with_hash() {
        assert_eq!(parse_hex_color("#00FF00").unwrap(), [0, 255, 0]);
    }

    #[test]
    fn parse_hex_color_without_hash() {
        assert_eq!(parse_hex_color("ff0080").unwrap(), [255, 0, 128]);
    }

    #[test]
    fn parse_hex_color_invalid_length() {
        assert!(parse_hex_color("#FFF").is_err());
    }
}
