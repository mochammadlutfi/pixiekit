//! Preset subcommand — save / load / list reusable tool configurations.
//!
//! Presets are JSON files under `~/.config/pixiekit/presets/<name>.json` (PRD
//! §9.1). Each preset records the tool it belongs to plus an opaque `options`
//! object that mirrors the tool's `Options` struct from `pixiekit_core`.
//!
//! Save accepts tool options either as inline flags (the same `--fuzz`,
//! `--erode`, … the actual tool subcommand uses) or as a JSON file via
//! `--from`. Load is performed by the actual tool subcommands via their
//! `--config <PATH>` flag.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use clap::{Args as ClapArgs, Subcommand};

use pixiekit_core::{audio, bg_remove, preset, vectorize, video_to_sprite};

#[derive(ClapArgs, Debug)]
pub struct Args {
    #[command(subcommand)]
    pub command: PresetCommand,
}

#[derive(Subcommand, Debug)]
pub enum PresetCommand {
    /// Save a new preset (overwrites if it exists)
    Save(SaveArgs),

    /// List all saved presets
    List(ListArgs),

    /// Print a saved preset (pretty JSON)
    Show(ShowArgs),

    /// Delete a saved preset
    Delete(DeleteArgs),

    /// Print the on-disk path for a preset name (without checking existence)
    Path(PathArgs),
}

#[derive(ClapArgs, Debug)]
pub struct SaveArgs {
    /// Preset name (letters, digits, dash, underscore — used as filename)
    pub name: String,

    #[command(subcommand)]
    pub tool: SaveTool,
}

#[derive(Subcommand, Debug)]
pub enum SaveTool {
    /// Save BG-remove options
    BgRemove(SaveBgRemoveArgs),
    /// Save vectorize options
    Vectorize(SaveVectorizeArgs),
    /// Save video-to-sprite options
    VideoToSprite(SaveVideoToSpriteArgs),
    /// Save audio options
    Audio(SaveAudioArgs),
}

#[derive(ClapArgs, Debug)]
pub struct SaveBgRemoveArgs {
    /// Read options from a JSON file instead of CLI flags (file shape: an
    /// `Options` struct, not a wrapped preset)
    #[arg(long)]
    pub from: Option<PathBuf>,

    #[arg(long, default_value = "#00FF00")]
    pub target_color: String,
    #[arg(long, default_value_t = 0.35)]
    pub fuzz: f32,
    #[arg(long)]
    pub no_despill: bool,
    #[arg(long, default_value_t = 1)]
    pub erode: u8,
}

#[derive(ClapArgs, Debug)]
pub struct SaveVectorizeArgs {
    #[arg(long)]
    pub from: Option<PathBuf>,

    #[arg(long, default_value = "color")]
    pub mode: String,
    #[arg(long)]
    pub smooth: Option<u8>,
    #[arg(long, default_value_t = 4)]
    pub filter_speckle: u32,
    #[arg(long, default_value_t = 6)]
    pub color_precision: u8,
    #[arg(long, default_value_t = 16)]
    pub layer_difference: u8,
    #[arg(long, default_value_t = 60)]
    pub corner_threshold: u8,
    #[arg(long, default_value_t = 4.0)]
    pub length_threshold: f64,
    #[arg(long, default_value_t = 45)]
    pub splice_threshold: u8,
    #[arg(long, default_value_t = 8)]
    pub path_precision: u8,
}

#[derive(ClapArgs, Debug)]
pub struct SaveVideoToSpriteArgs {
    #[arg(long)]
    pub from: Option<PathBuf>,

    #[arg(long, default_value_t = 8)]
    pub fps: u8,
    #[arg(long, default_value_t = 256)]
    pub size: u32,
    #[arg(long, default_value = "webp")]
    pub format: String,
    #[arg(long, default_value_t = 90)]
    pub webp_quality: u8,
    #[arg(long)]
    pub chroma_key: bool,
    #[arg(long, default_value = "#00FF00")]
    pub chroma_target: String,
    #[arg(long, default_value_t = 0.35)]
    pub chroma_fuzz: f32,
    #[arg(long)]
    pub no_despill: bool,
    #[arg(long, default_value_t = 1)]
    pub chroma_erode: u8,
}

