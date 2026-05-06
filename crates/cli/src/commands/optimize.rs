use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{anyhow, Context, Result};
use clap::Args as ClapArgs;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;

use pixiekit_core::{batch, optimize, preset};

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Input image file or folder
    #[arg(short, long)]
    pub input: PathBuf,

    /// Output folder (created if missing)
    #[arg(short, long)]
    pub output: PathBuf,

    /// Target format (png|webp|keep)
    #[arg(long, default_value = "webp")]
    pub format: String,

    /// Lossy quality 0-100 (ignored when --lossless is set)
    #[arg(long, default_value_t = 90)]
    pub quality: u8,

    /// Use lossless WebP encoding
    #[arg(long)]
    pub lossless: bool,

    /// Keep metadata chunks (default: stripped)
    #[arg(long)]
    pub keep_metadata: bool,

    /// oxipng optimization level 0-6 (higher = smaller, slower)
    #[arg(long, default_value_t = 3)]
    pub optimization_level: u8,

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
        None => optimize::Options {
            target_format: parse_format(&args.format)?,
            quality: args.quality,
            lossless: args.lossless,
            strip_metadata: !args.keep_metadata,
            optimization_level: args.optimization_level,
        },
    };

    let files = batch::list_images(&args.input, args.recursive, &["png", "jpg", "jpeg", "webp"])
        .context("Listing input files")?;

    if files.is_empty() {
        if args.json {
            println!(
                "{}",
                serde_json::json!({
                    "tool": "optimize",
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
                Ok(r) => FileResult {
                    input: input_path.clone(),
                    output: Some(r.output_path),
                    input_size: Some(r.input_size),
                    output_size: Some(r.output_size),
                    ratio: Some(r.ratio),
                    error: None,
                },
                Err(e) => FileResult {
                    input: input_path.clone(),
                    output: None,
                    input_size: None,
                    output_size: None,
                    ratio: None,
                    error: Some(format!("{e:#}")),
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
            "tool": "optimize",
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
        let saved: i128 = results
            .iter()
            .map(|r| {
                let inp = r.input_size.unwrap_or(0) as i128;
                let outp = r.output_size.unwrap_or(0) as i128;
                inp - outp
            })
            .sum();
        println!(
            "✓ Optimized {}/{} files in {}ms (saved {} bytes)",
            processed,
            files.len(),
            duration_ms,
            saved
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
    opts: &optimize::Options,
    overwrite: bool,
) -> Result<optimize::OptimizeReport> {
    let stem = input_path
        .file_stem()
        .ok_or_else(|| anyhow!("Invalid filename: {}", input_path.display()))?;
    // Pass a path *without* extension; `optimize::process` attaches the
    // resolved extension. Existing-file check uses the resolved path.
    let target_stub = output_dir.join(stem);

    let resolved_ext = match opts.target_format {
        optimize::TargetFormat::Png => "png".to_string(),
        optimize::TargetFormat::Webp => "webp".to_string(),
        optimize::TargetFormat::Keep => input_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| {
                let lc = s.to_ascii_lowercase();
                if lc == "jpeg" {
                    "jpg".to_string()
                } else {
                    lc
                }
            })
            .unwrap_or_else(|| "png".to_string()),
    };
    let resolved_path = output_dir.join(format!("{}.{}", stem.to_string_lossy(), resolved_ext));

    if !overwrite && resolved_path.exists() {
        return Err(anyhow!(
            "Output file exists (use --overwrite): {}",
            resolved_path.display()
        ));
    }

    optimize::process(input_path, &target_stub, opts)
        .with_context(|| format!("Optimizing {}", input_path.display()))
}

fn load_options_from_config(path: &Path) -> Result<optimize::Options> {
    let preset = preset::load_from_path(path)
        .with_context(|| format!("Loading preset {}", path.display()))?;
    preset::ensure_tool(&preset, preset::TOOL_OPTIMIZE)
        .with_context(|| format!("Preset {} is not an optimize preset", path.display()))?;
    serde_json::from_value(preset.options)
        .with_context(|| format!("Decoding optimize options from {}", path.display()))
}

fn parse_format(s: &str) -> Result<optimize::TargetFormat> {
    match s.to_ascii_lowercase().as_str() {
        "png" => Ok(optimize::TargetFormat::Png),
        "webp" => Ok(optimize::TargetFormat::Webp),
        "keep" => Ok(optimize::TargetFormat::Keep),
        other => Err(anyhow!(
            "Unsupported format: {} (expected png|webp|keep)",
            other
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_format_png() {
        assert_eq!(parse_format("png").unwrap(), optimize::TargetFormat::Png);
    }

    #[test]
    fn parse_format_webp() {
        assert_eq!(parse_format("webp").unwrap(), optimize::TargetFormat::Webp);
    }

    #[test]
    fn parse_format_keep() {
        assert_eq!(parse_format("keep").unwrap(), optimize::TargetFormat::Keep);
    }

    #[test]
    fn parse_format_uppercase() {
        assert_eq!(parse_format("WEBP").unwrap(), optimize::TargetFormat::Webp);
    }

    #[test]
    fn parse_format_unknown_errors() {
        assert!(parse_format("avif").is_err());
    }
}
