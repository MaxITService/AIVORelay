//! Experimental native Rust client for Microsoft Edge's online Read Aloud service.
//!
//! The wire protocol is unofficial and can change independently of AivoRelay.
//! The implementation uses the MIT-licensed `kothok-edge-tts` crate and keeps
//! AivoRelay's own input, timeout, media-size, decoding, and PCM limits around it.

use anyhow::{anyhow, Context, Result};
use futures_util::StreamExt;
use kothok_edge_tts::{
    EdgeTts, Engine, TtsError as NativeEdgeTtsError, TtsEvent as NativeTtsEvent, VoiceInfo,
};
use rodio::Source;
use rubato::{FftFixedIn, Resampler};
use std::io::{BufReader, Cursor};
use std::time::Duration;

pub const EDGE_TTS_MODEL: &str = "microsoft-edge-read-aloud";
pub const EDGE_TTS_PROVIDER_LIMIT: usize = 4_096;
pub const DEFAULT_EDGE_TTS_VOICE: &str = "en-US-AriaNeural";

const EDGE_TTS_TIMEOUT: Duration = Duration::from_secs(120);
const EDGE_TTS_CATALOG_TIMEOUT: Duration = Duration::from_secs(30);
const EDGE_TTS_ESCAPED_TEXT_BYTES_LIMIT: usize = 4_096;
const EDGE_TTS_VOICE_CATALOG_BYTES_LIMIT: usize = 4 * 1024 * 1024;
const MAX_EDGE_MEDIA_BYTES: usize = 64 * 1024 * 1024;
const MAX_EDGE_PCM_SAMPLES: usize = 24_000 * 60 * 30;
const MAX_EDGE_VOICES: usize = 1_024;
const EDGE_TTS_TRUSTED_CLIENT_TOKEN: &str = "6A5AA1D4EAFF4E9FB37E23D68491D6F4";
const EDGE_TTS_GEC_VERSION: &str = "1-143.0.3650.75";

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

pub async fn synthesize(
    text: &str,
    voice: &str,
    speed: f32,
) -> std::result::Result<Vec<i16>, EdgeTtsError> {
    let voice = voice.trim();
    if voice.is_empty() || voice.chars().count() > 256 {
        return Err(permanent("Edge-TTS voice must contain 1 to 256 characters"));
    }
    if text.trim().is_empty() {
        return Err(permanent("Edge-TTS text must not be empty"));
    }

    // The native crate accepts one SSML utterance. AivoRelay normally chunks
    // first, but XML escaping can expand a 4,096-character Unicode-safe chunk
    // beyond Edge's practical message limit. Split the original text by its
    // escaped byte cost here and let the crate escape it exactly once.
    let segments = split_native_segments(text);
    if segments.is_empty() {
        return Err(permanent("Edge-TTS text must not be empty"));
    }

    let rate_percent = ((speed.clamp(0.5, 2.0) - 1.0) * 100.0).round() as i32;
    let rate = format!("{rate_percent:+}%");
    let language = match voice_language(voice) {
        language if !language.is_empty() => language,
        _ => "en-US".to_string(),
    };

    kothok_edge_tts::init_tls();
    let mut pcm = Vec::new();
    for segment in segments {
        let events = tokio::time::timeout(
            EDGE_TTS_TIMEOUT,
            EdgeTts.synthesize(&segment, voice, &rate, &language),
        )
        .await
        .map_err(|_| transient("Edge-TTS request timed out"))?
        .map_err(|error| native_failure("Edge-TTS request failed", error))?;

        let media = collect_bounded_media(events)?;
        let decoded = tokio::task::spawn_blocking(move || decode_edge_mp3(media))
            .await
            .map_err(|error| permanent(format!("Edge-TTS audio decoder stopped: {error}")))?
            .map_err(|error| permanent(format!("Could not decode Edge-TTS audio: {error}")))?;
        let total_samples = pcm
            .len()
            .checked_add(decoded.len())
            .filter(|samples| *samples <= MAX_EDGE_PCM_SAMPLES)
            .ok_or_else(|| permanent("Edge-TTS audio exceeds the supported duration"))?;
        pcm.reserve(total_samples.saturating_sub(pcm.len()));
        pcm.extend_from_slice(&decoded);
    }

    if pcm.is_empty() {
        Err(transient("Edge-TTS returned no audio"))
    } else {
        Ok(pcm)
    }
}

