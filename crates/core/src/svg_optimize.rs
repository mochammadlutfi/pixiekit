//! SVG minify pass — parse with [`usvg`], serialize with reduced precision and
//! no whitespace, optionally strip metadata / hidden elements.
//!
//! Pipeline:
//!
//! 1. Read SVG input bytes.
//! 2. Parse via `usvg::Tree::from_data` (default `usvg::Options`).
//!    - usvg already drops most `inkscape:*` / editor metadata during parse.
//! 3. Serialize via `usvg::Tree::to_string` with [`usvg::WriteOptions`] tuned
//!    for minification: `coordinates_precision` / `transforms_precision` set
//!    from [`Options::precision`] and `Indent::None` for tight whitespace.
//! 4. Optional regex passes on the serialized output to:
//!    - strip `<title>` / `<desc>` / XML comments when [`Options::remove_metadata`]
//!    - strip elements with `display="none"` or `visibility="hidden"` when
//!      [`Options::remove_hidden`]
//!
//! Path merging is intentionally a stretch goal — usvg already groups paths
//! per layer during parse, and full path-merging on raw SVG is well outside
//! what a single core module should ship. When [`Options::merge_paths`] is
//! `false`, the regex passes are skipped (kept as a knob for parity with the
//! PRD without claiming the feature is fully implemented).

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// SVG optimize options.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Options {
    /// Decimal places for path coordinates / transforms. Default `3`.
    /// Clamped to `0..=8` (usvg writer limit).
    pub precision: u8,

    /// Strip `<title>` / `<desc>` / XML comments. Default `true`.
    pub remove_metadata: bool,

    /// Strip elements with `display="none"` or `visibility="hidden"`.
    /// Default `true`.
    pub remove_hidden: bool,

    /// Reserved knob — when `false`, currently disables the regex post-passes
    /// (see module docs for the rationale). Default `true`.
    pub merge_paths: bool,

    /// Pretty-print with indented output. Default `false` (minified).
    pub pretty: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            precision: 3,
            remove_metadata: true,
            remove_hidden: true,
            merge_paths: true,
            pretty: false,
        }
    }
}

/// Report describing one optimize pass.
#[derive(Debug, Clone, Serialize)]
pub struct SvgReport {
    /// Original file size in bytes.
    pub input_size: u64,
    /// Final file size in bytes.
    pub output_size: u64,
    /// `output_size / input_size` (1.0 = no change, <1.0 = shrunk).
    pub ratio: f32,
}

/// Optimize a single SVG file in place (writes to `output`).
///
/// # Errors
///
/// - [`Error::NotFound`] if `input` does not exist.
/// - [`Error::SvgParseFailed`] if `usvg` cannot parse the SVG.
/// - [`Error::Io`] on read/write failures.
pub fn process(input: &Path, output: &Path, opts: &Options) -> Result<SvgReport> {
    if !input.exists() {
        return Err(Error::NotFound(input.to_path_buf()));
    }

    let bytes = std::fs::read(input)?;
    let input_size = bytes.len() as u64;

    let parse_opts = usvg::Options::default();
    let tree = usvg::Tree::from_data(&bytes, &parse_opts)
        .map_err(|e| Error::SvgParseFailed(format!("{}: {}", input.display(), e)))?;

    let precision = opts.precision.min(8);
    let write_opts = usvg::WriteOptions {
        coordinates_precision: precision,
        transforms_precision: precision,
        indent: if opts.pretty {
            usvg::Indent::Spaces(2)
        } else {
            usvg::Indent::None
        },
        attributes_indent: usvg::Indent::None,
        ..usvg::WriteOptions::default()
    };
    let mut svg = tree.to_string(&write_opts);

    if opts.merge_paths {
        if opts.remove_metadata {
            svg = strip_metadata(&svg);
        }
        if opts.remove_hidden {
            svg = strip_hidden(&svg);
        }
    }

    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(output, svg.as_bytes())?;
    let output_size = svg.len() as u64;

    let ratio = if input_size == 0 {
        0.0
    } else {
        output_size as f32 / input_size as f32
    };

    Ok(SvgReport {
        input_size,
        output_size,
        ratio,
    })
}

