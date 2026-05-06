//! Audio processor — LUFS normalize + silence trim + format convert via ffmpeg.
//!
//! Wraps `ffmpeg` as a subprocess (mirrors the [`crate::video_to_sprite`]
//! pattern). Builds an `-af` filter chain from the provided options, picks the
//! right encoder per [`TargetFormat`], and parses ffmpeg stderr to surface
//! basic input/output durations.
//!
//! No Rust audio crate dependency — keeps the dep tree minimal and reuses the
//! existing system `ffmpeg` requirement.

use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TargetFormat {
    Ogg,
    Opus,
    Mp3,
    Wav,
}

impl TargetFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            TargetFormat::Ogg => "ogg",
            TargetFormat::Opus => "opus",
            TargetFormat::Mp3 => "mp3",
            TargetFormat::Wav => "wav",
        }
    }

    /// Suggested HTTP `Content-Type` for the encoded payload.
    pub fn content_type(&self) -> &'static str {
        match self {
            TargetFormat::Ogg => "audio/ogg",
            TargetFormat::Opus => "audio/opus",
            TargetFormat::Mp3 => "audio/mpeg",
            TargetFormat::Wav => "audio/wav",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Channels {
    Mono,
    Stereo,
    Keep,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Options {
    /// Output container/codec.
    pub target_format: TargetFormat,

    /// Integrated loudness target in LUFS (used by ffmpeg `loudnorm`).
    /// Typical values: game SFX -16.0, dialogue -19.0, music -14.0.
    pub target_lufs: f32,

    /// Apply LUFS normalization (`loudnorm`).
    pub normalize: bool,

    /// Trim leading/trailing silence (`silenceremove`).
    pub trim_silence: bool,

    /// Silence detection threshold, in dB. Lower = more aggressive trim.
    pub silence_threshold_db: f32,

    /// Output sample rate in Hz (uses `aresample`).
    pub sample_rate: u32,

    /// Output channel layout (`mono`, `stereo`, or `keep` to passthrough).
    pub channels: Channels,

    /// Encoder bitrate in kbps. Ignored for WAV.
    pub bitrate_kbps: u16,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            target_format: TargetFormat::Ogg,
            target_lufs: -16.0,
            normalize: true,
            trim_silence: true,
            silence_threshold_db: -50.0,
            sample_rate: 44_100,
            channels: Channels::Keep,
            bitrate_kbps: 128,
        }
    }
}

/// Result of processing a single audio file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioReport {
    pub duration_ms_in: u32,
    pub duration_ms_out: u32,
    /// Integrated LUFS measured by `loudnorm` (only present when `normalize`).
    pub integrated_lufs: Option<f32>,
}

/// Process a single audio file. `output` should include the desired filename;
/// the parent directory must already exist.
pub fn process(input: &Path, output: &Path, opts: &Options) -> Result<AudioReport> {
    if !input.exists() {
        return Err(Error::NotFound(input.to_path_buf()));
    }
    check_ffmpeg()?;

    let duration_ms_in = probe_duration_ms(input).unwrap_or(0);

    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-y", "-i"]).arg(input);

    let filter = build_filter_chain(opts);
    if !filter.is_empty() {
        cmd.args(["-af", &filter]);
    }

    match opts.channels {
        Channels::Mono => {
            cmd.args(["-ac", "1"]);
        }
        Channels::Stereo => {
            cmd.args(["-ac", "2"]);
        }
        Channels::Keep => {}
    }

    apply_encoder(&mut cmd, opts);
    cmd.arg(output);

    let result = cmd.output()?;
    if !result.status.success() {
        return Err(Error::FfmpegFailed {
            code: result.status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&result.stderr).into_owned(),
        });
    }

    let stderr_text = String::from_utf8_lossy(&result.stderr);
    let integrated_lufs = if opts.normalize {
        parse_integrated_lufs(&stderr_text)
    } else {
        None
    };

    let duration_ms_out = probe_duration_ms(output).unwrap_or(duration_ms_in);

    Ok(AudioReport {
        duration_ms_in,
        duration_ms_out,
        integrated_lufs,
    })
}

