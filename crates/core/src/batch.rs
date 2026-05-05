//! Folder traversal helpers for batch processing.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// List image files in `input`. If `input` is a file, returns it as a single-element
/// vector (no extension check). If `input` is a directory, walks it (depth 1 unless
/// `recursive`) and filters by `exts` (case-insensitive, no leading dot).
pub fn list_images(input: &Path, recursive: bool, exts: &[&str]) -> Result<Vec<PathBuf>> {
    if input.is_file() {
        return Ok(vec![input.to_path_buf()]);
    }
    if !input.exists() {
        return Err(Error::NotFound(input.to_path_buf()));
    }
    if !input.is_dir() {
        return Err(Error::InvalidInput(format!(
            "Not a file or directory: {}",
            input.display()
        )));
    }

    let max_depth = if recursive { usize::MAX } else { 1 };
    let mut files = Vec::new();
    for entry in walkdir::WalkDir::new(input).max_depth(max_depth) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        if exts.iter().any(|allowed| allowed.eq_ignore_ascii_case(ext)) {
            files.push(path.to_path_buf());
        }
    }
    files.sort();
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    /// Per-test temp dir — uses test-specific suffix to avoid race when tests
    /// run in parallel (default `cargo test` behavior).
    fn tmpdir(test_name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "pixiekit-test-{}-{}",
            std::process::id(),
            test_name
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn touch(dir: &Path, name: &str) {
        let mut f = fs::File::create(dir.join(name)).unwrap();
        f.write_all(b"x").unwrap();
    }

    #[test]
    fn lists_pngs_only() {
        let dir = tmpdir("lists_pngs_only");
        touch(&dir, "a.png");
        touch(&dir, "b.PNG");
        touch(&dir, "c.jpg");
        touch(&dir, "d.txt");

        let files = list_images(&dir, false, &["png"]).unwrap();
        assert_eq!(files.len(), 2);
        assert!(files.iter().all(|p| {
            let ext = p.extension().unwrap().to_string_lossy().to_lowercase();
            ext == "png"
        }));
    }

    #[test]
    fn lists_multiple_extensions() {
        let dir = tmpdir("lists_multiple_extensions");
        touch(&dir, "a.png");
        touch(&dir, "b.jpg");
        touch(&dir, "c.webp");
        touch(&dir, "d.txt");

        let files = list_images(&dir, false, &["png", "jpg", "webp"]).unwrap();
        assert_eq!(files.len(), 3);
    }

    #[test]
    fn single_file_input_returns_self() {
        let dir = tmpdir("single_file_input_returns_self");
        touch(&dir, "a.png");
        let path = dir.join("a.png");
        let files = list_images(&path, false, &["png"]).unwrap();
        assert_eq!(files, vec![path]);
    }

    #[test]
    fn missing_path_errors() {
        let path = std::env::temp_dir().join("pixiekit-does-not-exist-xyz-9876");
        let result = list_images(&path, false, &["png"]);
        assert!(matches!(result, Err(Error::NotFound(_))));
    }
}
