use std::path::PathBuf;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Image error: {0}")]
    Image(#[from] image::ImageError),

    #[error("Walkdir error: {0}")]
    Walk(#[from] walkdir::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Path not found: {0}")]
    NotFound(PathBuf),

    #[error("ffmpeg not found in PATH — install via Homebrew: `brew install ffmpeg`")]
    FfmpegMissing,

    #[error("ffmpeg failed (exit {code}): {stderr}")]
    FfmpegFailed { code: i32, stderr: String },

    #[error("WebP encoding failed: {0}")]
    WebpEncode(String),

    #[error("No frames extracted from video: {0}")]
    NoFrames(PathBuf),

    #[error("Inconsistent frame size: expected {expected}x{expected}, got {got_w}x{got_h}")]
    InconsistentFrameSize {
        expected: u32,
        got_w: u32,
        got_h: u32,
    },

    #[error("vtracer failed: {0}")]
    VtracerFailed(String),
}
