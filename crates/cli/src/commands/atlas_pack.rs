use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{anyhow, Context, Result};
use clap::Args as ClapArgs;

use pixiekit_core::{atlas_pack, batch, preset};

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Folder containing PNG sprites
    #[arg(short, long)]
    pub input: PathBuf,

    /// Output folder (atlas image + JSON written here)
    #[arg(short, long)]
    pub output: PathBuf,

    /// Atlas basename
    #[arg(long, default_value = "atlas")]
    pub name: String,

    /// Max texture dimension (256 - 8192)
    #[arg(long, default_value_t = 2048)]
    pub max_size: u16,

    /// Pixel padding between sprites (0 - 16)
    #[arg(long, default_value_t = 2)]
    pub padding: u8,

    /// Edge bleed prevention pixels (0 - 4)
    #[arg(long, default_value_t = 1)]
    pub extrude: u8,

    /// Disable power-of-two atlas dimensions (default: enabled)
    #[arg(long)]
    pub no_power_of_two: bool,

    /// Disable auto-trim of transparent borders (default: enabled)
    #[arg(long)]
    pub no_trim: bool,

    /// Output format (png | webp)
    #[arg(long, default_value = "png")]
    pub format: String,

    /// WebP quality 0-100 (alpha is always lossless)
    #[arg(long, default_value_t = 90)]
    pub webp_quality: u8,

    /// Recursive folder scan
    #[arg(short, long)]
    pub recursive: bool,

    /// Overwrite existing atlas / metadata
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
            let format = parse_format(&args.format)
                .with_context(|| format!("Invalid --format: {}", args.format))?;
            atlas_pack::Options {
                name: args.name.clone(),
                max_size: args.max_size,
                padding: args.padding,
                extrude: args.extrude,
                power_of_two: !args.no_power_of_two,
                trim: !args.no_trim,
                format,
                webp_quality: args.webp_quality,
            }
        }
    };

    let sprites =
        batch::list_images(&args.input, args.recursive, &["png"]).context("Listing sprites")?;

    if sprites.is_empty() {
        if args.json {
            println!(
                "{}",
                serde_json::json!({
                    "tool": "atlas-pack",
                    "processed": 0,
                    "failed": 0,
                    "warning": "No PNG sprites found",
                })
            );
        } else {
            eprintln!("No PNG sprites found in {}", args.input.display());
        }
        return Ok(());
    }

    if args.dry_run {
        for s in &sprites {
            println!("[dry-run] would pack: {}", s.display());
        }
        return Ok(());
    }

    std::fs::create_dir_all(&args.output)
        .with_context(|| format!("Creating output dir {}", args.output.display()))?;

    let atlas_path = args
        .output
        .join(format!("{}.{}", opts.name, opts.format.extension()));
    let metadata_path = args.output.join(format!("{}.json", opts.name));

    if !args.overwrite && (atlas_path.exists() || metadata_path.exists()) {
        return Err(anyhow!(
            "Atlas output exists (use --overwrite): {}",
            atlas_path.display()
        ));
    }

    let start = Instant::now();
    let report = atlas_pack::process(&sprites, &args.output, &opts)
        .with_context(|| format!("Packing {} sprites", sprites.len()))?;
    let duration_ms = start.elapsed().as_millis();

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "tool": "atlas-pack",
                "processed": report.packed,
                "failed": report.total - report.packed,
                "duration_ms": duration_ms,
                "atlas_path": report.atlas_path,
                "metadata_path": report.metadata_path,
                "atlas_size": { "w": report.atlas_size.0, "h": report.atlas_size.1 },
                "efficiency": report.efficiency,
            }))?
        );
    } else {
        println!(
            "✓ Packed {}/{} sprites into {}×{} atlas ({}% efficiency) in {}ms",
            report.packed,
            report.total,
            report.atlas_size.0,
            report.atlas_size.1,
            (report.efficiency * 100.0) as u32,
            duration_ms
        );
        println!("  atlas:    {}", report.atlas_path.display());
        println!("  metadata: {}", report.metadata_path.display());
    }
    Ok(())
}

fn load_options_from_config(path: &Path) -> Result<atlas_pack::Options> {
    let preset = preset::load_from_path(path)
        .with_context(|| format!("Loading preset {}", path.display()))?;
    preset::ensure_tool(&preset, preset::TOOL_ATLAS_PACK)
        .with_context(|| format!("Preset {} is not an atlas-pack preset", path.display()))?;
    serde_json::from_value(preset.options)
        .with_context(|| format!("Decoding atlas-pack options from {}", path.display()))
}

fn parse_format(s: &str) -> Result<atlas_pack::OutputFormat> {
    match s.to_ascii_lowercase().as_str() {
        "png" => Ok(atlas_pack::OutputFormat::Png),
        "webp" => Ok(atlas_pack::OutputFormat::Webp),
        other => Err(anyhow!("Unsupported format: {} (expected png|webp)", other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_format_lowercase() {
        assert_eq!(parse_format("png").unwrap(), atlas_pack::OutputFormat::Png);
        assert_eq!(
            parse_format("webp").unwrap(),
            atlas_pack::OutputFormat::Webp
        );
    }

    #[test]
    fn parse_format_uppercase() {
        assert_eq!(parse_format("PNG").unwrap(), atlas_pack::OutputFormat::Png);
    }

    #[test]
    fn parse_format_unknown() {
        assert!(parse_format("gif").is_err());
    }
}