/// Verify `ffmpeg` is callable. Errors with [`Error::FfmpegMissing`] otherwise.
pub fn check_ffmpeg() -> Result<()> {
    let result = Command::new("ffmpeg").arg("-version").output();
    match result {
        Ok(output) if output.status.success() => Ok(()),
        _ => Err(Error::FfmpegMissing),
    }
}

/// Build the `-af` filter chain string from the user's options.
///
/// Order: silenceremove → loudnorm → aresample. ffmpeg evaluates filters
/// left-to-right, and trimming silence before normalising avoids the silent
/// tail biasing the loudness measurement.
fn build_filter_chain(opts: &Options) -> String {
    let mut parts: Vec<String> = Vec::new();

    if opts.trim_silence {
        // stop_periods=-1 means "trim from both ends and any silence run".
        parts.push(format!(
            "silenceremove=stop_periods=-1:stop_duration=0.05:stop_threshold={}dB",
            opts.silence_threshold_db
        ));
    }

    if opts.normalize {
        parts.push(format!("loudnorm=I={}:TP=-1.5:LRA=11", opts.target_lufs));
    }

    parts.push(format!("aresample={}", opts.sample_rate));

    parts.join(",")
}

fn apply_encoder(cmd: &mut Command, opts: &Options) {
    let bitrate = format!("{}k", opts.bitrate_kbps);
    match opts.target_format {
        TargetFormat::Ogg => {
            cmd.args(["-c:a", "libvorbis", "-b:a"]).arg(&bitrate);
        }
        TargetFormat::Opus => {
            cmd.args(["-c:a", "libopus", "-b:a"]).arg(&bitrate);
        }
        TargetFormat::Mp3 => {
            cmd.args(["-c:a", "libmp3lame", "-b:a"]).arg(&bitrate);
        }
        TargetFormat::Wav => {
            cmd.args(["-c:a", "pcm_s16le"]);
        }
    }
}

/// Probe duration via `ffmpeg -i` (we don't ship `ffprobe`). Best-effort only —
/// callers should fall back to 0 / input duration when this returns `None`.
fn probe_duration_ms(path: &Path) -> Option<u32> {
    let output = Command::new("ffmpeg").arg("-i").arg(path).output().ok()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    parse_duration_ms(&stderr)
}

/// Parse `Duration: HH:MM:SS.SS` from ffmpeg stderr.
fn parse_duration_ms(stderr: &str) -> Option<u32> {
    let idx = stderr.find("Duration:")?;
    let rest = &stderr[idx + "Duration:".len()..];
    let token = rest.trim_start().split([',', ' ']).next()?.trim();
    let mut iter = token.split(':');
    let hh: u32 = iter.next()?.parse().ok()?;
    let mm: u32 = iter.next()?.parse().ok()?;
    let ss_str = iter.next()?;
    let ss: f32 = ss_str.parse().ok()?;
    Some(hh * 3_600_000 + mm * 60_000 + (ss * 1000.0) as u32)
}

