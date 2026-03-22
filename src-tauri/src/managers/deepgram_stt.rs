use crate::file_transcription_diarization::RawSpeakerBlock;
use anyhow::{anyhow, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::error::Error as _;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tauri::AppHandle;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::{connect_async, tungstenite::Message};

const DEEPGRAM_WS_BASE_URL: &str = "wss://api.deepgram.com/v1/listen";
const DEEPGRAM_HTTP_BASE_URL: &str = "https://api.deepgram.com/v1/listen";
const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 10;
const DEFAULT_READ_TIMEOUT_SECS: u64 = 60;
const MIN_TIMEOUT_SECONDS: u32 = 5;
const AUDIO_CHUNK_SIZE_BYTES: usize = 32 * 1024;

#[derive(Debug, Clone, Default)]
pub struct DeepgramTranscriptionOptions {
    pub interim_results: Option<bool>,
    pub smart_format: Option<bool>,
    pub diarize: Option<bool>,
    pub multichannel: Option<bool>,
}

pub struct DeepgramSttManager {
    client: reqwest::Client,
    /// Monotonically increasing operation ID; when cancel() is called, all
    /// operations started before that point should abort.
    current_operation_id: AtomicU64,
    /// The operation ID at the time cancel() was last called.
    cancelled_before_id: AtomicU64,
}

#[derive(Debug, Clone, Default)]
pub struct DeepgramPrerecordedTranscription {
    pub text: String,
    pub speaker_blocks: Vec<RawSpeakerBlock>,
}

impl DeepgramSttManager {
    pub fn new(_app_handle: &AppHandle) -> Result<Self> {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(DEFAULT_CONNECT_TIMEOUT_SECS))
            .build()
            .map_err(|e| anyhow!("Failed to build Deepgram HTTP client: {}", e))?;

        Ok(Self {
            client,
            current_operation_id: AtomicU64::new(0),
            cancelled_before_id: AtomicU64::new(0),
        })
    }

    /// Returns a new operation ID for tracking cancellation.
    pub fn start_operation(&self) -> u64 {
        self.current_operation_id.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Marks all operations started before now as cancelled.
    pub fn cancel(&self) {
        let current = self.current_operation_id.load(Ordering::SeqCst);
        self.cancelled_before_id
            .store(current + 1, Ordering::SeqCst);
        log::info!(
            "DeepgramSttManager: cancelled all operations up to id {}",
            current + 1
        );
    }

    /// Returns true if the given operation ID has been cancelled.
    pub fn is_cancelled(&self, operation_id: u64) -> bool {
        operation_id < self.cancelled_before_id.load(Ordering::SeqCst)
    }

    fn ensure_not_cancelled(&self, operation_id: Option<u64>) -> Result<()> {
        if let Some(op_id) = operation_id {
            if self.is_cancelled(op_id) {
                return Err(anyhow!("Transcription cancelled"));
            }
        }
        Ok(())
    }

    fn normalize_model(model: &str) -> String {
        let trimmed = model.trim();
        if trimmed.is_empty() {
            return "nova-3".to_string();
        }
        trimmed.to_string()
    }

    fn default_language_for_model(model: &str) -> &'static str {
        if model.trim().eq_ignore_ascii_case("nova-3-medical") {
            "en"
        } else {
            "multi"
        }
    }

    fn normalize_language(language: Option<&str>, model: &str) -> Option<String> {
        let mut lang = language.unwrap_or_default().trim().to_string();
        if lang.is_empty() || lang.eq_ignore_ascii_case("auto") {
            return Some(Self::default_language_for_model(model).to_string());
        }

        if lang.eq_ignore_ascii_case("os_input") {
            if let Some(resolved) = crate::input_source::get_language_from_input_source() {
                lang = resolved;
            } else {
                return Some(Self::default_language_for_model(model).to_string());
            }
        }

        if lang == "zh-Hans" || lang == "zh-Hant" {
            return Some("zh".to_string());
        }

        Some(lang)
    }

    fn remaining_timeout(start: Instant, timeout_seconds: u32) -> Result<Duration> {
        let total = Duration::from_secs(timeout_seconds.max(MIN_TIMEOUT_SECONDS) as u64);
        let elapsed = start.elapsed();
        if elapsed >= total {
            return Err(anyhow!(
                "Transcription timed out after {} seconds",
                timeout_seconds
            ));
        }
        Ok(total - elapsed)
    }

    fn build_ws_url(
        model: &str,
        language: Option<&str>,
        options: &DeepgramTranscriptionOptions,
    ) -> Result<String> {
        let mut url = reqwest::Url::parse(DEEPGRAM_WS_BASE_URL)
            .map_err(|e| anyhow!("Invalid Deepgram WebSocket URL: {}", e))?;
        {
            let mut qp = url.query_pairs_mut();
            qp.append_pair("model", model);
            qp.append_pair("encoding", "linear16");
            qp.append_pair("sample_rate", "16000");
            qp.append_pair("channels", "1");
            qp.append_pair(
                "smart_format",
                if options.smart_format.unwrap_or(true) {
                    "true"
                } else {
                    "false"
                },
            );
            qp.append_pair(
                "interim_results",
                if options.interim_results.unwrap_or(false) {
                    "true"
                } else {
                    "false"
                },
            );
            qp.append_pair(
                "diarize",
                if options.diarize.unwrap_or(false) {
                    "true"
                } else {
                    "false"
                },
            );

            if let Some(normalized_language) = Self::normalize_language(language, model) {
                qp.append_pair("language", &normalized_language);
            }
        }
        Ok(url.to_string())
    }

    fn build_prerecorded_url(
        model: &str,
        language: Option<&str>,
        options: &DeepgramTranscriptionOptions,
    ) -> Result<String> {
        let mut url = reqwest::Url::parse(DEEPGRAM_HTTP_BASE_URL)
            .map_err(|e| anyhow!("Invalid Deepgram HTTP URL: {}", e))?;
        let diarize_enabled = options.diarize.unwrap_or(false);
        let multichannel_enabled = options.multichannel.unwrap_or(false);
        {
            let mut qp = url.query_pairs_mut();
            qp.append_pair("model", model);
            qp.append_pair(
                "smart_format",
                if options.smart_format.unwrap_or(true) {
                    "true"
                } else {
                    "false"
                },
            );
            qp.append_pair("diarize", if diarize_enabled { "true" } else { "false" });
            qp.append_pair(
                "multichannel",
                if multichannel_enabled {
                    "true"
                } else {
                    "false"
                },
            );
            if diarize_enabled {
                qp.append_pair("utterances", "true");
                qp.append_pair("punctuate", "true");
            }

            if let Some(normalized_language) = Self::normalize_language(language, model) {
                qp.append_pair("language", &normalized_language);
            }
        }
        Ok(url.to_string())
    }

    fn extract_alternative<'a>(payload: &'a Value) -> Option<&'a Value> {
        payload
            .get("channel")
            .and_then(|ch| ch.get("alternatives"))
            .and_then(|alts| alts.as_array())
            .and_then(|alts| alts.first())
    }

    fn extract_prerecorded_alternative<'a>(payload: &'a Value) -> Option<&'a Value> {
        payload
            .get("results")
            .and_then(|results| results.get("channels"))
            .and_then(|channels| channels.as_array())
            .and_then(|channels| channels.first())
            .and_then(|channel| channel.get("alternatives"))
            .and_then(|alts| alts.as_array())
            .and_then(|alts| alts.first())
    }

    fn extract_prerecorded_channel_alternatives<'a>(payload: &'a Value) -> Vec<(u32, &'a Value)> {
        payload
            .get("results")
            .and_then(|results| results.get("channels"))
            .and_then(|channels| channels.as_array())
            .map(|channels| {
                channels
                    .iter()
                    .enumerate()
                    .filter_map(|(channel_index, channel)| {
                        channel
                            .get("alternatives")
                            .and_then(|alts| alts.as_array())
                            .and_then(|alts| alts.first())
                            .map(|alternative| (channel_index as u32, alternative))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    }

    fn extract_transcript(alternative: Option<&Value>) -> String {
        alternative
            .and_then(|alt| alt.get("transcript"))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .trim()
            .to_string()
    }

    fn parse_speaker_value(speaker: &Value) -> Option<u32> {
        if let Some(v) = speaker.as_str() {
            return v.trim().parse::<u32>().ok();
        }
        if let Some(v) = speaker.as_u64() {
            return if v <= u32::MAX as u64 {
                Some(v as u32)
            } else {
                None
            };
        }
        if let Some(v) = speaker.as_i64() {
            return if v >= 0 && (v as u64) <= u32::MAX as u64 {
                Some(v as u32)
            } else {
                None
            };
        }
        if let Some(v) = speaker.as_f64() {
            if v.is_finite() && v >= 0.0 && v <= u32::MAX as f64 {
                return Some(v as u32);
            }
        }
        None
    }

    fn extract_speaker_id(word: &Value) -> Option<u32> {
        word.get("speaker").and_then(Self::parse_speaker_value)
    }

    fn parse_channel_value(channel: &Value) -> Option<u32> {
        Self::parse_speaker_value(channel).or_else(|| {
            channel
                .as_array()
                .and_then(|values| values.first())
                .and_then(Self::parse_channel_value)
        })
    }

    fn extract_channel_index(value: &Value) -> Option<u32> {
        value
            .get("channel")
            .and_then(Self::parse_channel_value)
            .or_else(|| {
                value
                    .get("channel_index")
                    .and_then(Self::parse_channel_value)
            })
    }

    fn extract_token_text(value: &Value) -> Option<String> {
        value
            .get("punctuated_word")
            .and_then(|v| v.as_str())
            .or_else(|| value.get("word").and_then(|v| v.as_str()))
            .map(str::trim)
            .filter(|token| !token.is_empty())
            .map(ToOwned::to_owned)
    }

    fn build_transcript_from_words(words: &[Value]) -> String {
        words
            .iter()
            .filter_map(Self::extract_token_text)
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn default_speaker_name(speaker: u32) -> String {
        format!("Speaker {}", speaker)
    }

    fn default_channel_name(channel_index: u32) -> String {
        format!("Channel {}", channel_index)
    }

    fn default_channel_speaker_name(channel_index: u32, speaker: u32) -> String {
        format!("Channel {} Speaker {}", channel_index, speaker)
    }

    fn make_speaker_block(
        speaker: u32,
        channel_index: Option<u32>,
        text: String,
    ) -> RawSpeakerBlock {
        match channel_index {
            Some(channel_index) => RawSpeakerBlock {
                speaker_key: format!("channel:{}:speaker:{}", channel_index, speaker),
                default_name: Some(Self::default_channel_speaker_name(channel_index, speaker)),
                text,
            },
            None => RawSpeakerBlock {
                speaker_key: speaker.to_string(),
                default_name: Some(Self::default_speaker_name(speaker)),
                text,
            },
        }
    }

    fn make_channel_block(channel_index: u32, text: String) -> RawSpeakerBlock {
        RawSpeakerBlock {
            speaker_key: format!("channel:{}", channel_index),
            default_name: Some(Self::default_channel_name(channel_index)),
            text,
        }
    }

    fn extract_diarized_chunk_blocks(
        alternative: Option<&Value>,
        channel_index: Option<u32>,
    ) -> Vec<RawSpeakerBlock> {
        let Some(words) = alternative
            .and_then(|value| value.get("words"))
            .and_then(|value| value.as_array())
        else {
            return Vec::new();
        };
        let mut groups: Vec<RawSpeakerBlock> = Vec::new();

        for word in words {
            let Some(speaker) = Self::extract_speaker_id(word) else {
                continue;
            };
            let Some(token) = Self::extract_token_text(word) else {
                continue;
            };
            if let Some(last_block) = groups.last_mut() {
                let speaker_key = match channel_index {
                    Some(channel_index) => {
                        format!("channel:{}:speaker:{}", channel_index, speaker)
                    }
                    None => speaker.to_string(),
                };
                if last_block.speaker_key == speaker_key {
                    if !last_block.text.is_empty() {
                        last_block.text.push(' ');
                    }
                    last_block.text.push_str(&token);
                    continue;
                }
            }

            groups.push(Self::make_speaker_block(speaker, channel_index, token));
        }

        groups
    }

    fn extract_diarized_utterance_blocks(
        payload: &Value,
        multichannel_enabled: bool,
    ) -> Vec<RawSpeakerBlock> {
        let Some(utterances) = payload
            .get("results")
            .and_then(|results| results.get("utterances"))
            .and_then(|utterances| utterances.as_array())
        else {
            return Vec::new();
        };
        let mut groups: Vec<RawSpeakerBlock> = Vec::new();

        for utterance in utterances {
            let transcript = utterance
                .get("transcript")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .or_else(|| {
                    utterance
                        .get("words")
                        .and_then(|words| words.as_array())
                        .map(|words| Self::build_transcript_from_words(words))
                        .filter(|value| !value.is_empty())
                });
            let Some(transcript) = transcript else {
                continue;
            };

            let speaker = utterance
                .get("speaker")
                .and_then(Self::parse_speaker_value)
                .or_else(|| {
                    utterance
                        .get("words")
                        .and_then(|words| words.as_array())
                        .and_then(|words| words.iter().find_map(Self::extract_speaker_id))
                });
            let Some(speaker) = speaker else {
                continue;
            };
            let channel_index = if multichannel_enabled {
                Self::extract_channel_index(utterance).or_else(|| {
                    utterance
                        .get("words")
                        .and_then(|words| words.as_array())
                        .and_then(|words| words.iter().find_map(Self::extract_channel_index))
                })
            } else {
                None
            };
            if multichannel_enabled && channel_index.is_none() {
                continue;
            }

            if let Some(last_block) = groups.last_mut() {
                let speaker_key = match channel_index {
                    Some(channel_index) => {
                        format!("channel:{}:speaker:{}", channel_index, speaker)
                    }
                    None => speaker.to_string(),
                };
                if last_block.speaker_key == speaker_key {
                    if !last_block.text.is_empty() {
                        last_block.text.push(' ');
                    }
                    last_block.text.push_str(&transcript);
                    continue;
                }
            }

            groups.push(Self::make_speaker_block(speaker, channel_index, transcript));
        }

        groups
    }

    fn extract_multichannel_channel_blocks(payload: &Value) -> Vec<RawSpeakerBlock> {
        let mut blocks = Vec::new();

        for (channel_index, alternative) in Self::extract_prerecorded_channel_alternatives(payload)
        {
            let transcript = Self::extract_transcript(Some(alternative));
            let transcript = if transcript.is_empty() {
                alternative
                    .get("words")
                    .and_then(|words| words.as_array())
                    .map(|words| Self::build_transcript_from_words(words))
                    .filter(|value| !value.is_empty())
                    .unwrap_or_default()
            } else {
                transcript
            };
            if transcript.is_empty() {
                continue;
            }
            blocks.push(Self::make_channel_block(channel_index, transcript));
        }

        blocks
    }

    fn render_speaker_blocks(blocks: &[RawSpeakerBlock]) -> Option<String> {
        if blocks.is_empty() {
            return None;
        }

        Some(
            blocks
                .iter()
                .map(|block| {
                    let speaker_name = block
                        .default_name
                        .as_deref()
                        .filter(|value| !value.trim().is_empty())
                        .unwrap_or(block.speaker_key.as_str());
                    format!("[{}] {}", speaker_name, block.text)
                })
                .collect::<Vec<_>>()
                .join("\n"),
        )
    }

    fn format_diarized_chunk(alternative: Option<&Value>) -> Option<String> {
        Self::render_speaker_blocks(&Self::extract_diarized_chunk_blocks(alternative, None))
    }

    fn format_reqwest_error(error: &reqwest::Error) -> String {
        let mut flags = Vec::new();
        if error.is_connect() {
            flags.push("connect");
        }
        if error.is_timeout() {
            flags.push("timeout");
        }
        if error.is_request() {
            flags.push("request");
        }
        if error.is_body() {
            flags.push("body");
        }
        if error.is_decode() {
            flags.push("decode");
        }
        if error.is_redirect() {
            flags.push("redirect");
        }
        if error.is_status() {
            flags.push("status");
        }

        let kind = if flags.is_empty() {
            "unknown".to_string()
        } else {
            flags.join("|")
        };
        let status = error
            .status()
            .map(|value| format!("; status={}", value))
            .unwrap_or_default();
        let url = error
            .url()
            .map(|value| format!("; url={}", value))
            .unwrap_or_default();

        let mut causes = Vec::new();
        let mut current = error.source();
        while let Some(source) = current {
            causes.push(source.to_string());
            current = source.source();
        }
        let causes = if causes.is_empty() {
            String::new()
        } else {
            format!("; causes={}", causes.join(" | "))
        };

        format!("kind={kind}; error={error}{status}{url}{causes}")
    }

    pub async fn transcribe_prerecorded_bytes(
        &self,
        operation_id: Option<u64>,
        api_key: &str,
        model: &str,
        timeout_seconds: u32,
        audio_bytes: &[u8],
        language: Option<&str>,
        options: DeepgramTranscriptionOptions,
    ) -> Result<DeepgramPrerecordedTranscription> {
        self.ensure_not_cancelled(operation_id)?;
        if api_key.trim().is_empty() {
            return Err(anyhow!("Deepgram API key is missing"));
        }
        if audio_bytes.is_empty() {
            return Ok(DeepgramPrerecordedTranscription::default());
        }

        let model = Self::normalize_model(model);
        let diarize_enabled = options.diarize.unwrap_or(false);
        let multichannel_enabled = options.multichannel.unwrap_or(false);
        let url = Self::build_prerecorded_url(&model, language, &options)?;
        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Token {}", api_key.trim()))
            .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
            .timeout(Duration::from_secs(
                timeout_seconds.max(MIN_TIMEOUT_SECONDS) as u64,
            ))
            .body(audio_bytes.to_vec())
            .send()
            .await
            .map_err(|e| {
                anyhow!(
                    "Deepgram pre-recorded request failed (timeout={}s, audio_bytes={}): {}",
                    timeout_seconds.max(MIN_TIMEOUT_SECONDS),
                    audio_bytes.len(),
                    Self::format_reqwest_error(&e)
                )
            })?;

        self.ensure_not_cancelled(operation_id)?;

        let status = response.status();
        let body = response.text().await.map_err(|e| {
            anyhow!(
                "Failed to read Deepgram pre-recorded response: {}",
                Self::format_reqwest_error(&e)
            )
        })?;

        self.ensure_not_cancelled(operation_id)?;

        if !status.is_success() {
            let preview: String = body.chars().take(400).collect();
            return Err(anyhow!(
                "Deepgram pre-recorded request returned {}: {}",
                status,
                preview
            ));
        }

        let payload: Value = serde_json::from_str(&body).map_err(|e| {
            let preview: String = body.chars().take(400).collect();
            anyhow!(
                "Invalid Deepgram pre-recorded payload: {} (body: {})",
                e,
                preview
            )
        })?;

        let alternative = Self::extract_prerecorded_alternative(&payload);
        let transcript = if multichannel_enabled {
            Self::render_speaker_blocks(&Self::extract_multichannel_channel_blocks(&payload))
                .unwrap_or_else(|| Self::extract_transcript(alternative))
        } else {
            Self::extract_transcript(alternative)
        };
        if diarize_enabled {
            let speaker_blocks = {
                let utterance_blocks =
                    Self::extract_diarized_utterance_blocks(&payload, multichannel_enabled);
                if utterance_blocks.is_empty() {
                    if multichannel_enabled {
                        Self::extract_prerecorded_channel_alternatives(&payload)
                            .into_iter()
                            .flat_map(|(channel_index, alternative)| {
                                Self::extract_diarized_chunk_blocks(
                                    Some(alternative),
                                    Some(channel_index),
                                )
                            })
                            .collect()
                    } else {
                        Self::extract_diarized_chunk_blocks(alternative, None)
                    }
                } else {
                    utterance_blocks
                }
            };
            let text = Self::render_speaker_blocks(&speaker_blocks).unwrap_or(transcript);
            return Ok(DeepgramPrerecordedTranscription {
                text,
                speaker_blocks,
            });
        }

        if multichannel_enabled {
            let speaker_blocks = Self::extract_multichannel_channel_blocks(&payload);
            let text = Self::render_speaker_blocks(&speaker_blocks).unwrap_or(transcript);
            return Ok(DeepgramPrerecordedTranscription {
                text,
                speaker_blocks,
            });
        }

        Ok(DeepgramPrerecordedTranscription {
            text: transcript,
            speaker_blocks: Vec::new(),
        })
    }

    pub async fn transcribe(
        &self,
        operation_id: Option<u64>,
        api_key: &str,
        model: &str,
        timeout_seconds: u32,
        audio_samples: &[f32],
        language: Option<&str>,
        options: DeepgramTranscriptionOptions,
    ) -> Result<String> {
        self.ensure_not_cancelled(operation_id)?;
        if api_key.trim().is_empty() {
            return Err(anyhow!("Deepgram API key is missing"));
        }
        if audio_samples.is_empty() {
            return Ok(String::new());
        }

        let started = Instant::now();
        let diarize_enabled = options.diarize.unwrap_or(false);
        let ws_url = Self::build_ws_url(&Self::normalize_model(model), language, &options)?;

        let mut request = ws_url
            .into_client_request()
            .map_err(|e| anyhow!("Failed to create Deepgram request: {}", e))?;
        request.headers_mut().insert(
            "Authorization",
            format!("Token {}", api_key.trim())
                .parse()
                .map_err(|e| anyhow!("Invalid Deepgram auth header: {}", e))?,
        );

        let (stream, _) = timeout(
            Duration::from_secs(DEFAULT_CONNECT_TIMEOUT_SECS),
            connect_async(request),
        )
        .await
        .map_err(|_| anyhow!("Timed out while connecting to Deepgram WebSocket"))?
        .map_err(|e| anyhow!("Failed to connect to Deepgram WebSocket: {}", e))?;

        let (mut write, mut read) = stream.split();

        let bytes = frame_16khz_mono_to_pcm_s16le_bytes(audio_samples);
        for chunk in bytes.chunks(AUDIO_CHUNK_SIZE_BYTES) {
            self.ensure_not_cancelled(operation_id)?;
            let _ = Self::remaining_timeout(started, timeout_seconds)?;
            write
                .send(Message::Binary(chunk.to_vec().into()))
                .await
                .map_err(|e| anyhow!("Failed to send audio chunk to Deepgram: {}", e))?;
        }

        write
            .send(Message::Text(r#"{"type":"Finalize"}"#.to_string().into()))
            .await
            .map_err(|e| anyhow!("Failed to send Deepgram finalize message: {}", e))?;
        write
            .send(Message::Text(
                r#"{"type":"CloseStream"}"#.to_string().into(),
            ))
            .await
            .map_err(|e| anyhow!("Failed to send Deepgram close stream message: {}", e))?;
        write
            .flush()
            .await
            .map_err(|e| anyhow!("Failed to flush Deepgram WebSocket stream: {}", e))?;

        let mut final_segments: Vec<String> = Vec::new();
        let mut finished = false;

        loop {
            self.ensure_not_cancelled(operation_id)?;
            let wait = std::cmp::min(
                Self::remaining_timeout(started, timeout_seconds)?,
                Duration::from_secs(DEFAULT_READ_TIMEOUT_SECS),
            );
            let frame = timeout(wait, read.next())
                .await
                .map_err(|_| anyhow!("Deepgram WebSocket read timed out"))?;
            let Some(frame) = frame else {
                break;
            };
            let frame = frame.map_err(|e| anyhow!("Deepgram WebSocket read failed: {}", e))?;

            match frame {
                Message::Text(text) => {
                    let payload: Value = serde_json::from_str(text.as_ref()).map_err(|e| {
                        let preview: String = text.chars().take(200).collect();
                        anyhow!(
                            "Invalid Deepgram WebSocket payload: {} (body: {})",
                            e,
                            preview
                        )
                    })?;

                    let msg_type = payload
                        .get("type")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default();
                    if msg_type == "Metadata" {
                        finished = true;
                        break;
                    }
                    if msg_type != "Results" {
                        continue;
                    }

                    let alternative = Self::extract_alternative(&payload);
                    let transcript = Self::extract_transcript(alternative);
                    let is_final = payload
                        .get("is_final")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    if is_final {
                        let final_chunk = if diarize_enabled {
                            Self::format_diarized_chunk(alternative).unwrap_or(transcript)
                        } else {
                            transcript
                        };
                        if !final_chunk.is_empty() {
                            final_segments.push(final_chunk);
                        }
                    }
                }
                Message::Ping(_) | Message::Pong(_) | Message::Binary(_) => {}
                Message::Close(_) => {
                    finished = true;
                    break;
                }
                _ => {}
            }
        }

        if !finished && final_segments.is_empty() {
            return Err(anyhow!(
                "Deepgram WebSocket transcription did not report completion"
            ));
        }

        let separator = if diarize_enabled { "\n" } else { " " };
        Ok(final_segments.join(separator))
    }
}

fn frame_16khz_mono_to_pcm_s16le_bytes(frame: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(frame.len() * 2);
    for sample in frame {
        let clamped = sample.clamp(-1.0, 1.0);
        let value = (clamped * i16::MAX as f32).round() as i16;
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}
