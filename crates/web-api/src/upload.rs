//! Multipart upload helpers — collect a single uploaded file into a temp dir.
//!
//! The handlers need:
//! - the raw file bytes written to a temp path the core tools can read
//! - an optional JSON `options` field
//!
//! [`collect_multipart`] walks the multipart fields once and returns both.

use axum::extract::Multipart;
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::io::AsyncWriteExt;

use crate::error::{AppError, AppResult};

/// Result of consuming a multipart request: the saved file's path inside a
/// retained [`TempDir`] (kept alive by the caller for the duration of the
/// request) plus the optional `options` field as a raw JSON string.
pub struct UploadedFile {
    pub temp_dir: TempDir,
    pub path: PathBuf,
    pub original_filename: Option<String>,
    pub options_json: Option<String>,
}

/// Walk a multipart body. Saves the first `file` field to a temp file. Reads
/// the `options` field as a UTF-8 string (caller parses to its option type).
///
/// Errors with [`AppError::Multipart`] when the body is malformed and
/// [`AppError::BadRequest`] when the `file` field is missing.
pub async fn collect_multipart(
    mut multipart: Multipart,
    fallback_filename: &str,
) -> AppResult<UploadedFile> {
    let temp_dir = tempfile::Builder::new()
        .prefix("pixiekit-upload-")
        .tempdir()?;

    let mut saved_path: Option<PathBuf> = None;
    let mut original_filename: Option<String> = None;
    let mut options_json: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::Multipart(e.to_string()))?
    {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "file" => {
                let filename = field.file_name().map(|s| s.to_string());
                let safe_name = filename
                    .as_deref()
                    .and_then(sanitize_filename)
                    .unwrap_or_else(|| fallback_filename.to_string());
                let path = temp_dir.path().join(&safe_name);

                let bytes = field
                    .bytes()
                    .await
                    .map_err(|e| AppError::Multipart(e.to_string()))?;

                let mut file = tokio::fs::File::create(&path).await?;
                file.write_all(&bytes).await?;
                file.flush().await?;

                saved_path = Some(path);
                original_filename = filename;
            }
            "options" => {
                let text = field
                    .text()
                    .await
                    .map_err(|e| AppError::Multipart(e.to_string()))?;
                options_json = Some(text);
            }
            _ => {
                // Drain unknown fields so the parser stays in sync.
                let _ = field.bytes().await;
            }
        }
    }

    let path =
        saved_path.ok_or_else(|| AppError::BadRequest("multipart: missing 'file' field".into()))?;

    Ok(UploadedFile {
        temp_dir,
        path,
        original_filename,
        options_json,
    })
}

/// Strip path separators / parent traversal from an uploaded filename.
fn sanitize_filename(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    // Take the basename, drop any directory components.
    let base = std::path::Path::new(trimmed).file_name()?.to_string_lossy();
    // Disallow leading dots and pure traversal markers.
    if base == "." || base == ".." {
        return None;
    }
    Some(base.into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_strips_directories() {
        assert_eq!(
            sanitize_filename("../etc/passwd").as_deref(),
            Some("passwd")
        );
        assert_eq!(
            sanitize_filename("/abs/path/x.png").as_deref(),
            Some("x.png")
        );
        assert_eq!(
            sanitize_filename("normal.png").as_deref(),
            Some("normal.png")
        );
    }

    #[test]
    fn sanitize_rejects_dot_only() {
        assert!(sanitize_filename(".").is_none());
        assert!(sanitize_filename("..").is_none());
        assert!(sanitize_filename("").is_none());
        assert!(sanitize_filename("   ").is_none());
    }
}