/// Parse `Input Integrated:    -16.4 LUFS` from `loudnorm` summary.
fn parse_integrated_lufs(stderr: &str) -> Option<f32> {
    let needle = "Input Integrated:";
    let idx = stderr.find(needle)?;
    let rest = &stderr[idx + needle.len()..];
    let line_end = rest.find('\n').unwrap_or(rest.len());
    let line = &rest[..line_end];
    let token = line
        .split_whitespace()
        .find(|t| t.parse::<f32>().is_ok() || t.starts_with('-'))?;
    token.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_format_extensions() {
        assert_eq!(TargetFormat::Ogg.extension(), "ogg");
        assert_eq!(TargetFormat::Opus.extension(), "opus");
        assert_eq!(TargetFormat::Mp3.extension(), "mp3");
        assert_eq!(TargetFormat::Wav.extension(), "wav");
    }

    #[test]
    fn target_format_content_types() {
        assert_eq!(TargetFormat::Ogg.content_type(), "audio/ogg");
        assert_eq!(TargetFormat::Opus.content_type(), "audio/opus");
        assert_eq!(TargetFormat::Mp3.content_type(), "audio/mpeg");
        assert_eq!(TargetFormat::Wav.content_type(), "audio/wav");
    }

    #[test]
    fn options_default_values() {
        let opts = Options::default();
        assert_eq!(opts.target_format, TargetFormat::Ogg);
        assert!((opts.target_lufs - -16.0).abs() < f32::EPSILON);
        assert!(opts.normalize);
        assert!(opts.trim_silence);
        assert!((opts.silence_threshold_db - -50.0).abs() < f32::EPSILON);
        assert_eq!(opts.sample_rate, 44_100);
        assert_eq!(opts.channels, Channels::Keep);
        assert_eq!(opts.bitrate_kbps, 128);
    }

    #[test]
    fn filter_chain_default_has_all_three_stages() {
        let chain = build_filter_chain(&Options::default());
        assert!(
            chain.contains("silenceremove=stop_periods=-1:stop_duration=0.05:stop_threshold=-50dB"),
            "missing silenceremove: {chain}"
        );
        assert!(
            chain.contains("loudnorm=I=-16:TP=-1.5:LRA=11"),
            "missing loudnorm: {chain}"
        );
        assert!(
            chain.contains("aresample=44100"),
            "missing aresample: {chain}"
        );
        // Ordering: silenceremove, loudnorm, aresample
        let s = chain.find("silenceremove").unwrap();
        let l = chain.find("loudnorm").unwrap();
        let a = chain.find("aresample").unwrap();
        assert!(s < l && l < a, "wrong order: {chain}");
    }

    #[test]
    fn filter_chain_skips_silenceremove_when_disabled() {
        let opts = Options {
            trim_silence: false,
            ..Options::default()
        };
        let chain = build_filter_chain(&opts);
        assert!(!chain.contains("silenceremove"), "{chain}");
        assert!(chain.contains("loudnorm"));
        assert!(chain.contains("aresample"));
    }

    #[test]
    fn filter_chain_skips_loudnorm_when_disabled() {
        let opts = Options {
            normalize: false,
            ..Options::default()
        };
        let chain = build_filter_chain(&opts);
        assert!(!chain.contains("loudnorm"), "{chain}");
        assert!(chain.contains("silenceremove"));
        assert!(chain.contains("aresample"));
    }

    #[test]
    fn filter_chain_only_aresample_when_all_disabled() {
        let opts = Options {
            normalize: false,
            trim_silence: false,
            sample_rate: 48_000,
            ..Options::default()
        };
        let chain = build_filter_chain(&opts);
        assert_eq!(chain, "aresample=48000");
    }

    #[test]
    fn filter_chain_uses_custom_lufs_and_threshold() {
        let opts = Options {
            target_lufs: -19.5,
            silence_threshold_db: -40.0,
            ..Options::default()
        };
        let chain = build_filter_chain(&opts);
        assert!(chain.contains("loudnorm=I=-19.5"), "{chain}");
        assert!(chain.contains("stop_threshold=-40dB"), "{chain}");
    }

    #[test]
    fn parse_duration_ms_basic() {
        let stderr = "  Duration: 00:00:03.45, start: 0.000000, bitrate: 128 kb/s";
        assert_eq!(parse_duration_ms(stderr), Some(3450));
    }

    #[test]
    fn parse_duration_ms_with_minutes() {
        let stderr = "  Duration: 01:02:03.50, ...";
        // 1h + 2m + 3.5s = 3600000 + 120000 + 3500 = 3723500
        assert_eq!(parse_duration_ms(stderr), Some(3_723_500));
    }

    #[test]
    fn parse_duration_ms_missing_returns_none() {
        assert_eq!(parse_duration_ms("no duration here"), None);
    }

    #[test]
    fn parse_integrated_lufs_basic() {
        let stderr = "[Parsed_loudnorm_0 @ 0x] \nInput Integrated:    -16.4 LUFS\nInput True Peak:";
        let v = parse_integrated_lufs(stderr).unwrap();
        assert!((v - -16.4).abs() < 0.01, "got {v}");
    }

    #[test]
    fn parse_integrated_lufs_missing_returns_none() {
        assert_eq!(parse_integrated_lufs("no lufs here"), None);
    }

    #[test]
    fn check_ffmpeg_works_when_installed() {
        // Skip silently when ffmpeg isn't on PATH (matches video_to_sprite test).
        if Command::new("ffmpeg").arg("-version").output().is_ok() {
            assert!(check_ffmpeg().is_ok());
        }
    }
}
