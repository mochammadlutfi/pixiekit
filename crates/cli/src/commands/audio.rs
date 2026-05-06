use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{anyhow, Context, Result};
use clap::Args as ClapArgs;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;

use pixiekit_core::{audio, batch, preset};

const ALLOWED_EXTS: &[&str] = &["wav", "mp3", "ogg", "m4a", "flac", "opus"];

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Input audio file or folder of audio files
    #[arg(short, long)]
    pub input: PathBuf,

    /// Output folder (created if missing)
    #[arg(short, long)]
    pub output: PathBuf,

    /// Output format (ogg | opus | mp3 | wav)
    #[arg(long, default_value = "ogg")]
    pub target_format: String,

    /// Target integrated loudness in LUFS (used by loudnorm)
    #[arg(long, default_value_t = -16.0)]
    pub target_lufs: f32,

    /// Enable LUFS normalization (default: enabled, use --no-normalize to disable)
    #[arg(long, default_value_t = true, conflicts_with = "no_normalize")]
    pub normalize: bool,

    /// Disable LUFS normalization
    #[arg(long, default_value_t = false)]
    pub no_normalize: bool,

    /// Trim leading/trailing silence (default: enabled, use --no-trim-silence to disable)
    #[arg(long, default_value_t = true, conflicts_with = "no_trim_silence")]
    pub trim_silence: bool,

    /// Disable silence trimming
    #[arg(long, default_value_t = false)]
    pub no_trim_silence: bool,

    /// Silence detection threshold in dB
    #[arg(long, default_value_t = -50.0)]
    pub silence_threshold_db: f32,

    /// Output sample rate in Hz
    #[arg(long, default_value_t = 44_100)]
    pub sample_rate: u32,

    /// Channel layout (mono | stereo | keep)
    #[arg(long, default_value = "keep")]
    pub channels: String,

    /// Encoder bitrate in kbps (ignored for WAV)
    #[arg(long, default_value_t = 128)]
    pub bitrate_kbps: u16,

    /// Recursive folder scan
    #[arg(short, long)]
    pub recursive: bool,

    /// Overwrite existing output files
    #[arg(long)]
    pub overwrite: bool,

    /// Print plan, don't write
    #[arg(long)]
    pub dry_run: bool,

    /// JSON output (for AI / scripting)
    #[arg(long)]
    pub json: bool,

    /// Load tool options from a preset JSON file (overrides individual flags)
    #[arg(long, value_name = "PATH")]
    pub config: Option<PathBuf>,
}

