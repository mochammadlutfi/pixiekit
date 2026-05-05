//! Pixiekit core library — pure logic for asset preparation tools.
//!
//! Public modules are re-exported at the crate root for convenience.
//! Frontends (CLI, MCP, future Tauri/Nuxt) consume this crate; the core itself
//! never imports from frontend code.

pub mod batch;
pub mod bg_remove;
pub mod error;
pub mod vectorize;
pub mod video_to_sprite;

pub use error::{Error, Result};
