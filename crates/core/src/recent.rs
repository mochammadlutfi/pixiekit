//! Recent paths cache — tracks the last N input/output paths used per tool
//! (M12.7). Stored at `~/.config/pixiekit/recent.json` (override via
//! `PIXIEKIT_CONFIG_DIR`).
//!
//! Used primarily by the desktop GUI to pre-fill folder pickers.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::preset::config_dir;

/// Maximum entries kept per kind.
pub const MAX_ENTRIES: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Input,
    Output,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecentPaths {
    #[serde(default)]
    pub input: Vec<String>,
    #[serde(default)]
    pub output: Vec<String>,
}

fn recent_file() -> Result<PathBuf> {
    Ok(config_dir()?.join("recent.json"))
}

/// Load the recent-paths cache. Returns an empty cache if the file is missing
/// or unreadable (recent paths are best-effort UX, never a hard error).
pub fn load() -> RecentPaths {
    let Ok(path) = recent_file() else {
        return RecentPaths::default();
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return RecentPaths::default();
    };
    serde_json::from_str(&content).unwrap_or_default()
}

/// Add a path to the front of the relevant list, dedupe, cap at [`MAX_ENTRIES`].
pub fn add(kind: Kind, path: impl Into<String>) -> Result<RecentPaths> {
    let mut state = load();
    let value = path.into();
    if value.is_empty() {
        return Ok(state);
    }
    let list = match kind {
        Kind::Input => &mut state.input,
        Kind::Output => &mut state.output,
    };
    list.retain(|p| p != &value);
    list.insert(0, value);
    list.truncate(MAX_ENTRIES);
    save(&state)?;
    Ok(state)
}

/// Clear all entries for a kind.
pub fn clear(kind: Kind) -> Result<RecentPaths> {
    let mut state = load();
    match kind {
        Kind::Input => state.input.clear(),
        Kind::Output => state.output.clear(),
    }
    save(&state)?;
    Ok(state)
}

fn save(state: &RecentPaths) -> Result<()> {
    let path = recent_file()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(state)?;
    std::fs::write(&path, json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::MutexGuard;

    /// Shared with `preset` tests — see `crate::test_util`.
    fn env_lock() -> MutexGuard<'static, ()> {
        crate::test_util::config_dir_lock()
    }

    struct ScopedConfigDir {
        _tmp: tempfile::TempDir,
        _guard: MutexGuard<'static, ()>,
    }

    impl ScopedConfigDir {
        fn new() -> Self {
            let guard = env_lock();
            let tmp = tempfile::Builder::new()
                .prefix("pixiekit-recent-test-")
                .tempdir()
                .unwrap();
            std::env::set_var("PIXIEKIT_CONFIG_DIR", tmp.path());
            Self { _tmp: tmp, _guard: guard }
        }
    }

    impl Drop for ScopedConfigDir {
        fn drop(&mut self) {
            std::env::remove_var("PIXIEKIT_CONFIG_DIR");
        }
    }

    #[test]
    fn add_dedupes_and_caps() {
        let _g = ScopedConfigDir::new();
        for i in 0..15 {
            add(Kind::Input, format!("/path/{i}")).unwrap();
        }
        let state = load();
        assert_eq!(state.input.len(), MAX_ENTRIES);
        assert_eq!(state.input[0], "/path/14");

        add(Kind::Input, "/path/10").unwrap();
        let state = load();
        assert_eq!(state.input[0], "/path/10");
        assert_eq!(state.input.len(), MAX_ENTRIES);
    }

    #[test]
    fn clear_works() {
        let _g = ScopedConfigDir::new();
        add(Kind::Input, "/a").unwrap();
        add(Kind::Output, "/b").unwrap();
        clear(Kind::Input).unwrap();
        let state = load();
        assert!(state.input.is_empty());
        assert_eq!(state.output, vec!["/b"]);
    }
}
