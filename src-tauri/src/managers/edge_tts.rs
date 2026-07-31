//! Experimental adapter for the community `edge-tts` command-line package.
//!
//! The package uses Microsoft Edge's online Read Aloud service and does not
//! require an API key. It is intentionally isolated behind a subprocess: the
//! protocol is unofficial and changes independently of AivoRelay.

use anyhow::{anyhow, Context, Result};
use rodio::Source;
use rubato::{FftFixedIn, Resampler};
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;

pub const EDGE_TTS_MODEL: &str = "microsoft-edge-read-aloud";
pub const EDGE_TTS_PROVIDER_LIMIT: usize = 4_096;
pub const DEFAULT_EDGE_TTS_VOICE: &str = "en-US-AriaNeural";

const EDGE_TTS_TIMEOUT: Duration = Duration::from_secs(120);
const EDGE_TTS_CATALOG_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_EDGE_MEDIA_BYTES: u64 = 64 * 1024 * 1024;
const MAX_EDGE_PROCESS_OUTPUT_BYTES: usize = 4 * 1024 * 1024;
const MAX_EDGE_PCM_SAMPLES: usize = 24_000 * 60 * 30;
static EDGE_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub fn voice_language(voice: &str) -> String {
    let mut parts = voice.trim().split('-');
    match (parts.next(), parts.next()) {
        (Some(language), Some(region)) if !language.is_empty() && !region.is_empty() => {
            format!("{language}-{region}")
        }
        _ => String::new(),
    }
}

#[derive(Debug)]
pub struct EdgeTtsError {
    pub safe_message: String,
    pub transient: bool,
}

impl std::fmt::Display for EdgeTtsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.safe_message)
    }
}

#[derive(Debug, Clone)]
pub struct EdgeVoice {
    pub id: String,
    pub language: String,
    pub gender: String,
    pub description: String,
}

struct EdgeTempFiles {
    input: PathBuf,
    output: PathBuf,
}

impl Drop for EdgeTempFiles {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.input);
        let _ = fs::remove_file(&self.output);
    }
}

pub async fn synthesize(
    cache_root: &Path,
    operation_id: u64,
    text: &str,
    voice: &str,
    speed: f32,
) -> std::result::Result<Vec<i16>, EdgeTtsError> {
    let voice = voice.trim();
    if voice.is_empty() || voice.chars().count() > 256 {
        return Err(permanent("Edge-TTS voice must contain 1 to 256 characters"));
    }

    let directory = cache_root.join("edge");
    fs::create_dir_all(&directory).map_err(|error| {
        permanent(format!(
            "Could not create the Edge-TTS cache directory: {error}"
        ))
    })?;
    let sequence = EDGE_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let stem = format!("{operation_id}-{sequence}");
    let files = EdgeTempFiles {
        input: directory.join(format!("{stem}.txt")),
        output: directory.join(format!("{stem}.mp3")),
    };
    fs::write(&files.input, text.as_bytes())
        .map_err(|error| permanent(format!("Could not prepare Edge-TTS input: {error}")))?;

    let rate_percent = ((speed.clamp(0.5, 2.0) - 1.0) * 100.0).round() as i32;
    let args = vec![
        OsString::from("--voice"),
        OsString::from(voice),
        OsString::from("--file"),
        files.input.as_os_str().to_owned(),
        OsString::from("--rate"),
        OsString::from(format!("{rate_percent:+}%")),
        OsString::from("--write-media"),
        files.output.as_os_str().to_owned(),
    ];
    let output = run_edge_tts(&args, EDGE_TTS_TIMEOUT).await?;
    if !output.status.success() {
        return Err(command_failure(&output));
    }

    let media_size = fs::metadata(&files.output)
        .map_err(|error| transient(format!("Edge-TTS did not create audio: {error}")))?
        .len();
    if media_size == 0 || media_size > MAX_EDGE_MEDIA_BYTES {
        return Err(permanent(format!(
            "Edge-TTS returned an invalid audio file ({media_size} bytes)"
        )));
    }

    tokio::task::spawn_blocking(move || decode_edge_mp3(&files.output))
        .await
        .map_err(|error| permanent(format!("Edge-TTS audio decoder stopped: {error}")))?
        .map_err(|error| permanent(format!("Could not decode Edge-TTS audio: {error}")))
}

