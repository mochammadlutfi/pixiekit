use anyhow::Result;
use clap::{Parser, Subcommand};

mod commands;

#[derive(Parser)]
#[command(
    name = "pixiekit-cli",
    version,
    about = "Asset preparation toolkit — CLI",
    long_about = "Pixiekit CLI: chroma-key BG removal, raster→SVG vectorize, video→sprite sheet.\n\
                  See https://github.com/mochammadlutfi/pixiekit"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Remove background (chroma key + despill + erode)
    BgRemove(commands::bg_remove::Args),

    /// Trace raster image to SVG via vtracer
    Vectorize(commands::vectorize::Args),

    /// Extract video frames into a horizontal sprite sheet
    VideoToSprite(commands::video_to_sprite::Args),

    /// Pack PNG sprites into a texture atlas with TexturePacker JSON metadata
    AtlasPack(commands::atlas_pack::Args),

    /// Optimize PNG/JPG/WebP images (oxipng / format convert / metadata strip)
    Optimize(commands::optimize::Args),

    /// Generate multi-resolution variants (Flutter / @suffix / nested)
    Scale(commands::scale::Args),

    /// Process audio (LUFS normalize, trim silence, format convert via ffmpeg)
    Audio(commands::audio::Args),

    /// Auto-crop transparent borders and add uniform padding
    TrimPad(commands::trim_pad::Args),

    /// Minify SVG (round coords, strip metadata, drop hidden elements)
    SvgOptimize(commands::svg_optimize::Args),

    /// Split image into 9 tiles or generate Flame metadata
    NineSlice(commands::nine_slice::Args),

    /// Generate animation preview (GIF/MP4/WebM) from sprite sheet or frames
    AnimPreview(commands::anim_preview::Args),

    /// Manage saved presets (save / list / show / delete)
    Preset(commands::preset::Args),
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::BgRemove(args) => commands::bg_remove::run(args),
        Commands::Vectorize(args) => commands::vectorize::run(args),
        Commands::VideoToSprite(args) => commands::video_to_sprite::run(args),
        Commands::AtlasPack(args) => commands::atlas_pack::run(args),
        Commands::Optimize(args) => commands::optimize::run(args),
        Commands::Scale(args) => commands::scale::run(args),
        Commands::Audio(args) => commands::audio::run(args),
        Commands::TrimPad(args) => commands::trim_pad::run(args),
        Commands::SvgOptimize(args) => commands::svg_optimize::run(args),
        Commands::NineSlice(args) => commands::nine_slice::run(args),
        Commands::AnimPreview(args) => commands::anim_preview::run(args),
        Commands::Preset(args) => commands::preset::run(args),
    }
}
