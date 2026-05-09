use crate::audio_toolkit::encode_wav_bytes;
use crate::settings::{RemoteSttDebugMode, RemoteSttSettings};
use crate::url_security::{
    infer_remote_stt_preset, validate_remote_stt_base_url, REMOTE_STT_OPENAI_BASE_URL,
    REMOTE_STT_PRESET_CUSTOM, REMOTE_STT_PRESET_GROQ, REMOTE_STT_PRESET_OPENAI,
};
use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use futures_util::{SinkExt, Stream, StreamExt};
use serde::Deserialize;
use serde_json::Value;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::{connect_async, tungstenite::Message};

/// Default timeout for Remote STT requests (60 seconds)
const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 60;
/// Default connection timeout (10 seconds)
const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 10;

const REMOTE_STT_SERVICE: &str = "fi.maxits.aivorelay";
const REMOTE_STT_USER_PREFIX: &str = "remote_stt_api_key";
const OPENAI_REALTIME_MODEL: &str = "gpt-realtime-2";
const OPENAI_REALTIME_TRANSLATE_MODEL: &str = "gpt-realtime-translate";
const OPENAI_REALTIME_WS_URL: &str = "wss://api.openai.com/v1/realtime?model=gpt-realtime-2";
const OPENAI_REALTIME_TRANSLATE_WS_URL: &str =
    "wss://api.openai.com/v1/realtime/translations?model=gpt-realtime-translate";
const OPENAI_REALTIME_AUDIO_CHUNK_BYTES: usize = 48_000;
const OPENAI_REALTIME_AGENT_DEFAULT_PROMPT: &str =
    "Additional context for speech-to-text transcription. \
     Current language setting: ${language}. Translate to English: ${translate_to_english}. \
     Preserve the speaker's language unless translation is enabled. \
     Use context to create proper punctuation and fix recognition errors only when the intended words are recoverable from audio and context. \
     If speech is not recoverable because of microphone noise, speech defects, or background noise, use [⚠️inaudible⚠️] instead of guessing. \
     The user may provide custom words that are rare in the language; try to recognize them properly. \
     Make sure to properly recognize names, product names, and vocabulary exactly when recognizable.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RemoteSttApiKeySource {
    Scoped,
    Legacy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RemoteSttApiKey {
    value: String,
    source: RemoteSttApiKeySource,
}

/// Languages supported by Whisper models (ISO 639-1 codes)
/// Based on OpenAI Whisper documentation and Groq's supported languages list
/// https://github.com/openai/whisper/blob/main/whisper/tokenizer.py
const WHISPER_SUPPORTED_LANGUAGES: &[&str] = &[
    "af",  // Afrikaans
    "am",  // Amharic
    "ar",  // Arabic
    "as",  // Assamese
    "az",  // Azerbaijani
    "ba",  // Bashkir
    "be",  // Belarusian
    "bg",  // Bulgarian
    "bn",  // Bengali
    "bo",  // Tibetan
    "br",  // Breton
    "bs",  // Bosnian
    "ca",  // Catalan
    "cs",  // Czech
    "cy",  // Welsh
    "da",  // Danish
    "de",  // German
    "el",  // Greek
    "en",  // English
    "es",  // Spanish
    "et",  // Estonian
    "eu",  // Basque
    "fa",  // Persian
    "fi",  // Finnish
    "fo",  // Faroese
    "fr",  // French
    "gl",  // Galician
    "gu",  // Gujarati
    "ha",  // Hausa
    "haw", // Hawaiian
    "he",  // Hebrew
    "hi",  // Hindi
    "hr",  // Croatian
    "ht",  // Haitian Creole
    "hu",  // Hungarian
    "hy",  // Armenian
    "id",  // Indonesian
    "is",  // Icelandic
    "it",  // Italian
    "ja",  // Japanese
    "jv",  // Javanese
    "ka",  // Georgian
    "kk",  // Kazakh
    "km",  // Khmer
    "kn",  // Kannada
    "ko",  // Korean
    "la",  // Latin
    "lb",  // Luxembourgish
    "ln",  // Lingala
    "lo",  // Lao
    "lt",  // Lithuanian
    "lv",  // Latvian
    "mg",  // Malagasy
    "mi",  // Maori
    "mk",  // Macedonian
    "ml",  // Malayalam
    "mn",  // Mongolian
    "mr",  // Marathi
    "ms",  // Malay
    "mt",  // Maltese
    "my",  // Myanmar (Burmese)
    "ne",  // Nepali
    "nl",  // Dutch
    "nn",  // Norwegian Nynorsk
    "no",  // Norwegian
    "oc",  // Occitan
    "pa",  // Punjabi
    "pl",  // Polish
    "ps",  // Pashto
    "pt",  // Portuguese
    "ro",  // Romanian
    "ru",  // Russian
    "sa",  // Sanskrit
    "sd",  // Sindhi
    "si",  // Sinhala
    "sk",  // Slovak
    "sl",  // Slovenian
    "sn",  // Shona
    "so",  // Somali
    "sq",  // Albanian
    "sr",  // Serbian
    "su",  // Sundanese
    "sv",  // Swedish
    "sw",  // Swahili
    "ta",  // Tamil
    "te",  // Telugu
    "tg",  // Tajik
    "th",  // Thai
    "tk",  // Turkmen
    "tl",  // Tagalog
    "tr",  // Turkish
    "tt",  // Tatar
    "uk",  // Ukrainian
    "ur",  // Urdu
    "uz",  // Uzbek
    "vi",  // Vietnamese
    "yi",  // Yiddish
    "yo",  // Yoruba
    "yue", // Cantonese
    "zh",  // Chinese
];

/// Check if a language code is supported by Whisper models
fn is_whisper_supported_language(lang: &str) -> bool {
    WHISPER_SUPPORTED_LANGUAGES.contains(&lang)
}