pub async fn list_voices() -> std::result::Result<Vec<EdgeVoice>, EdgeTtsError> {
    let output = run_edge_tts(&[OsString::from("--list-voices")], EDGE_TTS_CATALOG_TIMEOUT).await?;
    if !output.status.success() {
        return Err(command_failure(&output));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let voices = parse_voice_table(&stdout);
    if voices.is_empty() {
        return Err(permanent(
            "edge-tts returned an empty or unsupported voice catalog",
        ));
    }
    Ok(voices)
}

async fn run_edge_tts(
    args: &[OsString],
    timeout: Duration,
) -> std::result::Result<Output, EdgeTtsError> {
    let candidates: [(&str, &[&str]); 3] = [
        ("edge-tts", &[]),
        ("py", &["-m", "edge_tts"]),
        ("python", &["-m", "edge_tts"]),
    ];
    let mut missing_runtime = false;
    for (program, prefix) in candidates {
        let mut command = Command::new(program);
        command
            .args(prefix)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                missing_runtime = true;
                continue;
            }
            Err(error) => {
                return Err(permanent(format!(
                    "Could not start the Edge-TTS helper: {error}"
                )))
            }
        };
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| permanent("Could not capture Edge-TTS output"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| permanent("Could not capture Edge-TTS errors"))?;
        let wait = async {
            let (status, stdout, stderr) = tokio::join!(
                child.wait(),
                read_bounded_output(stdout),
                read_bounded_output(stderr),
            );
            Ok::<Output, std::io::Error>(Output {
                status: status?,
                stdout: stdout?,
                stderr: stderr?,
            })
        };
        let output = match tokio::time::timeout(timeout, wait).await {
            Ok(Ok(output)) => output,
            Ok(Err(error)) => {
                return Err(transient(format!(
                    "The Edge-TTS helper stopped unexpectedly: {error}"
                )))
            }
            Err(_) => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                return Err(transient("The Edge-TTS helper timed out"));
            }
        };
        if !output.status.success() && reports_missing_runtime(&output.stderr) {
            missing_runtime = true;
            continue;
        }
        return Ok(output);
    }

    let detail = if missing_runtime {
        "Install the experimental helper with `uv tool install edge-tts` or `pipx install edge-tts`, then refresh the voice list."
    } else {
        "The Edge-TTS helper is unavailable."
    };
    Err(permanent(detail))
}

fn reports_missing_runtime(stderr: &[u8]) -> bool {
    let message = String::from_utf8_lossy(stderr).to_ascii_lowercase();
    message.contains("no module named edge_tts")
        || message.contains("no module named 'edge_tts'")
        || message.contains("python was not found")
        || message.contains("no suitable python runtime")
}

async fn read_bounded_output<R>(reader: R) -> std::io::Result<Vec<u8>>
where
    R: AsyncRead + Unpin,
{
    let mut reader = reader.take((MAX_EDGE_PROCESS_OUTPUT_BYTES + 1) as u64);
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).await?;
    if bytes.len() > MAX_EDGE_PROCESS_OUTPUT_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Edge-TTS helper output exceeded the safety limit",
        ));
    }
    Ok(bytes)
}

fn command_failure(output: &Output) -> EdgeTtsError {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let raw_message = stderr
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .map(str::trim)
        .unwrap_or("The Edge-TTS helper returned an error");
    let message = raw_message
        .chars()
        .filter(|character| !character.is_control())
        .take(1_024)
        .collect::<String>();
    let message = if message.is_empty() {
        "The Edge-TTS helper returned an error"
    } else {
        &message
    };
    let lower = message.to_ascii_lowercase();
    let transient_error = [
        "timeout",
        "timed out",
        "connection",
        "temporarily",
        "network",
        "websocket",
        "429",
        "503",
    ]
    .iter()
    .any(|marker| lower.contains(marker));
    EdgeTtsError {
        safe_message: format!("Edge-TTS helper: {message}"),
        transient: transient_error,
    }
}

fn parse_voice_table(table: &str) -> Vec<EdgeVoice> {
    table
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let id = fields.next()?;
            let gender = fields.next()?;
            if !matches!(gender, "Female" | "Male" | "Neutral") || !id.contains('-') {
                return None;
            }
            let language = voice_language(id);
            Some(EdgeVoice {
                id: id.to_string(),
                language,
                gender: gender.to_ascii_lowercase(),
                description: fields.collect::<Vec<_>>().join(" "),
            })
        })
        .collect()
}