pub fn run(args: Args) -> Result<()> {
    crate::commands::preflight_input(&args.input)?;

    let opts = match &args.config {
        Some(path) => load_options_from_config(path)?,
        None => build_options(&args)?,
    };

    let inputs = batch::list_images(&args.input, args.recursive, ALLOWED_EXTS)
        .context("Listing input audio files")?;

    if inputs.is_empty() {
        if args.json {
            println!(
                "{}",
                serde_json::json!({
                    "tool": "audio",
                    "processed": 0,
                    "failed": 0,
                    "files": [],
                    "warning": "No audio files found"
                })
            );
        } else {
            eprintln!("No audio files found in {}", args.input.display());
        }
        return Ok(());
    }

    if args.dry_run {
        if args.json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "tool": "audio",
                    "dry_run": true,
                    "files": inputs.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>(),
                }))?
            );
        } else {
            println!("Dry run — would process {} file(s):", inputs.len());
            for p in &inputs {
                println!("  {}", p.display());
            }
        }
        return Ok(());
    }

    std::fs::create_dir_all(&args.output)
        .with_context(|| format!("Creating output dir {}", args.output.display()))?;

    audio::check_ffmpeg().context("ffmpeg check")?;

    let pb = if args.json {
        ProgressBar::hidden()
    } else {
        let bar = ProgressBar::new(inputs.len() as u64);
        bar.set_style(
            ProgressStyle::with_template("{bar:40.cyan/blue} {pos}/{len} {wide_msg}")
                .unwrap()
                .progress_chars("█▓░"),
        );
        bar
    };

    let start = Instant::now();
    let results: Vec<AudioResult> = inputs
        .par_iter()
        .map(|input_path| {
            let result = process_one(input_path, &args.output, &opts, args.overwrite);
            pb.inc(1);
            if let Some(name) = input_path.file_name() {
                pb.set_message(name.to_string_lossy().into_owned());
            }
            AudioResult {
                input: input_path.clone(),
                output: result.as_ref().ok().map(|(p, _)| p.clone()),
                report: result.as_ref().ok().map(|(_, r)| r.clone()),
                error: result.err().map(|e| format!("{:#}", e)),
            }
        })
        .collect();
    pb.finish_and_clear();

    let processed = results.iter().filter(|r| r.error.is_none()).count();
    let failed = results.len() - processed;
    let duration_ms = start.elapsed().as_millis();

    if args.json {
        let json = serde_json::json!({
            "tool": "audio",
            "processed": processed,
            "failed": failed,
            "duration_ms": duration_ms,
            "files": results.iter().map(|r| {
                serde_json::json!({
                    "input": r.input,
                    "output": r.output,
                    "duration_ms_in": r.report.as_ref().map(|x| x.duration_ms_in),
                    "duration_ms_out": r.report.as_ref().map(|x| x.duration_ms_out),
                    "integrated_lufs": r.report.as_ref().and_then(|x| x.integrated_lufs),
                    "status": if r.error.is_none() { "ok" } else { "failed" },
                    "error": r.error,
                })
            }).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        println!(
            "✓ Processed {}/{} audio file(s) in {}ms",
            processed,
            inputs.len(),
            duration_ms
        );
        for r in &results {
            match (&r.report, &r.output) {
                (Some(rep), Some(out)) => println!(
                    "  ✓ {} → {} ({}ms in / {}ms out)",
                    r.input.file_name().unwrap_or_default().to_string_lossy(),
                    out.file_name().unwrap_or_default().to_string_lossy(),
                    rep.duration_ms_in,
                    rep.duration_ms_out,
                ),
                _ => eprintln!(
                    "  ✗ {}: {}",
                    r.input.file_name().unwrap_or_default().to_string_lossy(),
                    r.error.as_deref().unwrap_or("unknown")
                ),
            }
        }
    }

    if failed > 0 {
        std::process::exit(1);
    }
    Ok(())
}

struct AudioResult {
    input: PathBuf,
    output: Option<PathBuf>,
    report: Option<audio::AudioReport>,
    error: Option<String>,
}

fn process_one(
    input_path: &Path,
    output_dir: &Path,
    opts: &audio::Options,
    overwrite: bool,
) -> Result<(PathBuf, audio::AudioReport)> {
    let stem = input_path
        .file_stem()
        .ok_or_else(|| anyhow!("Invalid filename: {}", input_path.display()))?;
    let out_path = output_dir.join(format!(
        "{}.{}",
        stem.to_string_lossy(),
        opts.target_format.extension()
    ));
    if !overwrite && out_path.exists() {
        return Err(anyhow!(
            "Output exists (use --overwrite): {}",
            out_path.display()
        ));
    }
    let report = audio::process(input_path, &out_path, opts)?;
    Ok((out_path, report))
}

fn build_options(args: &Args) -> Result<audio::Options> {
    let target_format = parse_target_format(&args.target_format)
        .with_context(|| format!("Invalid --target-format: {}", args.target_format))?;
    let channels = parse_channels(&args.channels)
        .with_context(|| format!("Invalid --channels: {}", args.channels))?;
    let normalize = !args.no_normalize && args.normalize;
    let trim_silence = !args.no_trim_silence && args.trim_silence;
    Ok(audio::Options {
        target_format,
        target_lufs: args.target_lufs,
        normalize,
        trim_silence,
        silence_threshold_db: args.silence_threshold_db,
        sample_rate: args.sample_rate,
        channels,
        bitrate_kbps: args.bitrate_kbps,
    })
}

fn load_options_from_config(path: &Path) -> Result<audio::Options> {
    let preset = preset::load_from_path(path)
        .with_context(|| format!("Loading preset {}", path.display()))?;
    preset::ensure_tool(&preset, preset::TOOL_AUDIO)
        .with_context(|| format!("Preset {} is not an audio preset", path.display()))?;
    serde_json::from_value(preset.options)
        .with_context(|| format!("Decoding audio options from {}", path.display()))
}

pub(crate) fn parse_target_format(s: &str) -> Result<audio::TargetFormat> {
    match s.to_ascii_lowercase().as_str() {
        "ogg" => Ok(audio::TargetFormat::Ogg),
        "opus" => Ok(audio::TargetFormat::Opus),
        "mp3" => Ok(audio::TargetFormat::Mp3),
        "wav" => Ok(audio::TargetFormat::Wav),
        other => Err(anyhow!(
            "Unsupported target format: {} (expected ogg|opus|mp3|wav)",
            other
        )),
    }
}

pub(crate) fn parse_channels(s: &str) -> Result<audio::Channels> {
    match s.to_ascii_lowercase().as_str() {
        "mono" => Ok(audio::Channels::Mono),
        "stereo" => Ok(audio::Channels::Stereo),
        "keep" => Ok(audio::Channels::Keep),
        other => Err(anyhow!(
            "Unsupported channels: {} (expected mono|stereo|keep)",
            other
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_target_format_lowercase() {
        assert_eq!(
            parse_target_format("ogg").unwrap(),
            audio::TargetFormat::Ogg
        );
        assert_eq!(
            parse_target_format("opus").unwrap(),
            audio::TargetFormat::Opus
        );
        assert_eq!(
            parse_target_format("mp3").unwrap(),
            audio::TargetFormat::Mp3
        );
        assert_eq!(
            parse_target_format("wav").unwrap(),
            audio::TargetFormat::Wav
        );
    }

    #[test]
    fn parse_target_format_uppercase() {
        assert_eq!(
            parse_target_format("WAV").unwrap(),
            audio::TargetFormat::Wav
        );
    }

    #[test]
    fn parse_target_format_unknown_errors() {
        assert!(parse_target_format("flac").is_err());
    }

    #[test]
    fn parse_channels_variants() {
        assert_eq!(parse_channels("mono").unwrap(), audio::Channels::Mono);
        assert_eq!(parse_channels("stereo").unwrap(), audio::Channels::Stereo);
        assert_eq!(parse_channels("keep").unwrap(), audio::Channels::Keep);
        assert_eq!(parse_channels("KEEP").unwrap(), audio::Channels::Keep);
        assert!(parse_channels("surround").is_err());
    }
}