#[derive(Debug, Deserialize)]
struct TranscriptionResponse {
    text: String,
}

/// Returns the known character limit for a model's prompt parameter.
/// Returns None if the model is unknown (no limit enforced by us, API may handle).
pub fn get_model_prompt_limit(model_id: &str) -> Option<usize> {
    let lower = model_id.to_lowercase();

    // Groq Whisper models - 896 character limit
    // https://console.groq.com/docs/speech-to-text
    if lower.contains("whisper") {
        return Some(896);
    }

    // OpenAI whisper-1 - also uses ~224 tokens ≈ 896 chars
    if lower == "whisper-1" {
        return Some(896);
    }

    // Deepgram - supports longer prompts (based on their docs)
    if lower.contains("deepgram") || lower.contains("nova") {
        return Some(2000);
    }

    // Unknown model - no limit enforced by us
    // Let the API handle it and return error if needed
    None
}

/// Returns whether a remote STT model supports translation to English.
/// Uses the OpenAI-compatible /audio/translations endpoint.
///
/// Known model support:
/// - Groq: whisper-large-v3 supports translation, whisper-large-v3-turbo does NOT
/// - OpenAI: whisper-1, gpt-realtime-2, and gpt-realtime-translate support translation
/// - Unknown models default to false (safe fallback)
pub fn supports_translation(model_id: &str) -> bool {
    let lower = model_id.to_lowercase();

    // Groq whisper-large-v3-turbo does NOT support translation
    // https://console.groq.com/docs/speech-to-text
    if lower.contains("whisper") && lower.contains("turbo") {
        return false;
    }

    // Groq whisper-large-v3 supports translation
    if lower.contains("whisper-large-v3") {
        return true;
    }

    // OpenAI whisper-1 and GPT Realtime 2 support /audio/translations.
    if lower == "whisper-1" || lower == "gpt-realtime-2" || lower == "gpt-realtime-translate" {
        return true;
    }

    // Generic whisper models (e.g., self-hosted) - assume they support translation
    if lower.contains("whisper") && !lower.contains("turbo") {
        return true;
    }

    // Deepgram, Parakeet, and other non-Whisper models don't use OpenAI translation endpoint
    false
}

fn is_openai_realtime_model(model_id: &str) -> bool {
    model_id.trim().eq_ignore_ascii_case(OPENAI_REALTIME_MODEL)
}

fn is_openai_realtime_translate_model(model_id: &str) -> bool {
    model_id
        .trim()
        .eq_ignore_ascii_case(OPENAI_REALTIME_TRANSLATE_MODEL)
}

fn resample_16khz_f32_to_24khz_pcm16(samples: &[f32]) -> Vec<u8> {
    if samples.is_empty() {
        return Vec::new();
    }

    let output_len = samples.len().saturating_mul(3) / 2;
    let mut out = Vec::with_capacity(output_len.saturating_mul(2));
    for out_index in 0..output_len {
        let src_numerator = out_index.saturating_mul(2);
        let src_index = src_numerator / 3;
        let frac = (src_numerator % 3) as f32 / 3.0;
        let left = samples.get(src_index).copied().unwrap_or(0.0);
        let right = samples.get(src_index + 1).copied().unwrap_or(left);
        let sample = left + (right - left) * frac;
        let pcm = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        out.extend_from_slice(&pcm.to_le_bytes());
    }
    out
}

fn build_openai_realtime_agent_transcription_prompt(
    prompt: Option<String>,
    language: Option<String>,
    translate_to_english: bool,
) -> String {
    let task = if translate_to_english {
        "Translate the user's spoken audio into English."
    } else {
        "Transcribe the user's spoken audio in the original language."
    };
    let language_for_template = resolve_realtime_agent_language_for_prompt(language.as_deref());
    let language_hint = language_for_template
        .clone()
        .filter(|lang| !lang.trim().is_empty() && !lang.eq_ignore_ascii_case("auto"))
        .map(|lang| format!("\nLanguage hint: {}.", lang))
        .unwrap_or_default();
    let prompt_text = prompt
        .filter(|text| !text.trim().is_empty())
        .unwrap_or_else(|| OPENAI_REALTIME_AGENT_DEFAULT_PROMPT.to_string());
    let prompt_hint = format!(
        "\nAdditional STT instructions/context: {}",
        apply_realtime_agent_prompt_vars(
            prompt_text.trim(),
            language_for_template.as_deref(),
            translate_to_english,
        )
    );

    format!(
        "You are being used as a speech-to-text engine inside AivoRelay STT application. \
         {} Output ONLY the final transcript text. Do not answer the speaker, \
         summarize, explain, add labels, add Markdown, or mention that you are an AI. \
         If a word is unclear, use [⚠️inaudible⚠️].{}{}",
        task, language_hint, prompt_hint
    )
}

fn resolve_realtime_agent_language_for_prompt(language: Option<&str>) -> Option<String> {
    let requested = language?.trim();
    if requested.is_empty() {
        return None;
    }
    if requested.eq_ignore_ascii_case("os_input") {
        return crate::input_source::get_language_from_input_source()
            .or_else(|| Some("os_input".to_string()));
    }
    Some(requested.to_string())
}

fn apply_realtime_agent_prompt_vars(
    template: &str,
    language: Option<&str>,
    translate_to_english: bool,
) -> String {
    template
        .replace("${language}", language.unwrap_or("auto"))
        .replace("${translate_to_english}", &translate_to_english.to_string())
}

fn resolve_explicit_realtime_language(language: Option<String>) -> Option<String> {
    let mut lang = language?;
    if lang == "os_input" || lang == "auto" {
        lang = crate::input_source::get_language_from_input_source()?;
    }
    if lang.trim().is_empty() {
        return None;
    }
    if lang == "zh-Hans" || lang == "zh-Hant" {
        return Some("zh".to_string());
    }
    Some(lang)
}

