use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{anyhow, Context, Result};
use clap::Args as ClapArgs;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;

use pixiekit_core::{batch, preset, scale};

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Input image file or folder
    #[arg(short, long)]
    pub input: PathBuf,

    /// Output folder (created if missing)
    #[arg(short, long)]
    pub output: PathBuf,

    /// Density of the source artwork (e.g. 4 for `4x` originals)
    #[arg(long, default_value_t = 4.0)]
    pub base_scale: f32,

    /// Comma-separated densities to emit (e.g. "1,1.5,2,3")
    #[arg(long, default_value = "1.0,1.5,2.0,3.0")]
    pub target_scales: String,

    /// Output naming layout (flutter|suffix|nested)
    #[arg(long, default_value = "flutter")]
    pub naming: String,

    /// Resampling filter (lanczos|bilinear|nearest)
    #[arg(long, default_value = "lanczos")]
    pub filter: String,

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
        None => scale::Options {
            base_scale: args.base_scale,
            target_scales: parse_scales(&args.target_scales)?,
            naming: parse_naming(&args.naming)?,
            filter: parse_filter(&args.filter)?,
        },
    };

    let files = batch::list_images(&args.input, args.recursive, &["png", "jpg", "jpeg", "webp"])
        .context("Listing input files")?;

    if files.is_empty() {
        if args.json {
            println!(
                "{}",
                serde_json::json!({
                    "tool": "scale",
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
            for s in &opts.target_scales {
                println!("[dry-run] would scale {} → {}x", f.display(), s);
            }
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
                Ok(report) => FileResult {
                    input: input_path.clone(),
                    variants: report.variants,
                    error: None,
                },
                Err(e) => FileResult {
                    input: input_path.clone(),
                    variants: Vec::new(),
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
            "tool": "scale",
            "processed": processed,
            "failed": failed,
            "duration_ms": duration_ms,
            "files": results.iter().map(|r| {
                serde_json::json!({
                    "input": r.input,
                    "variants": r.variants,
                    "status": if r.error.is_none() { "ok" } else { "failed" },
                    "error": r.error,
                })
            }).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        let total_variants: usize = results.iter().map(|r| r.variants.len()).sum();
        println!(
            "✓ Scaled {}/{} files into {} variants in {}ms",
            processed,
            files.len(),
            total_variants,
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
    variants: Vec<PathBuf>,
    error: Option<String>,
}

fn process_one(
    input_path: &Path,
    output_dir: &Path,
    opts: &scale::Options,
    overwrite: bool,
) -> Result<scale::ScaleReport> {
    if !overwrite {
        // Check expected variant paths up-front so we don't half-write.
        for &s in &opts.target_scales {
            let p = expected_variant_path(input_path, output_dir, s, opts.naming)?;
            if p.exists() {
                return Err(anyhow!(
                    "Output file exists (use --overwrite): {}",
                    p.display()
                ));
            }
        }
    }
    scale::process(input_path, output_dir, opts)
        .with_context(|| format!("Scaling {}", input_path.display()))
}

fn expected_variant_path(
    input_path: &Path,
    output_dir: &Path,
    target_scale: f32,
    naming: scale::NamingMode,
) -> Result<PathBuf> {
    let stem = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow!("Invalid filename: {}", input_path.display()))?;
    let ext = input_path
        .extension()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow!("Input has no extension: {}", input_path.display()))?;
    let label = format_scale_label(target_scale);
    let path = match naming {
        scale::NamingMode::Flutter => output_dir
            .join(format!("{label}x"))
            .join(format!("{stem}.{ext}")),
        scale::NamingMode::Suffix => {
            if (target_scale - 1.0).abs() < f32::EPSILON {
                output_dir.join(format!("{stem}.{ext}"))
            } else {
                output_dir.join(format!("{stem}@{label}x.{ext}"))
            }
        }
        scale::NamingMode::Nested => output_dir.join(label).join(format!("{stem}.{ext}")),
    };
    Ok(path)
}

fn format_scale_label(scale: f32) -> String {
    if scale.fract().abs() < f32::EPSILON {
        format!("{}", scale.round() as i32)
    } else {
        let s = format!("{scale:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn load_options_from_config(path: &Path) -> Result<scale::Options> {
    let preset = preset::load_from_path(path)
        .with_context(|| format!("Loading preset {}", path.display()))?;
    preset::ensure_tool(&preset, preset::TOOL_SCALE)
        .with_context(|| format!("Preset {} is not a scale preset", path.display()))?;
    serde_json::from_value(preset.options)
        .with_context(|| format!("Decoding scale options from {}", path.display()))
}

fn parse_scales(s: &str) -> Result<Vec<f32>> {
    let mut out = Vec::new();
    for part in s.split(',') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        let v: f32 = trimmed
            .parse()
            .with_context(|| format!("Invalid scale: {trimmed}"))?;
        if v <= 0.0 {
            return Err(anyhow!("Scale must be > 0, got {v}"));
        }
        out.push(v);
    }
    if out.is_empty() {
        return Err(anyhow!("--target-scales must list at least one density"));
    }
    Ok(out)
}

fn parse_naming(s: &str) -> Result<scale::NamingMode> {
    match s.to_ascii_lowercase().as_str() {
        "flutter" => Ok(scale::NamingMode::Flutter),
        "suffix" => Ok(scale::NamingMode::Suffix),
        "nested" => Ok(scale::NamingMode::Nested),
        other => Err(anyhow!(
            "Unsupported naming: {other} (expected flutter|suffix|nested)"
        )),
    }
}

fn parse_filter(s: &str) -> Result<scale::Filter> {
    match s.to_ascii_lowercase().as_str() {
        "lanczos" => Ok(scale::Filter::Lanczos),
        "bilinear" => Ok(scale::Filter::Bilinear),
        "nearest" => Ok(scale::Filter::Nearest),
        other => Err(anyhow!(
            "Unsupported filter: {other} (expected lanczos|bilinear|nearest)"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_default_scales() {
        let v = parse_scales("1.0,1.5,2.0,3.0").unwrap();
        assert_eq!(v, vec![1.0, 1.5, 2.0, 3.0]);
    }

    #[test]
    fn parses_with_whitespace() {
        let v = parse_scales(" 1, 2 , 3 ").unwrap();
        assert_eq!(v, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn rejects_empty_scales() {
        assert!(parse_scales("").is_err());
        assert!(parse_scales(", , ").is_err());
    }

    #[test]
    fn rejects_zero_or_negative() {
        assert!(parse_scales("0").is_err());
        assert!(parse_scales("-1").is_err());
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_scales("1,foo,2").is_err());
    }

    #[test]
    fn parses_naming_flutter() {
        assert_eq!(parse_naming("flutter").unwrap(), scale::NamingMode::Flutter);
    }

    #[test]
    fn parses_naming_suffix_uppercase() {
        assert_eq!(parse_naming("SUFFIX").unwrap(), scale::NamingMode::Suffix);
    }

    #[test]
    fn rejects_unknown_naming() {
        assert!(parse_naming("xcode").is_err());
    }

    #[test]
    fn parses_filters() {
        assert_eq!(parse_filter("lanczos").unwrap(), scale::Filter::Lanczos);
        assert_eq!(parse_filter("bilinear").unwrap(), scale::Filter::Bilinear);
        assert_eq!(parse_filter("nearest").unwrap(), scale::Filter::Nearest);
    }

    #[test]
    fn rejects_unknown_filter() {
        assert!(parse_filter("bicubic").is_err());
    }
}
