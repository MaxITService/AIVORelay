use anyhow::{anyhow, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tauri::AppHandle;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::{connect_async, tungstenite::Message};

const DEEPGRAM_WS_BASE_URL: &str = "wss://api.deepgram.com/v1/listen";
const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 10;
const DEFAULT_READ_TIMEOUT_SECS: u64 = 60;
const MIN_TIMEOUT_SECONDS: u32 = 5;
const AUDIO_CHUNK_SIZE_BYTES: usize = 32 * 1024;

#[derive(Debug, Clone, Default)]
pub struct DeepgramTranscriptionOptions {
    pub interim_results: Option<bool>,
    pub smart_format: Option<bool>,
    pub diarize: Option<bool>,
}

pub struct DeepgramSttManager {
    /// Monotonically increasing operation ID; when cancel() is called, all
    /// operations started before that point should abort.
    current_operation_id: AtomicU64,
    /// The operation ID at the time cancel() was last called.
    cancelled_before_id: AtomicU64,
}

impl DeepgramSttManager {
    pub fn new(_app_handle: &AppHandle) -> Result<Self> {
        Ok(Self {
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
        self.cancelled_before_id.store(current + 1, Ordering::SeqCst);
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

    fn extract_alternative<'a>(payload: &'a Value) -> Option<&'a Value> {
        payload
            .get("channel")
            .and_then(|ch| ch.get("alternatives"))
            .and_then(|alts| alts.as_array())
            .and_then(|alts| alts.first())
    }

    fn extract_transcript(alternative: Option<&Value>) -> String {
        alternative
            .and_then(|alt| alt.get("transcript"))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .trim()
            .to_string()
    }

    fn extract_speaker_id(word: &Value) -> Option<u32> {
        let speaker = word.get("speaker")?;
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

    fn format_diarized_chunk(alternative: Option<&Value>) -> Option<String> {
        let words = alternative?.get("words")?.as_array()?;
        let mut groups: Vec<(u32, Vec<String>)> = Vec::new();

        for word in words {
            let Some(speaker) = Self::extract_speaker_id(word) else {
                continue;
            };
            let token = word
                .get("punctuated_word")
                .and_then(|v| v.as_str())
                .or_else(|| word.get("word").and_then(|v| v.as_str()))
                .unwrap_or_default()
                .trim();
            if token.is_empty() {
                continue;
            }

            if let Some((last_speaker, tokens)) = groups.last_mut() {
                if *last_speaker == speaker {
                    tokens.push(token.to_string());
                    continue;
                }
            }
            groups.push((speaker, vec![token.to_string()]));
        }

        if groups.is_empty() {
            return None;
        }

        let mut formatted = String::new();
        for (speaker, tokens) in groups {
            if !formatted.is_empty() {
                formatted.push('\n');
            }
            formatted.push_str(&format!("[Speaker {}] {}", speaker, tokens.join(" ")));
        }
        Some(formatted)
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
            .send(Message::Text(r#"{"type":"CloseStream"}"#.to_string().into()))
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
                        anyhow!("Invalid Deepgram WebSocket payload: {} (body: {})", e, preview)
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