#[derive(ClapArgs, Debug)]
pub struct SaveAudioArgs {
    #[arg(long)]
    pub from: Option<PathBuf>,

    #[arg(long, default_value = "ogg")]
    pub target_format: String,
    #[arg(long, default_value_t = -16.0)]
    pub target_lufs: f32,
    #[arg(long)]
    pub no_normalize: bool,
    #[arg(long)]
    pub no_trim_silence: bool,
    #[arg(long, default_value_t = -50.0)]
    pub silence_threshold_db: f32,
    #[arg(long, default_value_t = 44_100)]
    pub sample_rate: u32,
    #[arg(long, default_value = "keep")]
    pub channels: String,
    #[arg(long, default_value_t = 128)]
    pub bitrate_kbps: u16,
}

#[derive(ClapArgs, Debug)]
pub struct ListArgs {
    /// JSON output (for AI / scripting)
    #[arg(long)]
    pub json: bool,
}

#[derive(ClapArgs, Debug)]
pub struct ShowArgs {
    pub name: String,
}

#[derive(ClapArgs, Debug)]
pub struct DeleteArgs {
    pub name: String,
}

#[derive(ClapArgs, Debug)]
pub struct PathArgs {
    pub name: String,
}

pub fn run(args: Args) -> Result<()> {
    match args.command {
        PresetCommand::Save(a) => run_save(a),
        PresetCommand::List(a) => run_list(a),
        PresetCommand::Show(a) => run_show(a),
        PresetCommand::Delete(a) => run_delete(a),
        PresetCommand::Path(a) => run_path(a),
    }
}

fn run_save(args: SaveArgs) -> Result<()> {
    let (tool, options) = match args.tool {
        SaveTool::BgRemove(a) => (preset::TOOL_BG_REMOVE, build_bg_remove_options(a)?),
        SaveTool::Vectorize(a) => (preset::TOOL_VECTORIZE, build_vectorize_options(a)?),
        SaveTool::VideoToSprite(a) => (
            preset::TOOL_VIDEO_TO_SPRITE,
            build_video_to_sprite_options(a)?,
        ),
        SaveTool::Audio(a) => (preset::TOOL_AUDIO, build_audio_options(a)?),
    };

    let path = preset::save(&args.name, tool, options).context("Saving preset")?;
    println!("Saved preset '{}' to {}", args.name, path.display());
    Ok(())
}

fn build_bg_remove_options(a: SaveBgRemoveArgs) -> Result<serde_json::Value> {
    if let Some(from) = a.from {
        return read_options_json::<bg_remove::Options>(&from);
    }
    let target_color = parse_hex_color(&a.target_color)
        .with_context(|| format!("Invalid --target-color: {}", a.target_color))?;
    let opts = bg_remove::Options {
        target_color,
        fuzz: a.fuzz,
        despill: !a.no_despill,
        erode: a.erode,
    };
    Ok(serde_json::to_value(opts)?)
}

fn build_vectorize_options(a: SaveVectorizeArgs) -> Result<serde_json::Value> {
    if let Some(from) = a.from {
        return read_options_json::<vectorize::Options>(&from);
    }
    let mode = parse_vectorize_mode(&a.mode)?;
    let (corner_threshold, length_threshold, splice_threshold) = match a.smooth {
        Some(s) => vectorize::smooth_to_params(s),
        None => (a.corner_threshold, a.length_threshold, a.splice_threshold),
    };
    let opts = vectorize::Options {
        mode,
        filter_speckle: a.filter_speckle,
        color_precision: a.color_precision,
        layer_difference: a.layer_difference,
        corner_threshold,
        length_threshold,
        splice_threshold,
        path_precision: a.path_precision,
        posterize: None,
    };
    Ok(serde_json::to_value(opts)?)
}

