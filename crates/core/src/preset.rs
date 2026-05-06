//! Preset persistence — save / load / list reusable tool configurations.
//!
//! A preset wraps an opaque `options` JSON blob with the tool it belongs to.
//! Presets live under `~/.config/pixiekit/presets/<name>.json` (XDG-style on
//! every platform — PRD §9.1). Override the root via the
//! `PIXIEKIT_CONFIG_DIR` env var (used by tests and ephemeral setups).
//!
//! The `options` field is a `serde_json::Value` so each tool's frontend can
//! deserialize it into its own `Options` struct without this module needing to
//! know the per-tool schema.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Current preset file schema version. Bumped on breaking shape changes.
pub const PRESET_VERSION: u32 = 1;

/// Tool discriminator string. Use the same kebab-case form the CLI subcommand
/// uses so a preset's `tool` field round-trips unchanged.
pub const TOOL_BG_REMOVE: &str = "bg-remove";
pub const TOOL_VECTORIZE: &str = "vectorize";
pub const TOOL_VIDEO_TO_SPRITE: &str = "video-to-sprite";
pub const TOOL_ATLAS_PACK: &str = "atlas-pack";
pub const TOOL_OPTIMIZE: &str = "optimize";
pub const TOOL_SCALE: &str = "scale";
pub const TOOL_AUDIO: &str = "audio";
pub const TOOL_TRIM_PAD: &str = "trim-pad";
pub const TOOL_SVG_OPTIMIZE: &str = "svg-optimize";
pub const TOOL_NINE_SLICE: &str = "nine-slice";
pub const TOOL_ANIM_PREVIEW: &str = "anim-preview";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    pub tool: String,
    pub version: u32,
    pub options: serde_json::Value,
}

/// Resolve `~/.config/pixiekit/` (or the env override). Created lazily on save.
pub fn config_dir() -> Result<PathBuf> {
    if let Ok(custom) = std::env::var("PIXIEKIT_CONFIG_DIR") {
        if !custom.is_empty() {
            return Ok(PathBuf::from(custom));
        }
    }
    let base = directories::BaseDirs::new()
        .ok_or_else(|| Error::ConfigDir("no home directory available".into()))?;
    Ok(base.home_dir().join(".config").join("pixiekit"))
}

/// Resolve `~/.config/pixiekit/presets/`.
pub fn presets_dir() -> Result<PathBuf> {
    Ok(config_dir()?.join("presets"))
}

/// Compute the path for a preset by name (does not check existence).
pub fn path_for(name: &str) -> Result<PathBuf> {
    validate_name(name)?;
    Ok(presets_dir()?.join(format!("{name}.json")))
}

/// Save a preset. Overwrites if the file already exists.
pub fn save(name: &str, tool: &str, options: serde_json::Value) -> Result<PathBuf> {
    validate_name(name)?;
    let dir = presets_dir()?;
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{name}.json"));

    let preset = Preset {
        name: name.to_string(),
        tool: tool.to_string(),
        version: PRESET_VERSION,
        options,
    };
    let json = serde_json::to_string_pretty(&preset)?;
    std::fs::write(&path, json)?;
    Ok(path)
}

/// Load a preset by name.
pub fn load(name: &str) -> Result<Preset> {
    validate_name(name)?;
    let path = path_for(name)?;
    load_from_path(&path).map_err(|err| match err {
        Error::Io(io) if io.kind() == std::io::ErrorKind::NotFound => Error::PresetNotFound {
            name: name.to_string(),
            path,
        },
        other => other,
    })
}

/// Load a preset directly from a path (used by CLI `--config <PATH>`).
pub fn load_from_path(path: &Path) -> Result<Preset> {
    let bytes = std::fs::read(path)?;
    let preset: Preset = serde_json::from_slice(&bytes)?;
    Ok(preset)
}

/// List preset names (filenames without extension), sorted.
pub fn list() -> Result<Vec<String>> {
    let dir = presets_dir()?;
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut names: Vec<String> = std::fs::read_dir(&dir)?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                return None;
            }
            path.file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())
        })
        .collect();
    names.sort();
    Ok(names)
}

/// Delete a preset. Returns the removed file path, or
/// [`Error::PresetNotFound`] if it didn't exist.
pub fn delete(name: &str) -> Result<PathBuf> {
    let path = path_for(name)?;
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(path),
        Err(io) if io.kind() == std::io::ErrorKind::NotFound => Err(Error::PresetNotFound {
            name: name.to_string(),
            path,
        }),
        Err(io) => Err(Error::Io(io)),
    }
}

