//! Pixiekit core library — pure logic for asset preparation tools.
//!
//! Public modules are re-exported at the crate root for convenience.
//! Frontends (CLI, MCP, future Tauri/Nuxt) consume this crate; the core itself
//! never imports from frontend code.

pub mod atlas_pack;
pub mod audio;
pub mod batch;
pub mod bg_remove;
pub mod error;
pub mod optimize;
pub mod posterize;
pub mod preset;
pub mod recent;
pub mod scale;
pub mod svg_optimize;
pub mod trim_pad;
pub mod vectorize;
pub mod video_to_sprite;
pub mod nine_slice;
pub mod anim_preview;

#[cfg(test)]
pub(crate) mod test_util;

pub use error::{Error, Result};