fn build_video_to_sprite_options(a: SaveVideoToSpriteArgs) -> Result<serde_json::Value> {
    if let Some(from) = a.from {
        return read_options_json::<video_to_sprite::Options>(&from);
    }
    let format = parse_video_format(&a.format)?;
    let chroma_key = if a.chroma_key {
        Some(bg_remove::Options {
            target_color: parse_hex_color(&a.chroma_target)
                .with_context(|| format!("Invalid --chroma-target: {}", a.chroma_target))?,
            fuzz: a.chroma_fuzz,
            despill: !a.no_despill,
            erode: a.chroma_erode,
        })
    } else {
        None
    };
    let opts = video_to_sprite::Options {
        fps: a.fps,
        frame_size: a.size,
        output_format: format,
        webp_quality: a.webp_quality,
        chroma_key,
    };
    Ok(serde_json::to_value(opts)?)
}

fn build_audio_options(a: SaveAudioArgs) -> Result<serde_json::Value> {
    if let Some(from) = a.from {
        return read_options_json::<audio::Options>(&from);
    }
    let target_format = crate::commands::audio::parse_target_format(&a.target_format)
        .with_context(|| format!("Invalid --target-format: {}", a.target_format))?;
    let channels = crate::commands::audio::parse_channels(&a.channels)
        .with_context(|| format!("Invalid --channels: {}", a.channels))?;
    let opts = audio::Options {
        target_format,
        target_lufs: a.target_lufs,
        normalize: !a.no_normalize,
        trim_silence: !a.no_trim_silence,
        silence_threshold_db: a.silence_threshold_db,
        sample_rate: a.sample_rate,
        channels,
        bitrate_kbps: a.bitrate_kbps,
    };
    Ok(serde_json::to_value(opts)?)
}

fn read_options_json<T>(path: &Path) -> Result<serde_json::Value>
where
    T: serde::de::DeserializeOwned + serde::Serialize,
{
    let bytes = std::fs::read(path).with_context(|| format!("Reading {}", path.display()))?;
    let opts: T = serde_json::from_slice(&bytes)
        .with_context(|| format!("Parsing options JSON from {}", path.display()))?;
    Ok(serde_json::to_value(opts)?)
}

fn run_list(args: ListArgs) -> Result<()> {
    let names = preset::list().context("Listing presets")?;
    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "presets": names,
            }))?
        );
    } else if names.is_empty() {
        let dir = preset::presets_dir().unwrap_or_default();
        println!("No presets found in {}", dir.display());
    } else {
        for n in &names {
            println!("{n}");
        }
    }
    Ok(())
}

fn run_show(args: ShowArgs) -> Result<()> {
    let preset = preset::load(&args.name).context("Loading preset")?;
    println!("{}", serde_json::to_string_pretty(&preset)?);
    Ok(())
}

fn run_delete(args: DeleteArgs) -> Result<()> {
    let path = preset::delete(&args.name).context("Deleting preset")?;
    println!("Deleted preset '{}' ({})", args.name, path.display());
    Ok(())
}

fn run_path(args: PathArgs) -> Result<()> {
    let path = preset::path_for(&args.name).context("Resolving preset path")?;
    println!("{}", path.display());
    Ok(())
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

fn parse_vectorize_mode(s: &str) -> Result<vectorize::Mode> {
    match s.to_ascii_lowercase().as_str() {
        "color" => Ok(vectorize::Mode::Color),
        "binary" => Ok(vectorize::Mode::Binary),
        other => Err(anyhow!(
            "Unsupported mode: {} (expected color|binary)",
            other
        )),
    }
}

fn parse_video_format(s: &str) -> Result<video_to_sprite::OutputFormat> {
    match s.to_ascii_lowercase().as_str() {
        "png" => Ok(video_to_sprite::OutputFormat::Png),
        "webp" => Ok(video_to_sprite::OutputFormat::Webp),
        other => Err(anyhow!("Unsupported format: {} (expected png|webp)", other)),
    }
}
