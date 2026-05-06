pub mod atlas_pack;
pub mod audio;
pub mod bg_remove;
pub mod optimize;
pub mod preset;
pub mod scale;
pub mod svg_optimize;
pub mod trim_pad;
pub mod vectorize;
pub mod video_to_sprite;
pub mod nine_slice;
pub mod anim_preview;

use std::path::Path;

use anyhow::{anyhow, Result};

/// Validate the user-supplied input path before doing any work. Returns a
/// friendly error explaining what to fix instead of a bare `NotFound`/`Other`
/// I/O error halfway through batch traversal.
pub(crate) fn preflight_input(path: &Path) -> Result<()> {
    let display = path.display();
    if !path.exists() {
        return Err(anyhow!(
            "Input path does not exist: {display}\n  hint: check spelling and that the path is absolute or relative to the current directory"
        ));
    }
    let meta = std::fs::metadata(path)
        .map_err(|e| anyhow!("Cannot read {display}: {e}\n  hint: check permissions"))?;
    if !meta.is_file() && !meta.is_dir() {
        return Err(anyhow!(
            "Input is not a file or directory: {display}\n  hint: symlinks must resolve to a regular file or folder"
        ));
    }
    Ok(())
}
