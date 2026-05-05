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
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::BgRemove(args) => commands::bg_remove::run(args),
    }
}