#[derive(Default)]
struct DebugBuffer {
    lines: VecDeque<String>,
    cap_normal: usize,
    cap_verbose: usize,
}

impl DebugBuffer {
    fn new() -> Self {
        Self {
            lines: VecDeque::new(),
            cap_normal: 50,
            cap_verbose: 300,
        }
    }

    fn push_line(&mut self, line: String, mode: RemoteSttDebugMode) {
        let cap = match mode {
            RemoteSttDebugMode::Verbose => self.cap_verbose,
            RemoteSttDebugMode::Normal => self.cap_normal,
        };

        self.lines.push_back(line);
        while self.lines.len() > cap {
            self.lines.pop_front();
        }
    }
}

pub struct RemoteSttManager {
    client: reqwest::Client,
    debug: Mutex<DebugBuffer>,
    app_handle: AppHandle,
    /// Monotonically increasing operation ID; when cancel() is called, all
    /// operations started before that point should abort.
    current_operation_id: AtomicU64,
    /// The operation ID at the time cancel() was last called.
    cancelled_before_id: AtomicU64,
}

impl RemoteSttManager {
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(DEFAULT_CONNECT_TIMEOUT_SECS))
            .timeout(Duration::from_secs(DEFAULT_REQUEST_TIMEOUT_SECS))
            .build()
            .map_err(|e| anyhow!("Failed to build HTTP client: {}", e))?;

        Ok(Self {
            client,
            debug: Mutex::new(DebugBuffer::new()),
            app_handle: app_handle.clone(),
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
            "RemoteSttManager: cancelled all operations up to id {}",
            current + 1
        );
    }

    /// Returns true if the given operation ID has been cancelled.
    pub fn is_cancelled(&self, operation_id: u64) -> bool {
        operation_id < self.cancelled_before_id.load(Ordering::SeqCst)
    }

    pub fn get_debug_dump(&self) -> Vec<String> {
        let buffer = self.debug.lock().unwrap();
        buffer.lines.iter().cloned().collect()
    }

    pub fn clear_debug(&self) {
        let mut buffer = self.debug.lock().unwrap();
        buffer.lines.clear();
    }

    fn record_line(&self, settings: &RemoteSttSettings, line: String, is_error: bool) {
        if !settings.debug_capture {
            return;
        }

        if settings.debug_mode == RemoteSttDebugMode::Normal && !is_error {
            return;
        }

        {
            let mut buffer = self.debug.lock().unwrap();
            buffer.push_line(line.clone(), settings.debug_mode);
        }

        let _ = self.app_handle.emit("remote-stt-debug-line", line);
    }

    fn record_info(&self, settings: &RemoteSttSettings, line: String) {
        self.record_line(settings, line, false);
    }

    fn record_error(&self, settings: &RemoteSttSettings, line: String) {
        self.record_line(settings, line, true);
    }

    pub async fn transcribe(
        &self,
        settings: &RemoteSttSettings,
        audio_samples: &[f32],
        prompt: Option<String>,
        language: Option<String>,
        translate_to_english: bool,
    ) -> Result<String> {
        if audio_samples.is_empty() {
            return Ok(String::new());
        }

        let base_url = validate_remote_stt_base_url(settings, None).map_err(|message| {
            self.record_error(settings, message.clone());
            anyhow!(message)
        })?;

        if settings.model_id.trim().is_empty() {
            let message = "Remote STT model ID is empty".to_string();
            self.record_error(settings, message.clone());
            return Err(anyhow!(message));
        }

        let api_key = get_remote_stt_api_key_for_request(settings).map_err(|e| {
            let message = format!("Remote STT API key unavailable: {}", e);
            self.record_error(settings, message.clone());
            anyhow!(message)
        })?;

        if is_openai_realtime_model(&settings.model_id) {
            if base_url != REMOTE_STT_OPENAI_BASE_URL {
                let message = format!(
                    "{} requires the OpenAI Remote STT preset at {}.",
                    settings.model_id, REMOTE_STT_OPENAI_BASE_URL
                );
                self.record_error(settings, message.clone());
                return Err(anyhow!(message));
            }

            let result = self
                .transcribe_openai_realtime_agent(
                    settings,
                    audio_samples,
                    prompt,
                    language,
                    translate_to_english,
                    &api_key.value,
                )
                .await;
            return self.migrate_legacy_api_key_after_success(settings, &api_key, result);
        }

        if is_openai_realtime_translate_model(&settings.model_id) {
            if base_url != REMOTE_STT_OPENAI_BASE_URL {
                let message = format!(
                    "{} requires the OpenAI Remote STT preset at {}.",
                    settings.model_id, REMOTE_STT_OPENAI_BASE_URL
                );
                self.record_error(settings, message.clone());
                return Err(anyhow!(message));
            }

            let result = self
                .transcribe_openai_realtime_translate(
                    settings,
                    audio_samples,
                    prompt,
                    language,
                    translate_to_english,
                    &api_key.value,
                )
                .await;
            return self.migrate_legacy_api_key_after_success(settings, &api_key, result);
        }

        let wav_bytes = encode_wav_bytes(audio_samples).map_err(|e| {
            let message = format!("Failed to encode WAV: {}", e);
            self.record_error(settings, message.clone());
            anyhow!(message)
        })?;

        let file_size = wav_bytes.len();

        // Use /audio/translations endpoint if translate_to_english is enabled AND model supports it
        // Otherwise use /audio/transcriptions (default behavior)
        let use_translation = translate_to_english && supports_translation(&settings.model_id);
        let endpoint = if use_translation {
            "translations"
        } else {
            "transcriptions"
        };
        let url = format!("{}/audio/{}", base_url, endpoint);

        if settings.debug_mode == RemoteSttDebugMode::Verbose {
            self.record_info(
                settings,
                format!(
                    "Remote STT request base_url={} model={} bytes={} endpoint={}",
                    base_url, settings.model_id, file_size, endpoint
                ),
            );
        }

        let mut form = reqwest::multipart::Form::new()
            .text("model", settings.model_id.clone())
            .text("response_format", "json".to_string())
            .part(
                "file",
                reqwest::multipart::Part::bytes(wav_bytes)
                    .file_name("audio.wav")
                    .mime_str("audio/wav")
                    .map_err(|e| anyhow!("Failed to build multipart file: {}", e))?,
            );

        if let Some(mut lang) = language {
            if lang != "auto" {
                // Handle "os_input" - resolve to current keyboard layout language
                if lang == "os_input" {
                    if let Some(resolved) = crate::input_source::get_language_from_input_source() {
                        // Only use resolved language if it's supported by Whisper
                        if is_whisper_supported_language(&resolved) {
                            lang = resolved;
                        } else {
                            // Unsupported language - fall back to auto-detect
                            log::debug!(
                                "OS keyboard language '{}' is not supported by Whisper, using auto-detect",
                                resolved
                            );
                            lang = "auto".to_string();
                        }
                    } else {
                        // Fall back to auto-detect if OS language can't be determined
                        lang = "auto".to_string();
                    }
                }

                // Skip "auto" - let API auto-detect
                if lang != "auto" {
                    // Normalize language code for OpenAI/Whisper
                    // Convert zh-Hans and zh-Hant to zh since Whisper uses ISO 639-1 codes
                    if lang == "zh-Hans" || lang == "zh-Hant" {
                        lang = "zh".to_string();
                    }
                    form = form.text("language", lang);
                }
            }
        }

        // Check prompt against known model limits
        // For known models: validate limit upfront and return user-friendly error
        // For unknown models: pass through, let API handle (and parse error if returned)
        if let Some(p) = prompt {
            let trimmed = p.trim();
            if !trimmed.is_empty() {
                // Get the limit for this model (if known)
                let model_limit = get_model_prompt_limit(&settings.model_id);

                if let Some(limit) = model_limit {
                    if trimmed.len() > limit {
                        let message = format!(
                            "System prompt is too long ({} characters). The {} model has a limit of {} characters. Please shorten your prompt.",
                            trimmed.len(),
                            settings.model_id,
                            limit
                        );
                        self.record_error(settings, message.clone());
                        return Err(anyhow!(message));
                    }
                }

                form = form.text("prompt", trimmed.to_string());
            }
        }

        let start = Instant::now();
        let response = self
            .client
            .post(url)
            .bearer_auth(&api_key.value)
            .multipart(form)
            .send()
            .await
            .map_err(|e| {
                let message = format!("Remote STT request failed: {}", e);
                self.record_error(settings, message.clone());
                anyhow!(message)
            })?;

        let status = response.status();
        let body = response.bytes().await.map_err(|e| {
            let message = format!("Remote STT response read failed: {}", e);
            self.record_error(settings, message.clone());
            anyhow!(message)
        })?;
        let elapsed_ms = start.elapsed().as_millis();

        if settings.debug_mode == RemoteSttDebugMode::Verbose {
            self.record_info(
                settings,
                format!(
                    "Remote STT response status={} elapsed_ms={}",
                    status, elapsed_ms
                ),
            );
        }

        if !status.is_success() {
            let snippet = String::from_utf8_lossy(&body);
            let snippet = snippet.chars().take(500).collect::<String>();
            let message = format!(
                "Remote STT failed: status={} elapsed_ms={} body_snippet={}",
                status, elapsed_ms, snippet
            );
            self.record_error(settings, message.clone());
            return Err(anyhow!(message));
        }

        let parsed: TranscriptionResponse = serde_json::from_slice(&body).map_err(|e| {
            let message = format!("Remote STT response parse failed: {}", e);
            self.record_error(settings, message.clone());
            anyhow!(message)
        })?;

        if settings.debug_mode == RemoteSttDebugMode::Verbose {
            self.record_info(
                settings,
                format!("Remote STT success output_len={}", parsed.text.len()),
            );
        }

        self.migrate_legacy_api_key_after_success(settings, &api_key, Ok(()))?;
        Ok(parsed.text)
    }

    fn migrate_legacy_api_key_after_success<T>(
        &self,
        settings: &RemoteSttSettings,
        api_key: &RemoteSttApiKey,
        result: Result<T>,
    ) -> Result<T> {
        let outcome = result?;
        if let Err(e) = migrate_remote_stt_legacy_api_key_after_success(settings, api_key) {
            log::warn!(
                "Failed to migrate legacy Remote STT API key after success: {}",
                e
            );
        }
        Ok(outcome)
    }

    async fn transcribe_openai_realtime_agent(
        &self,
        settings: &RemoteSttSettings,
        audio_samples: &[f32],
        prompt: Option<String>,
        language: Option<String>,
        translate_to_english: bool,
        api_key: &str,
    ) -> Result<String> {
        let started = Instant::now();
        let pcm_bytes = resample_16khz_f32_to_24khz_pcm16(audio_samples);
        let instructions = build_openai_realtime_agent_transcription_prompt(
            prompt,
            language,
            translate_to_english,
        );

        if settings.debug_mode == RemoteSttDebugMode::Verbose {
            self.record_info(
                settings,
                format!(
                    "OpenAI Realtime STT request model={} pcm_bytes={}",
                    settings.model_id,
                    pcm_bytes.len()
                ),
            );
        }

        let mut request = OPENAI_REALTIME_WS_URL
            .into_client_request()
            .map_err(|e| anyhow!("Failed to create OpenAI Realtime request: {}", e))?;
        request.headers_mut().insert(
            "Authorization",
            format!("Bearer {}", api_key.trim())
                .parse()
                .map_err(|e| anyhow!("Invalid OpenAI auth header: {}", e))?,
        );
        request.headers_mut().insert(
            "OpenAI-Safety-Identifier",
            "aivorelay-remote-stt"
                .parse()
                .map_err(|e| anyhow!("Invalid OpenAI safety identifier header: {}", e))?,
        );

        let (stream, _) = timeout(
            Duration::from_secs(DEFAULT_CONNECT_TIMEOUT_SECS),
            connect_async(request),
        )
        .await
        .map_err(|_| anyhow!("Timed out while connecting to OpenAI Realtime WebSocket"))?
        .map_err(|e| anyhow!("Failed to connect to OpenAI Realtime WebSocket: {}", e))?;
        let (mut write, mut read) = stream.split();

        let session_update = serde_json::json!({
            "type": "session.update",
            "session": {
                "type": "realtime",
                "model": OPENAI_REALTIME_MODEL,
                "output_modalities": ["text"],
                "instructions": instructions,
                "audio": {
                    "input": {
                        "format": {
                            "type": "audio/pcm",
                            "rate": 24000
                        },
                        "turn_detection": null
                    }
                }
            }
        });
        write
            .send(Message::Text(session_update.to_string().into()))
            .await
            .map_err(|e| anyhow!("Failed to send OpenAI Realtime session update: {}", e))?;
        self.wait_for_openai_realtime_event(
            settings,
            &mut read,
            "session.updated",
            "session update",
            started,
        )
        .await?;

        for chunk in pcm_bytes.chunks(OPENAI_REALTIME_AUDIO_CHUNK_BYTES) {
            let append = serde_json::json!({
                "type": "input_audio_buffer.append",
                "audio": BASE64_STANDARD.encode(chunk)
            });
            write
                .send(Message::Text(append.to_string().into()))
                .await
                .map_err(|e| anyhow!("Failed to send OpenAI Realtime audio chunk: {}", e))?;
        }

        write
            .send(Message::Text(
                serde_json::json!({ "type": "input_audio_buffer.commit" })
                    .to_string()
                    .into(),
            ))
            .await
            .map_err(|e| anyhow!("Failed to commit OpenAI Realtime audio buffer: {}", e))?;
        self.wait_for_openai_realtime_event(
            settings,
            &mut read,
            "input_audio_buffer.committed",
            "audio commit",
            started,
        )
        .await?;

        let response_create = serde_json::json!({
            "type": "response.create",
            "response": {
                "output_modalities": ["text"],
                "instructions": instructions
            }
        });
        write
            .send(Message::Text(response_create.to_string().into()))
            .await
            .map_err(|e| anyhow!("Failed to create OpenAI Realtime response: {}", e))?;
        write
            .flush()
            .await
            .map_err(|e| anyhow!("Failed to flush OpenAI Realtime stream: {}", e))?;

        let mut deltas = String::new();
        let mut final_text: Option<String> = None;

        loop {
            let frame = timeout(
                Duration::from_secs(DEFAULT_REQUEST_TIMEOUT_SECS),
                read.next(),
            )
            .await
            .map_err(|_| anyhow!("OpenAI Realtime WebSocket read timed out"))?;
            let Some(frame) = frame else {
                break;
            };
            let frame =
                frame.map_err(|e| anyhow!("OpenAI Realtime WebSocket read failed: {}", e))?;

            let Message::Text(text) = frame else {
                continue;
            };
            let payload: Value = serde_json::from_str(text.as_ref()).map_err(|e| {
                let preview: String = text.chars().take(200).collect();
                anyhow!(
                    "Invalid OpenAI Realtime WebSocket payload: {} (body: {})",
                    e,
                    preview
                )
            })?;
            let msg_type = payload
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or_default();

            if msg_type == "error" {
                let message = payload
                    .get("error")
                    .and_then(|error| error.get("message"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("OpenAI Realtime returned an error");
                self.record_error(settings, message.to_string());
                return Err(anyhow!("{}", message));
            }

            if msg_type == "response.output_text.delta" {
                if let Some(delta) = payload.get("delta").and_then(|v| v.as_str()) {
                    deltas.push_str(delta);
                }
            } else if msg_type == "response.output_text.done" {
                if let Some(text) = payload.get("text").and_then(|v| v.as_str()) {
                    final_text = Some(text.to_string());
                }
            } else if msg_type == "response.done" {
                break;
            }
        }

        let text = final_text.unwrap_or(deltas).trim().to_string();
        if settings.debug_mode == RemoteSttDebugMode::Verbose {
            self.record_info(
                settings,
                format!("OpenAI Realtime STT success output_len={}", text.len()),
            );
        }
        Ok(text)
    }

    async fn transcribe_openai_realtime_translate(
        &self,
        settings: &RemoteSttSettings,
        audio_samples: &[f32],
        prompt: Option<String>,
        language: Option<String>,
        translate_to_english: bool,
        api_key: &str,
    ) -> Result<String> {
        let target_language = if translate_to_english {
            "en".to_string()
        } else {
            resolve_explicit_realtime_language(language).ok_or_else(|| {
                anyhow!(
                    "{} requires an output target language or a detectable OS input language. Input speech can be multilingual, but same-language STT still needs AivoRelay to choose the output language. Auto is resolved from the current OS input language for this model; select the spoken language manually if OS input detection fails.",
                    OPENAI_REALTIME_TRANSLATE_MODEL
                )
            })?
        };
        let pcm_bytes = resample_16khz_f32_to_24khz_pcm16(audio_samples);

        if settings.debug_mode == RemoteSttDebugMode::Verbose {
            self.record_info(
                settings,
                format!(
                    "OpenAI Realtime Translate request model={} target_language={} pcm_bytes={}",
                    settings.model_id,
                    target_language,
                    pcm_bytes.len()
                ),
            );
        }

        let mut request = OPENAI_REALTIME_TRANSLATE_WS_URL
            .into_client_request()
            .map_err(|e| anyhow!("Failed to create OpenAI Realtime Translate request: {}", e))?;
        request.headers_mut().insert(
            "Authorization",
            format!("Bearer {}", api_key.trim())
                .parse()
                .map_err(|e| anyhow!("Invalid OpenAI auth header: {}", e))?,
        );
        request.headers_mut().insert(
            "OpenAI-Safety-Identifier",
            "aivorelay-remote-stt"
                .parse()
                .map_err(|e| anyhow!("Invalid OpenAI safety identifier header: {}", e))?,
        );

        let (stream, _) = timeout(
            Duration::from_secs(DEFAULT_CONNECT_TIMEOUT_SECS),
            connect_async(request),
        )
        .await
        .map_err(|_| anyhow!("Timed out while connecting to OpenAI Realtime Translate WebSocket"))?
        .map_err(|e| {
            anyhow!(
                "Failed to connect to OpenAI Realtime Translate WebSocket: {}",
                e
            )
        })?;
        let (mut write, mut read) = stream.split();

        let session_update = serde_json::json!({
            "type": "session.update",
            "session": {
                "audio": {
                    "output": {
                        "language": target_language
                    }
                }
            }
        });
        write
            .send(Message::Text(session_update.to_string().into()))
            .await
            .map_err(|e| {
                anyhow!(
                    "Failed to send OpenAI Realtime Translate session update: {}",
                    e
                )
            })?;

        let prompt_text = prompt
            .as_deref()
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .unwrap_or("");
        if !prompt_text.is_empty() && settings.debug_mode == RemoteSttDebugMode::Verbose {
            self.record_info(
                settings,
                "OpenAI Realtime Translate does not expose prompt instructions; using language-only session configuration.".to_string(),
            );
        }

        for chunk in pcm_bytes.chunks(OPENAI_REALTIME_AUDIO_CHUNK_BYTES) {
            let append = serde_json::json!({
                "type": "session.input_audio_buffer.append",
                "audio": BASE64_STANDARD.encode(chunk)
            });
            write
                .send(Message::Text(append.to_string().into()))
                .await
                .map_err(|e| {
                    anyhow!(
                        "Failed to send OpenAI Realtime Translate audio chunk: {}",
                        e
                    )
                })?;
        }

        let silence = vec![0_u8; OPENAI_REALTIME_AUDIO_CHUNK_BYTES * 2];
        write
            .send(Message::Text(
                serde_json::json!({
                    "type": "session.input_audio_buffer.append",
                    "audio": BASE64_STANDARD.encode(silence)
                })
                .to_string()
                .into(),
            ))
            .await
            .map_err(|e| {
                anyhow!(
                    "Failed to send OpenAI Realtime Translate trailing silence: {}",
                    e
                )
            })?;
        write
            .flush()
            .await
            .map_err(|e| anyhow!("Failed to flush OpenAI Realtime Translate stream: {}", e))?;

        let mut output_text = String::new();
        let mut input_text = String::new();
        let mut saw_transcript = false;
        let idle_timeout = Duration::from_secs(3);
        let max_wait = Instant::now() + Duration::from_secs(DEFAULT_REQUEST_TIMEOUT_SECS);

        loop {
            let now = Instant::now();
            if now >= max_wait {
                break;
            }
            let wait = if saw_transcript {
                idle_timeout.min(max_wait.saturating_duration_since(now))
            } else {
                Duration::from_secs(DEFAULT_REQUEST_TIMEOUT_SECS)
                    .min(max_wait.saturating_duration_since(now))
            };
            let frame = match timeout(wait, read.next()).await {
                Ok(Some(frame)) => frame,
                Ok(None) => break,
                Err(_) if saw_transcript => break,
                Err(_) => {
                    return Err(anyhow!(
                        "OpenAI Realtime Translate WebSocket read timed out"
                    ))
                }
            };
            let frame = frame
                .map_err(|e| anyhow!("OpenAI Realtime Translate WebSocket read failed: {}", e))?;

            let Message::Text(text) = frame else {
                continue;
            };
            let payload: Value = serde_json::from_str(text.as_ref()).map_err(|e| {
                let preview: String = text.chars().take(200).collect();
                anyhow!(
                    "Invalid OpenAI Realtime Translate WebSocket payload: {} (body: {})",
                    e,
                    preview
                )
            })?;
            let msg_type = payload
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or_default();

            if msg_type == "error" {
                let message = payload
                    .get("error")
                    .and_then(|error| error.get("message"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("OpenAI Realtime Translate returned an error");
                self.record_error(settings, message.to_string());
                return Err(anyhow!("{}", message));
            }

            if msg_type == "session.output_transcript.delta" {
                if let Some(delta) = payload.get("delta").and_then(|v| v.as_str()) {
                    output_text.push_str(delta);
                    saw_transcript = true;
                }
            } else if msg_type == "session.input_transcript.delta" {
                if let Some(delta) = payload.get("delta").and_then(|v| v.as_str()) {
                    input_text.push_str(delta);
                    saw_transcript = true;
                }
            }
        }

        let text = if output_text.trim().is_empty() {
            input_text
        } else {
            output_text
        }
        .trim()
        .to_string();

        if settings.debug_mode == RemoteSttDebugMode::Verbose {
            self.record_info(
                settings,
                format!(
                    "OpenAI Realtime Translate success output_len={}",
                    text.len()
                ),
            );
        }
        Ok(text)
    }

    async fn wait_for_openai_realtime_event<R>(
        &self,
        settings: &RemoteSttSettings,
        read: &mut R,
        expected_type: &str,
        action: &str,
        _started: Instant,
    ) -> Result<()>
    where
        R: Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
    {
        loop {
            let frame = timeout(
                Duration::from_secs(DEFAULT_REQUEST_TIMEOUT_SECS),
                read.next(),
            )
            .await
            .map_err(|_| anyhow!("OpenAI Realtime {} timed out", action))?;
            let Some(frame) = frame else {
                return Err(anyhow!(
                    "OpenAI Realtime WebSocket closed during {}",
                    action
                ));
            };
            let frame =
                frame.map_err(|e| anyhow!("OpenAI Realtime WebSocket read failed: {}", e))?;
            let Message::Text(text) = frame else {
                continue;
            };
            let payload: Value = serde_json::from_str(text.as_ref()).map_err(|e| {
                let preview: String = text.chars().take(200).collect();
                anyhow!(
                    "Invalid OpenAI Realtime WebSocket payload: {} (body: {})",
                    e,
                    preview
                )
            })?;
            let msg_type = payload
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            if msg_type == expected_type {
                return Ok(());
            }
            if msg_type == "error" {
                let message = payload
                    .get("error")
                    .and_then(|error| error.get("message"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("OpenAI Realtime returned an error");
                self.record_error(settings, message.to_string());
                return Err(anyhow!("{}", message));
            }
        }
    }

    pub async fn test_connection(
        &self,
        settings: &RemoteSttSettings,
        base_url: &str,
    ) -> Result<()> {
        let override_base_url = (!base_url.trim().is_empty()).then_some(base_url.trim());
        let base_url =
            validate_remote_stt_base_url(settings, override_base_url).map_err(|message| {
                self.record_error(settings, message.clone());
                anyhow!(message)
            })?;

        let api_key = get_remote_stt_api_key_for_request(settings).map_err(|e| {
            let message = format!("Remote STT API key unavailable: {}", e);
            self.record_error(settings, message.clone());
            anyhow!(message)
        })?;

        let url = format!("{}/models", base_url);

        if settings.debug_mode == RemoteSttDebugMode::Verbose {
            self.record_info(
                settings,
                format!("Remote STT test request base_url={}", base_url),
            );
        }

        let start = Instant::now();
        let response = self
            .client
            .get(url)
            .bearer_auth(&api_key.value)
            .send()
            .await
            .map_err(|e| {
                let message = format!("Remote STT test request failed: {}", e);
                self.record_error(settings, message.clone());
                anyhow!(message)
            })?;

        let status = response.status();
        let elapsed_ms = start.elapsed().as_millis();

        if settings.debug_mode == RemoteSttDebugMode::Verbose {
            self.record_info(
                settings,
                format!(
                    "Remote STT test response status={} elapsed_ms={}",
                    status, elapsed_ms
                ),
            );
        }

        if !status.is_success() {
            let body = response.bytes().await.unwrap_or_default();
            let snippet = String::from_utf8_lossy(&body);
            let snippet = snippet.chars().take(500).collect::<String>();
            let message = format!(
                "Remote STT test failed: status={} elapsed_ms={} body_snippet={}",
                status, elapsed_ms, snippet
            );
            self.record_error(settings, message.clone());
            return Err(anyhow!(message));
        }

        self.migrate_legacy_api_key_after_success(settings, &api_key, Ok(()))?;
        Ok(())
    }
}

fn remote_stt_api_key_scope(settings: &RemoteSttSettings) -> &'static str {
    match settings.provider_preset.as_str() {
        REMOTE_STT_PRESET_GROQ => REMOTE_STT_PRESET_GROQ,
        REMOTE_STT_PRESET_OPENAI => REMOTE_STT_PRESET_OPENAI,
        REMOTE_STT_PRESET_CUSTOM => REMOTE_STT_PRESET_CUSTOM,
        _ => infer_remote_stt_preset(&settings.base_url),
    }
}

fn remote_stt_api_key_user(settings: &RemoteSttSettings) -> String {
    format!(
        "{}_{}",
        REMOTE_STT_USER_PREFIX,
        remote_stt_api_key_scope(settings)
    )
}

fn legacy_remote_stt_api_key_user() -> &'static str {
    REMOTE_STT_USER_PREFIX
}

fn non_empty_remote_stt_api_key(
    key: Option<String>,
    source: RemoteSttApiKeySource,
) -> Option<RemoteSttApiKey> {
    let key = key?;
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(RemoteSttApiKey {
        value: trimmed.to_string(),
        source,
    })
}

fn select_remote_stt_api_key(
    scoped_key: Option<String>,
    legacy_key: Option<String>,
) -> Option<RemoteSttApiKey> {
    non_empty_remote_stt_api_key(scoped_key, RemoteSttApiKeySource::Scoped)
        .or_else(|| non_empty_remote_stt_api_key(legacy_key, RemoteSttApiKeySource::Legacy))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RemoteSttApiKeyClearTargets {
    scoped: bool,
    legacy: bool,
}

fn remote_stt_api_key_clear_targets(
    scoped_key: Option<&str>,
    legacy_key: Option<&str>,
) -> RemoteSttApiKeyClearTargets {
    RemoteSttApiKeyClearTargets {
        scoped: scoped_key
            .map(|key| !key.trim().is_empty())
            .unwrap_or(false),
        legacy: legacy_key
            .map(|key| !key.trim().is_empty())
            .unwrap_or(false),
    }
}

#[cfg(target_os = "windows")]
pub fn set_remote_stt_api_key(settings: &RemoteSttSettings, key: &str) -> Result<()> {
    let user = remote_stt_api_key_user(settings);
    let entry = keyring::Entry::new(REMOTE_STT_SERVICE, &user)?;
    entry
        .set_password(key)
        .map_err(|e| anyhow!("Failed to store API key: {}", e))
}

#[cfg(target_os = "windows")]
fn read_remote_stt_api_key_user(user: &str) -> Result<String> {
    let entry = keyring::Entry::new(REMOTE_STT_SERVICE, user)?;
    match entry.get_password() {
        Ok(key) => Ok(key),
        Err(keyring::Error::NoEntry) => Ok(String::new()),
        Err(e) => Err(anyhow!("Failed to read API key: {}", e)),
    }
}

#[cfg(target_os = "windows")]
fn delete_remote_stt_api_key_user(user: &str) -> Result<()> {
    let entry = keyring::Entry::new(REMOTE_STT_SERVICE, user)?;
    match entry.delete_password() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(anyhow!("Failed to delete API key: {}", e)),
    }
}

#[cfg(target_os = "windows")]
fn get_remote_stt_api_key_for_request(settings: &RemoteSttSettings) -> Result<RemoteSttApiKey> {
    let scoped_user = remote_stt_api_key_user(settings);
    let scoped_key = read_remote_stt_api_key_user(&scoped_user)?;
    let legacy_key = if scoped_key.trim().is_empty() {
        Some(read_remote_stt_api_key_user(
            legacy_remote_stt_api_key_user(),
        )?)
    } else {
        None
    };

    select_remote_stt_api_key(Some(scoped_key), legacy_key)
        .ok_or_else(|| anyhow!("No Remote STT API key is stored"))
}

#[cfg(target_os = "windows")]
fn migrate_remote_stt_legacy_api_key_after_success(
    settings: &RemoteSttSettings,
    api_key: &RemoteSttApiKey,
) -> Result<()> {
    if api_key.source != RemoteSttApiKeySource::Legacy {
        return Ok(());
    }

    set_remote_stt_api_key(settings, &api_key.value)?;
    delete_remote_stt_api_key_user(legacy_remote_stt_api_key_user())
}

#[cfg(target_os = "windows")]
pub fn get_remote_stt_api_key(settings: &RemoteSttSettings) -> Result<String> {
    get_remote_stt_api_key_for_request(settings).map(|api_key| api_key.value)
}

#[cfg(target_os = "windows")]
pub fn clear_remote_stt_api_key(settings: &RemoteSttSettings) -> Result<()> {
    let scoped_user = remote_stt_api_key_user(settings);
    let legacy_user = legacy_remote_stt_api_key_user();
    let scoped_key = read_remote_stt_api_key_user(&scoped_user)?;
    let legacy_key = read_remote_stt_api_key_user(legacy_user)?;
    let clear_targets = remote_stt_api_key_clear_targets(Some(&scoped_key), Some(&legacy_key));

    if clear_targets.scoped {
        delete_remote_stt_api_key_user(&scoped_user)?;
    }
    if clear_targets.legacy {
        delete_remote_stt_api_key_user(legacy_user)?;
    }

    Ok(())
}

#[cfg(target_os = "windows")]
pub fn has_remote_stt_api_key(settings: &RemoteSttSettings) -> bool {
    get_remote_stt_api_key(settings)
        .map(|key| !key.trim().is_empty())
        .unwrap_or(false)
}

#[cfg(not(target_os = "windows"))]
pub fn set_remote_stt_api_key(_settings: &RemoteSttSettings, _key: &str) -> Result<()> {
    Err(anyhow!("Remote STT is only available on Windows"))
}

#[cfg(not(target_os = "windows"))]
pub fn get_remote_stt_api_key(_settings: &RemoteSttSettings) -> Result<String> {
    Err(anyhow!("Remote STT is only available on Windows"))
}

#[cfg(not(target_os = "windows"))]
fn get_remote_stt_api_key_for_request(_settings: &RemoteSttSettings) -> Result<RemoteSttApiKey> {
    Err(anyhow!("Remote STT is only available on Windows"))
}

#[cfg(not(target_os = "windows"))]
fn migrate_remote_stt_legacy_api_key_after_success(
    _settings: &RemoteSttSettings,
    _api_key: &RemoteSttApiKey,
) -> Result<()> {
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn clear_remote_stt_api_key(_settings: &RemoteSttSettings) -> Result<()> {
    Err(anyhow!("Remote STT is only available on Windows"))
}

#[cfg(not(target_os = "windows"))]
pub fn has_remote_stt_api_key(_settings: &RemoteSttSettings) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::{
        remote_stt_api_key_clear_targets, select_remote_stt_api_key, supports_translation,
        RemoteSttApiKeySource,
    };

    #[test]
    fn gpt_realtime_2_supports_remote_stt_translation() {
        assert!(supports_translation("gpt-realtime-2"));
    }

    #[test]
    fn gpt_realtime_translate_supports_remote_stt_translation() {
        assert!(supports_translation("gpt-realtime-translate"));
    }

    #[test]
    fn whisper_turbo_still_does_not_support_remote_stt_translation() {
        assert!(!supports_translation("whisper-large-v3-turbo"));
    }

    #[test]
    fn remote_stt_api_key_prefers_scoped_key() {
        let key = select_remote_stt_api_key(
            Some("scoped-key".to_string()),
            Some("legacy-key".to_string()),
        )
        .unwrap();

        assert_eq!(key.value, "scoped-key");
        assert_eq!(key.source, RemoteSttApiKeySource::Scoped);
    }

    #[test]
    fn remote_stt_api_key_falls_back_to_legacy_when_scoped_missing() {
        let key =
            select_remote_stt_api_key(Some("  ".to_string()), Some(" legacy-key ".to_string()))
                .unwrap();

        assert_eq!(key.value, "legacy-key");
        assert_eq!(key.source, RemoteSttApiKeySource::Legacy);
    }

    #[test]
    fn remote_stt_api_key_treats_blank_keys_as_absent() {
        assert!(
            select_remote_stt_api_key(Some(" \t ".to_string()), Some("\n".to_string())).is_none()
        );
    }

    #[test]
    fn remote_stt_clear_targets_include_legacy_fallback() {
        let targets = remote_stt_api_key_clear_targets(None, Some("legacy-key"));

        assert!(!targets.scoped);
        assert!(targets.legacy);
    }

    #[test]
    fn remote_stt_clear_targets_remove_legacy_that_would_reappear_after_scoped_clear() {
        let targets = remote_stt_api_key_clear_targets(Some("scoped-key"), Some("legacy-key"));

        assert!(targets.scoped);
        assert!(targets.legacy);
    }
}