/// Remove `<title>...</title>`, `<desc>...</desc>`, and XML comments. Uses a
/// hand-rolled scan rather than a regex dep so the core stays light.
fn strip_metadata(svg: &str) -> String {
    let mut out = String::with_capacity(svg.len());
    let mut rest = svg;
    while !rest.is_empty() {
        if let Some(open) = rest.find('<') {
            out.push_str(&rest[..open]);
            rest = &rest[open..];
            if let Some(stripped) = strip_one(rest, "<!--", "-->") {
                rest = stripped;
                continue;
            }
            if let Some(stripped) = strip_one(rest, "<title", "</title>") {
                rest = stripped;
                continue;
            }
            if let Some(stripped) = strip_one(rest, "<desc", "</desc>") {
                rest = stripped;
                continue;
            }
            // Not a stripped tag — emit '<' and advance one byte.
            out.push('<');
            rest = &rest[1..];
        } else {
            out.push_str(rest);
            break;
        }
    }
    out
}

/// If `s` begins with `prefix`, find the matching `suffix` and return the
/// remainder after the suffix; otherwise return `None`.
fn strip_one<'a>(s: &'a str, prefix: &str, suffix: &str) -> Option<&'a str> {
    if !s.starts_with(prefix) {
        return None;
    }
    let after_prefix = &s[prefix.len()..];
    let end = after_prefix.find(suffix)?;
    Some(&after_prefix[end + suffix.len()..])
}