pub async fn list_voices(
    client: &reqwest::Client,
) -> std::result::Result<Vec<EdgeVoice>, EdgeTtsError> {
    kothok_edge_tts::init_tls();
    let catalog = tokio::time::timeout(EDGE_TTS_CATALOG_TIMEOUT, fetch_voice_catalog(client))
        .await
        .map_err(|_| transient("Edge-TTS voice refresh timed out"))??;

    let mut voices = catalog
        .into_iter()
        .filter(|voice| {
            !voice.short_name().trim().is_empty()
                && voice.short_name().chars().count() <= 256
                && voice.locale().chars().count() <= 64
                && voice.gender().chars().count() <= 32
                && voice.friendly_name().chars().count() <= 512
        })
        .map(|voice| EdgeVoice {
            id: voice.short_name().to_string(),
            language: voice.locale().to_string(),
            gender: voice.gender().to_ascii_lowercase(),
            description: voice.friendly_name().to_string(),
        })
        .collect::<Vec<_>>();
    voices.sort_by(|left, right| {
        left.language
            .cmp(&right.language)
            .then_with(|| left.id.cmp(&right.id))
    });
    voices.dedup_by(|left, right| left.id == right.id);
    voices.truncate(MAX_EDGE_VOICES);

    if voices.is_empty() {
        Err(permanent("Edge-TTS returned an empty voice catalog"))
    } else {
        Ok(voices)
    }
}

async fn fetch_voice_catalog(
    client: &reqwest::Client,
) -> std::result::Result<Vec<VoiceInfo>, EdgeTtsError> {
    // Use AivoRelay's HTTP client for the catalog endpoint. The upstream crate's
    // raw TLS reader rejects Microsoft's normal EOF when no TLS close_notify is
    // sent, while reqwest safely accepts the complete HTTP response.
    let url = format!(
        "https://speech.platform.bing.com/consumer/speech/synthesize/readaloud/voices/list\
         ?TrustedClientToken={EDGE_TTS_TRUSTED_CLIENT_TOKEN}\
         &Sec-MS-GEC={}\
         &Sec-MS-GEC-Version={EDGE_TTS_GEC_VERSION}",
        kothok_edge_tts::sec_ms_gec(0)
    );
    let response = client
        .get(url)
        .header(reqwest::header::USER_AGENT, "Mozilla/5.0")
        .send()
        .await
        .map_err(|_| transient("Edge-TTS voice refresh request failed"))?;
    let status = response.status();
    if response
        .content_length()
        .is_some_and(|length| length > EDGE_TTS_VOICE_CATALOG_BYTES_LIMIT as u64)
    {
        return Err(permanent("Edge-TTS voice catalog exceeds the safety limit"));
    }

    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk =
            chunk.map_err(|_| transient("Edge-TTS voice refresh response was interrupted"))?;
        if bytes.len().saturating_add(chunk.len()) > EDGE_TTS_VOICE_CATALOG_BYTES_LIMIT {
            return Err(permanent("Edge-TTS voice catalog exceeds the safety limit"));
        }
        bytes.extend_from_slice(&chunk);
    }
    if !status.is_success() {
        return Err(transient(format!(
            "Edge-TTS voice refresh failed with HTTP {status}"
        )));
    }

    serde_json::from_slice(&bytes)
        .map_err(|_| transient("Edge-TTS returned an invalid voice catalog"))
}