/// Verify `preset.tool` matches the expected tool string.
pub fn ensure_tool(preset: &Preset, expected: &str) -> Result<()> {
    if preset.tool == expected {
        Ok(())
    } else {
        Err(Error::PresetToolMismatch {
            expected: expected.to_string(),
            got: preset.tool.clone(),
        })
    }
}

fn validate_name(name: &str) -> Result<()> {
    if name.is_empty() || name.len() > 64 {
        return Err(Error::InvalidPresetName(name.to_string()));
    }
    let ok = name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !ok {
        return Err(Error::InvalidPresetName(name.to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::MutexGuard;

    /// Shared with `recent` tests via `crate::test_util` — both modules mutate
    /// `PIXIEKIT_CONFIG_DIR`, so they serialize on the same process-global mutex.
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
                .prefix("pixiekit-preset-test-")
                .tempdir()
                .unwrap();
            std::env::set_var("PIXIEKIT_CONFIG_DIR", tmp.path());
            Self {
                _tmp: tmp,
                _guard: guard,
            }
        }
    }

    impl Drop for ScopedConfigDir {
        fn drop(&mut self) {
            std::env::remove_var("PIXIEKIT_CONFIG_DIR");
        }
    }

    #[test]
    fn validate_rejects_empty() {
        assert!(validate_name("").is_err());
    }

    #[test]
    fn validate_rejects_path_traversal() {
        assert!(validate_name("../etc/passwd").is_err());
        assert!(validate_name("foo/bar").is_err());
        assert!(validate_name("foo.bar").is_err());
    }

    #[test]
    fn validate_accepts_safe_chars() {
        assert!(validate_name("domdom-bg-clean").is_ok());
        assert!(validate_name("preset_42").is_ok());
        assert!(validate_name("ABC123").is_ok());
    }

    #[test]
    fn save_then_load_roundtrip() {
        let _scope = ScopedConfigDir::new();
        let opts = serde_json::json!({"fuzz": 0.5, "erode": 2});
        let path = save("my-preset", TOOL_BG_REMOVE, opts.clone()).unwrap();
        assert!(path.exists());

        let loaded = load("my-preset").unwrap();
        assert_eq!(loaded.name, "my-preset");
        assert_eq!(loaded.tool, TOOL_BG_REMOVE);
        assert_eq!(loaded.version, PRESET_VERSION);
        assert_eq!(loaded.options, opts);
    }

    #[test]
    fn load_missing_returns_not_found() {
        let _scope = ScopedConfigDir::new();
        let err = load("nope").unwrap_err();
        assert!(matches!(err, Error::PresetNotFound { .. }));
    }

    #[test]
    fn list_empty_when_no_dir() {
        let _scope = ScopedConfigDir::new();
        assert!(list().unwrap().is_empty());
    }

    #[test]
    fn list_returns_sorted_names() {
        let _scope = ScopedConfigDir::new();
        save("bbb", TOOL_VECTORIZE, serde_json::json!({})).unwrap();
        save("aaa", TOOL_BG_REMOVE, serde_json::json!({})).unwrap();
        save("ccc", TOOL_VIDEO_TO_SPRITE, serde_json::json!({})).unwrap();
        let names = list().unwrap();
        assert_eq!(names, vec!["aaa", "bbb", "ccc"]);
    }

    #[test]
    fn delete_removes_file() {
        let _scope = ScopedConfigDir::new();
        save("temp", TOOL_BG_REMOVE, serde_json::json!({})).unwrap();
        let path = delete("temp").unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn delete_missing_returns_not_found() {
        let _scope = ScopedConfigDir::new();
        let err = delete("ghost").unwrap_err();
        assert!(matches!(err, Error::PresetNotFound { .. }));
    }

    #[test]
    fn ensure_tool_matches() {
        let preset = Preset {
            name: "x".into(),
            tool: TOOL_BG_REMOVE.into(),
            version: PRESET_VERSION,
            options: serde_json::json!({}),
        };
        assert!(ensure_tool(&preset, TOOL_BG_REMOVE).is_ok());
        assert!(matches!(
            ensure_tool(&preset, TOOL_VECTORIZE),
            Err(Error::PresetToolMismatch { .. })
        ));
    }

    #[test]
    fn save_overwrites_existing() {
        let _scope = ScopedConfigDir::new();
        save("p", TOOL_BG_REMOVE, serde_json::json!({"v": 1})).unwrap();
        save("p", TOOL_BG_REMOVE, serde_json::json!({"v": 2})).unwrap();
        let loaded = load("p").unwrap();
        assert_eq!(loaded.options, serde_json::json!({"v": 2}));
    }
}