fn decode_edge_mp3(path: &Path) -> Result<Vec<i16>> {
    let file = File::open(path).context("failed to open the generated MP3")?;
    let byte_len = file.metadata()?.len();
    let source = rodio::Decoder::builder()
        .with_data(BufReader::new(file))
        .with_byte_len(byte_len)
        .with_seekable(true)
        .with_hint("mp3")
        .with_mime_type("audio/mpeg")
        .build()
        .context("invalid MP3 stream")?;
    let sample_rate = source.sample_rate();
    let channels = usize::from(source.channels());
    if channels == 0 || sample_rate == 0 {
        return Err(anyhow!("invalid MP3 channel count or sample rate"));
    }
    let decoded: Vec<f32> = source.collect();
    if decoded.is_empty() || decoded.len() > MAX_EDGE_PCM_SAMPLES.saturating_mul(channels) {
        return Err(anyhow!("unsupported MP3 duration"));
    }
    let mono = if channels == 1 {
        decoded
    } else {
        decoded
            .chunks_exact(channels)
            .map(|frame| frame.iter().copied().sum::<f32>() / channels as f32)
            .collect()
    };
    let normalized = resample_exact(mono, sample_rate, 24_000)?;
    Ok(normalized
        .into_iter()
        .map(|sample| (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16)
        .collect())
}

fn resample_exact(samples: Vec<f32>, source_rate: u32, target_rate: u32) -> Result<Vec<f32>> {
    if source_rate == target_rate {
        return Ok(samples);
    }
    let expected = samples
        .len()
        .checked_mul(target_rate as usize)
        .map(|scaled| scaled.div_ceil(source_rate as usize))
        .filter(|length| *length != 0 && *length <= MAX_EDGE_PCM_SAMPLES)
        .ok_or_else(|| anyhow!("unsupported resampled duration"))?;
    let chunk = (source_rate / greatest_common_divisor(source_rate, target_rate)) as usize;
    let mut resampler = FftFixedIn::<f32>::new(
        source_rate as usize,
        target_rate as usize,
        chunk.max(1),
        1,
        1,
    )?;
    let delay = resampler.output_delay();
    let required = delay
        .checked_add(expected)
        .ok_or_else(|| anyhow!("resampled audio is too large"))?;
    let mut output = Vec::with_capacity(required);
    let mut offset = 0;
    while offset + chunk <= samples.len() {
        let result = resampler.process(&[&samples[offset..offset + chunk]], None)?;
        output.extend_from_slice(&result[0]);
        offset += chunk;
    }
    if offset < samples.len() {
        let result = resampler.process_partial(Some(&[&samples[offset..]]), None)?;
        output.extend_from_slice(&result[0]);
    }
    for _ in 0..2 {
        if output.len() >= required {
            break;
        }
        let result = resampler.process_partial::<&[f32]>(None, None)?;
        output.extend_from_slice(&result[0]);
    }
    if output.len() < required {
        return Err(anyhow!("resampler returned truncated audio"));
    }
    Ok(output[delay..required].to_vec())
}

fn greatest_common_divisor(mut left: u32, mut right: u32) -> u32 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left.max(1)
}

fn permanent(message: impl Into<String>) -> EdgeTtsError {
    EdgeTtsError {
        safe_message: message.into(),
        transient: false,
    }
}

fn transient(message: impl Into<String>) -> EdgeTtsError {
    EdgeTtsError {
        safe_message: message.into(),
        transient: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_edge_voice_table_without_trusting_column_widths() {
        let table = "Name Gender ContentCategories VoicePersonalities\n\
                     -------------------------------- -------- ----------------- ------------------\n\
                     en-US-AriaNeural Female General Friendly, Positive\n\
                     fi-FI-NooraNeural Female General Friendly\n";
        let voices = parse_voice_table(table);
        assert_eq!(voices.len(), 2);
        assert_eq!(voices[0].id, "en-US-AriaNeural");
        assert_eq!(voices[0].language, "en-US");
        assert_eq!(voices[0].gender, "female");
    }

    #[test]
    fn recognizes_missing_python_helper_diagnostics() {
        assert!(reports_missing_runtime(
            b"C:\\Python\\python.exe: No module named edge_tts"
        ));
        assert!(reports_missing_runtime(
            b"Python was not found; run without arguments to install"
        ));
        assert!(!reports_missing_runtime(b"Connection timed out"));
    }
}