fn collect_bounded_media(
    events: Vec<NativeTtsEvent>,
) -> std::result::Result<Vec<u8>, EdgeTtsError> {
    let mut media = Vec::new();
    for event in events {
        if let NativeTtsEvent::Audio(bytes) = event {
            let next_len = media
                .len()
                .checked_add(bytes.len())
                .filter(|length| *length <= MAX_EDGE_MEDIA_BYTES)
                .ok_or_else(|| permanent("Edge-TTS audio exceeded the safety limit"))?;
            media.reserve(next_len.saturating_sub(media.len()));
            media.extend_from_slice(&bytes);
        }
    }
    if media.is_empty() {
        Err(transient("Edge-TTS returned no MP3 audio"))
    } else {
        Ok(media)
    }
}

fn split_native_segments(text: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut segment = String::new();
    let mut escaped_bytes = 0usize;

    for character in text.chars() {
        let character_bytes = escaped_character_bytes(character);
        if !segment.is_empty()
            && escaped_bytes.saturating_add(character_bytes) > EDGE_TTS_ESCAPED_TEXT_BYTES_LIMIT
        {
            segments.push(std::mem::take(&mut segment));
            escaped_bytes = 0;
        }
        segment.push(character);
        escaped_bytes += character_bytes;
    }
    if !segment.is_empty() {
        segments.push(segment);
    }
    segments
}

fn escaped_character_bytes(character: char) -> usize {
    match character {
        '&' => "&amp;".len(),
        '<' => "&lt;".len(),
        '>' => "&gt;".len(),
        '\'' => "&apos;".len(),
        '"' => "&quot;".len(),
        character if matches!(character as u32, 0..=8 | 11..=12 | 14..=31) => 1,
        character => character.len_utf8(),
    }
}

fn decode_edge_mp3(media: Vec<u8>) -> Result<Vec<i16>> {
    let byte_len = media.len() as u64;
    if byte_len == 0 || byte_len > MAX_EDGE_MEDIA_BYTES as u64 {
        return Err(anyhow!("invalid MP3 byte length"));
    }
    let source = rodio::Decoder::builder()
        .with_data(BufReader::new(Cursor::new(media)))
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

fn native_failure(prefix: &str, error: NativeEdgeTtsError) -> EdgeTtsError {
    let transient_error = matches!(
        &error,
        NativeEdgeTtsError::Connect(_)
            | NativeEdgeTtsError::VoiceFetch(_)
            | NativeEdgeTtsError::Ws(_)
            | NativeEdgeTtsError::Io(_)
            | NativeEdgeTtsError::NoAudio
    );
    let detail = error
        .to_string()
        .chars()
        .filter(|character| !character.is_control())
        .take(1_024)
        .collect::<String>();
    EdgeTtsError {
        safe_message: if detail.is_empty() {
            prefix.to_string()
        } else {
            format!("{prefix}: {detail}")
        },
        transient: transient_error,
    }
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
    fn splits_native_text_by_post_escape_utf8_size_without_data_loss() {
        let source = format!("{}{}", "界".repeat(1_500), "&\"'<>".repeat(500));
        let segments = split_native_segments(&source);

        assert_eq!(segments.concat(), source);
        assert!(segments.len() > 1);
        assert!(segments.iter().all(|segment| {
            segment.chars().map(escaped_character_bytes).sum::<usize>()
                <= EDGE_TTS_ESCAPED_TEXT_BYTES_LIMIT
        }));
    }

    #[test]
    fn voice_language_uses_the_bcp_47_prefix() {
        assert_eq!(voice_language("en-US-AriaNeural"), "en-US");
        assert_eq!(voice_language("zh-CN-liaoning-XiaobeiNeural"), "zh-CN");
        assert!(voice_language("custom").is_empty());
    }

    #[tokio::test]
    #[ignore = "requires the live Microsoft Edge Read Aloud service"]
    async fn live_voice_catalog_contains_the_default_voice() {
        let client = reqwest::Client::builder()
            .timeout(EDGE_TTS_CATALOG_TIMEOUT)
            .build()
            .expect("HTTP client");
        let voices = list_voices(&client).await.expect("live voice catalog");

        assert!(voices.len() <= MAX_EDGE_VOICES);
        assert!(voices
            .iter()
            .any(|voice| voice.id == DEFAULT_EDGE_TTS_VOICE));
    }
}