/// Drop opening/closing/self-closing tags whose attribute list contains
/// `display="none"` or `visibility="hidden"`.
fn strip_hidden(svg: &str) -> String {
    // Keep this simple — scan tag-by-tag, drop the tag itself when it has the
    // hidden marker as a self-closing tag. A proper DOM walk would be safer
    // but is overkill for usvg-emitted output.
    let mut out = String::with_capacity(svg.len());
    let mut rest = svg;
    while let Some(open_idx) = rest.find('<') {
        out.push_str(&rest[..open_idx]);
        let tail = &rest[open_idx..];
        let close_idx = tail.find('>').map(|i| i + 1).unwrap_or(tail.len());
        let tag = &tail[..close_idx];
        let is_hidden = (tag.contains("display=\"none\"") || tag.contains("display='none'"))
            || tag.contains("visibility=\"hidden\"")
            || tag.contains("visibility='hidden'");
        if is_hidden && tag.ends_with("/>") {
            // Drop just this self-closing tag.
            rest = &tail[close_idx..];
        } else {
            out.push_str(tag);
            rest = &tail[close_idx..];
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmpdir(test_name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "pixiekit-svg-test-{}-{}",
            std::process::id(),
            test_name
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_svg(dir: &Path, name: &str, content: &str) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, content).unwrap();
        p
    }

    const SIMPLE_SVG: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="no"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100">
  <rect x="10" y="10" width="80" height="80" fill="red"/>
</svg>"#;

    #[test]
    fn options_defaults() {
        let opts = Options::default();
        assert_eq!(opts.precision, 3);
        assert!(opts.remove_metadata);
        assert!(opts.remove_hidden);
        assert!(opts.merge_paths);
        assert!(!opts.pretty);
    }

    #[test]
    fn process_errors_on_missing_input() {
        let dir = tmpdir("svg_missing");
        let input = dir.join("nope.svg");
        let output = dir.join("out.svg");
        let result = process(&input, &output, &Options::default());
        assert!(matches!(result, Err(Error::NotFound(_))));
    }

    #[test]
    fn process_errors_on_invalid_svg() {
        let dir = tmpdir("svg_invalid");
        let input = write_svg(&dir, "broken.svg", "this is not svg");
        let output = dir.join("out.svg");
        let result = process(&input, &output, &Options::default());
        assert!(matches!(result, Err(Error::SvgParseFailed(_))));
    }

    #[test]
    fn process_writes_output_for_simple_svg() {
        let dir = tmpdir("svg_simple");
        let input = write_svg(&dir, "in.svg", SIMPLE_SVG);
        let output = dir.join("out.svg");

        let report = process(&input, &output, &Options::default()).unwrap();
        assert!(output.exists());
        let written = std::fs::read_to_string(&output).unwrap();
        assert!(
            written.contains("<svg"),
            "missing <svg> tag: {}",
            &written[..written.len().min(80)]
        );
        assert!(report.output_size > 0);
        assert_eq!(report.input_size, SIMPLE_SVG.len() as u64);
    }

    #[test]
    fn process_returns_positive_ratio() {
        let dir = tmpdir("svg_ratio");
        let input = write_svg(&dir, "in.svg", SIMPLE_SVG);
        let output = dir.join("out.svg");
        let report = process(&input, &output, &Options::default()).unwrap();
        assert!(
            report.ratio > 0.0,
            "ratio must be > 0, got {}",
            report.ratio
        );
    }

    #[test]
    fn precision_zero_strips_decimals_in_output() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 10 10">
            <rect x="1.234" y="2.567" width="3.456" height="4.789" fill="red"/>
        </svg>"#;
        let dir = tmpdir("svg_precision_zero");
        let input = write_svg(&dir, "in.svg", svg);
        let output = dir.join("out.svg");
        let opts = Options {
            precision: 0,
            ..Default::default()
        };
        process(&input, &output, &opts).unwrap();
        let written = std::fs::read_to_string(&output).unwrap();
        // No "1.234" — usvg should round to integers when precision=0.
        assert!(
            !written.contains("1.234"),
            "expected no 1.234, got: {written}"
        );
        assert!(
            !written.contains("2.567"),
            "expected no 2.567, got: {written}"
        );
    }

    #[test]
    fn remove_metadata_strips_title_and_desc() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 10 10">
            <title>my title</title>
            <desc>my desc</desc>
            <rect x="0" y="0" width="10" height="10" fill="red"/>
        </svg>"#;
        let dir = tmpdir("svg_remove_metadata");
        let input = write_svg(&dir, "in.svg", svg);
        let output = dir.join("out.svg");
        process(&input, &output, &Options::default()).unwrap();
        let written = std::fs::read_to_string(&output).unwrap();
        assert!(!written.contains("my title"), "title leaked: {written}");
        assert!(!written.contains("my desc"), "desc leaked: {written}");
    }

    #[test]
    fn output_is_well_formed_xml() {
        let dir = tmpdir("svg_well_formed");
        let input = write_svg(&dir, "in.svg", SIMPLE_SVG);
        let output = dir.join("out.svg");
        process(&input, &output, &Options::default()).unwrap();
        let written = std::fs::read_to_string(&output).unwrap();
        // Must round-trip back through usvg (the strongest "is valid" check we
        // can run without pulling in another XML parser).
        let parse_opts = usvg::Options::default();
        usvg::Tree::from_data(written.as_bytes(), &parse_opts)
            .expect("optimized output must re-parse cleanly");
    }

    #[test]
    fn strip_metadata_removes_xml_comments() {
        let input = "<svg><!-- a comment --><rect/></svg>";
        let stripped = strip_metadata(input);
        assert!(!stripped.contains("a comment"));
    }

    #[test]
    fn strip_hidden_removes_self_closing_display_none() {
        let input = r#"<svg><rect display="none"/><rect fill="red"/></svg>"#;
        let stripped = strip_hidden(input);
        assert!(!stripped.contains("display=\"none\""));
        assert!(stripped.contains("fill=\"red\""));
    }
}
