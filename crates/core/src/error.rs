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

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Path not found: {0}")]
    NotFound(PathBuf),
}
