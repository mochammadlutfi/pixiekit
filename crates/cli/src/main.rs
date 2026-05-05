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

    /// Manage saved presets (save / list / show / delete)
    Preset(commands::preset::Args),
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::BgRemove(args) => commands::bg_remove::run(args),
        Commands::Vectorize(args) => commands::vectorize::run(args),
        Commands::VideoToSprite(args) => commands::video_to_sprite::run(args),
        Commands::Preset(args) => commands::preset::run(args),
    }
}
