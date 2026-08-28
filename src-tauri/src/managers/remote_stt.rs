use crate::audio_toolkit::encode_wav_bytes;
use crate::managers::openai_realtime_whisper::{
    OpenAiRealtimeWhisperManager, OpenAiRealtimeWhisperOptions, OPENAI_TRANSCRIBE_MODEL,
};
use crate::managers::provider_error::{parse_provider_error, parse_provider_error_value};
use crate::settings::{RemoteSttDebugMode, RemoteSttSettings};
use crate::subtitle::{
    timed_tokens_to_subtitle_segments, SubtitleSegment, TimedTranscriptToken,
};
use crate::url_security::{
    infer_remote_stt_preset, validate_remote_stt_base_url, REMOTE_STT_OPENAI_BASE_URL,
    REMOTE_STT_GOOGLE_DEFAULT_MODEL, REMOTE_STT_PRESET_CUSTOM, REMOTE_STT_PRESET_GOOGLE,
    REMOTE_STT_PRESET_GROQ, REMOTE_STT_PRESET_OPENAI, REMOTE_STT_PRESET_VERCEL,
    REMOTE_STT_VERCEL_DEFAULT_MODEL,
};
use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use futures_util::{SinkExt, Stream, StreamExt};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tokio_util::sync::CancellationToken;

/// Default timeout for Remote STT requests (60 seconds)
const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 60;
/// Default connection timeout (10 seconds)
const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 10;

const REMOTE_STT_SERVICE: &str = "fi.maxits.aivorelay";
const REMOTE_STT_USER_PREFIX: &str = "remote_stt_api_key";
const OPENAI_REALTIME_MODEL: &str = "gpt-realtime-2.1";
const OPENAI_REALTIME_LEGACY_MODEL: &str = "gpt-realtime-2";
const OPENAI_REALTIME_TRANSLATE_MODEL: &str = "gpt-realtime-translate";
const OPENAI_REALTIME_WS_URL: &str = "wss://api.openai.com/v1/realtime?model=gpt-realtime-2.1";
const OPENAI_REALTIME_LEGACY_WS_URL: &str = "wss://api.openai.com/v1/realtime?model=gpt-realtime-2";
const OPENAI_REALTIME_TRANSLATE_WS_URL: &str =
    "wss://api.openai.com/v1/realtime/translations?model=gpt-realtime-translate";
const VERCEL_GATEWAY_PROTOCOL_VERSION: &str = "0.0.1";
const VERCEL_TRANSCRIPTION_SPECIFICATION_VERSION: &str = "4";
const GEMINI_MAX_AUDIO_SAMPLES: usize = 60 * 60 * 16_000;
const GEMINI_WORD_TIMESTAMP_MAX_AUDIO_SAMPLES: usize = 30 * 60 * 16_000;
const OPENAI_REALTIME_AUDIO_CHUNK_BYTES: usize = 48_000;
const OPENAI_REALTIME_AGENT_DEFAULT_PROMPT: &str =
    "Additional context for speech-to-text transcription. \
     Current language setting: ${language}. Translate to English: ${translate_to_english}. \
     Preserve the speaker's language unless translation is enabled. \
     Use context to create proper punctuation and fix recognition errors only when the intended words are recoverable from audio and context. \
     If speech is not recoverable because of microphone noise, speech defects, or background noise, use [⚠️inaudible⚠️] instead of guessing. \
     The user may provide custom words that are rare in the language; try to recognize them properly. \
     Make sure to properly recognize names, product names, and vocabulary exactly when recognizable.";

fn remote_stt_api_key_redaction_marker(api_key: &str) -> String {
    let digest = Sha256::digest(api_key.as_bytes());
    let fingerprint = digest[..8]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("[redacted key, SHA-256: {fingerprint}]")
}

fn redact_remote_stt_api_key(value: &str, api_key: &str) -> String {
    let api_key = api_key.trim();
    if api_key.is_empty() {
        value.to_string()
    } else {
        value.replace(api_key, &remote_stt_api_key_redaction_marker(api_key))
    }
}

fn redacted_remote_stt_snippet(value: &str, api_key: &str, max_chars: usize) -> String {
    redact_remote_stt_api_key(value, api_key)
        .chars()
        .take(max_chars)
        .collect()
}

fn remote_stt_log_value(value: &str, api_key: &str, unsafe_log_secrets: bool) -> String {
    if unsafe_log_secrets {
        value.to_string()
    } else {
        redact_remote_stt_api_key(value, api_key)
    }
}

fn remote_stt_log_snippet(
    value: &str,
    api_key: &str,
    max_chars: usize,
    unsafe_log_secrets: bool,
) -> String {
    remote_stt_log_value(value, api_key, unsafe_log_secrets)
        .chars()
        .take(max_chars)
        .collect()
}

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
    #[serde(default)]
    segments: Vec<SubtitleSegment>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VercelTranscriptionSegment {
    text: String,
    start_second: f32,
    end_second: f32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VercelTranscriptionResponse {
    text: String,
    #[serde(default)]
    segments: Vec<VercelTranscriptionSegment>,
}

#[derive(Debug, Deserialize)]
struct GoogleWordAnnotation {
    #[serde(rename = "type")]
    annotation_type: Option<String>,
    text: Option<String>,
    start_offset: Option<String>,
    end_offset: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GoogleInteractionContent {
    #[serde(rename = "type")]
    content_type: Option<String>,
    text: Option<String>,
    annotations: Option<Vec<GoogleWordAnnotation>>,
}

#[derive(Debug, Deserialize)]
struct GoogleInteractionStep {
    content: Option<Vec<GoogleInteractionContent>>,
}

#[derive(Debug, Deserialize)]
struct GoogleInteractionsTranscriptionResponse {
    steps: Option<Vec<GoogleInteractionStep>>,
}

#[derive(Debug, Clone, Default)]
pub struct RemoteFileTranscription {
    pub text: String,
    pub segments: Vec<SubtitleSegment>,
}

/// Returns whether a remote STT model supports translation to English.
/// Uses the OpenAI-compatible /audio/translations endpoint.
///
/// Known model support:
/// - Groq: whisper-large-v3 supports translation, whisper-large-v3-turbo does NOT
/// - OpenAI: whisper-1, gpt-realtime-2.1, gpt-realtime-2, and gpt-realtime-translate support translation
/// - Unknown models default to false (safe fallback)
pub fn supports_translation(model_id: &str) -> bool {
    let lower = model_id.to_lowercase();

    if lower == "gpt-transcribe"
        || lower == "gpt-live-transcribe"
        || lower == "gpt-realtime-whisper"
    {
        return false;
    }

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
    if lower == "whisper-1"
        || lower == "gpt-realtime-2.1"
        || lower == "gpt-realtime-2"
        || lower == "gpt-realtime-translate"
    {
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
    let model_id = model_id.trim();
    model_id.eq_ignore_ascii_case(OPENAI_REALTIME_MODEL)
        || model_id.eq_ignore_ascii_case(OPENAI_REALTIME_LEGACY_MODEL)
}

fn openai_realtime_ws_url(model_id: &str) -> &'static str {
    if model_id
        .trim()
        .eq_ignore_ascii_case(OPENAI_REALTIME_LEGACY_MODEL)
    {
        OPENAI_REALTIME_LEGACY_WS_URL
    } else {
        OPENAI_REALTIME_WS_URL
    }
}

fn is_openai_realtime_translate_model(model_id: &str) -> bool {
    model_id
        .trim()
        .eq_ignore_ascii_case(OPENAI_REALTIME_TRANSLATE_MODEL)
}

fn uses_plural_language_hints(model_id: &str) -> bool {
    model_id
        .trim()
        .eq_ignore_ascii_case(OPENAI_TRANSCRIBE_MODEL)
}

pub fn supports_subtitle_timestamps(model_id: &str) -> bool {
    let model_id = model_id.trim().to_ascii_lowercase();
    model_id == REMOTE_STT_VERCEL_DEFAULT_MODEL
        || model_id == REMOTE_STT_GOOGLE_DEFAULT_MODEL
        || (model_id.contains("whisper")
            && !OpenAiRealtimeWhisperManager::is_realtime_model(&model_id)
            && !is_openai_realtime_model(&model_id)
            && !is_openai_realtime_translate_model(&model_id))
}

fn normalize_gemini_language_code(language: &str) -> Option<&'static str> {
    let normalized = language.trim().replace('_', "-").to_ascii_lowercase();
    Some(match normalized.as_str() {
        "af" | "af-za" => "af-ZA",
        "am" | "am-et" => "am-ET",
        "ar" | "ar-eg" => "ar-EG",
        "as" | "as-in" => "as-IN",
        "az" | "az-az" => "az-AZ",
        "be" | "be-by" => "be-BY",
        "bn-in" => "bn-IN",
        "bn-bd" => "bn-BD",
        "bs" | "bs-ba" => "bs-BA",
        "bg" | "bg-bg" => "bg-BG",
        "rup" | "rup-bg" => "rup-BG",
        "my" | "my-mm" => "my-MM",
        "ca" | "ca-es" => "ca-ES",
        "ceb" => "ceb",
        "km" | "km-kh" => "km-KH",
        "hr" | "hr-hr" => "hr-HR",
        "cs" | "cs-cz" => "cs-CZ",
        "da" | "da-dk" => "da-DK",
        "nl" | "nl-nl" => "nl-NL",
        "en-us" => "en-US",
        "en-gb" => "en-GB",
        "en-in" => "en-IN",
        "et" | "et-ee" => "et-EE",
        "fa" | "fa-ir" => "fa-IR",
        "fil" | "tl" | "fil-ph" => "fil-PH",
        "fi" | "fi-fi" => "fi-FI",
        "fr" | "fr-fr" => "fr-FR",
        "gl" | "gl-es" => "gl-ES",
        "ka" | "ka-ge" => "ka-GE",
        "de" | "de-de" => "de-DE",
        "el" | "el-gr" => "el-GR",
        "gu" | "gu-in" => "gu-IN",
        "ha" | "ha-ng" => "ha-NG",
        "he" | "he-il" => "he-IL",
        "hi" | "hi-in" => "hi-IN",
        "hu" | "hu-hu" => "hu-HU",
        "hy" | "hy-am" => "hy-AM",
        "is" | "is-is" => "is-IS",
        "id" | "id-id" => "id-ID",
        "it" | "it-it" => "it-IT",
        "ja" | "ja-jp" => "ja-JP",
        "jv" | "jw" | "jv-id" => "jv-ID",
        "kea" | "kea-cv" => "kea-CV",
        "kn" | "kn-in" => "kn-IN",
        "kk" | "kk-kz" => "kk-KZ",
        "ko" | "ko-kr" => "ko-KR",
        "ky" | "ky-kg" => "ky-KG",
        "lv" | "lv-lv" => "lv-LV",
        "ln" | "ln-cd" => "ln-CD",
        "lt" | "lt-lt" => "lt-LT",
        "mk" | "mk-mk" => "mk-MK",
        "ms" | "ms-my" => "ms-MY",
        "ml" | "ml-in" => "ml-IN",
        "mt" | "mt-mt" => "mt-MT",
        "cmn" | "zh-hans" | "cmn-hans-cn" => "cmn-Hans-CN",
        "mr" | "mr-in" => "mr-IN",
        "mn" | "mn-mn" => "mn-MN",
        "ne" | "ne-np" => "ne-NP",
        "nb" | "no" | "nb-no" => "nb-NO",
        "or" | "or-in" => "or-IN",
        "pa-in" => "pa-IN",
        "pa-guru-in" => "pa-Guru-IN",
        "pl" | "pl-pl" => "pl-PL",
        "pt-br" => "pt-BR",
        "pt-pt" => "pt-PT",
        "ro" | "ro-ro" => "ro-RO",
        "ru" | "ru-ru" => "ru-RU",
        "sr" | "sr-rs" => "sr-RS",
        "sd" | "sd-arab-in" => "sd-Arab-IN",
        "sk" | "sk-sk" => "sk-SK",
        "sl" | "sl-si" => "sl-SI",
        "es-419" => "es-419",
        "es-us" => "es-US",
        "sw" | "sw-ke" => "sw-KE",
        "sv" | "sv-se" => "sv-SE",
        "tg" | "tg-tj" => "tg-TJ",
        "te" | "te-in" => "te-IN",
        "th" | "th-th" => "th-TH",
        "tr" | "tr-tr" => "tr-TR",
        "uk" | "uk-ua" => "uk-UA",
        "uz" | "uz-uz" => "uz-UZ",
        "vi" | "vi-vn" => "vi-VN",
        "yue" | "yue-hant-hk" => "yue-Hant-HK",
        _ => return None,
    })
}

fn resolve_gemini_language(language: Option<String>) -> Option<String> {
    let mut language = language?;
    if language.eq_ignore_ascii_case("os_input") {
        language = crate::input_source::get_language_from_input_source()?;
    }
    if language.trim().is_empty() || language.eq_ignore_ascii_case("auto") {
        return None;
    }
    normalize_gemini_language_code(&language).map(str::to_string)
}

fn build_vercel_gemini_request_body(
    audio_base64: String,
    language: Option<&str>,
    request_word_timestamps: bool,
) -> Value {
    let mut google_options = serde_json::Map::new();
    if let Some(language) = language {
        google_options.insert("languageCodes".to_string(), serde_json::json!([language]));
    }
    if request_word_timestamps {
        google_options.insert("wordTimestamp".to_string(), Value::Bool(true));
    }

    let mut body = serde_json::json!({
        "audio": audio_base64,
        "mediaType": "audio/wav"
    });
    if !google_options.is_empty() {
        body["providerOptions"] = serde_json::json!({
            "google": Value::Object(google_options)
        });
    }
    body
}

fn build_google_gemini_request_body(
    audio_base64: String,
    language: Option<&str>,
    request_word_timestamps: bool,
) -> Value {
    let mut transcription_config = serde_json::Map::new();
    if let Some(language) = language {
        transcription_config.insert("language_codes".to_string(), serde_json::json!([language]));
    }
    if request_word_timestamps {
        transcription_config.insert(
            "mode".to_string(),
            serde_json::json!({
                "type": "verbatim",
                "timestamp_granularities": ["word"]
            }),
        );
    }

    let mut body = serde_json::json!({
        "model": REMOTE_STT_GOOGLE_DEFAULT_MODEL,
        "input": [{
            "type": "audio",
            "data": audio_base64,
            "mime_type": "audio/wav"
        }]
    });
    if !transcription_config.is_empty() {
        body["generation_config"] = serde_json::json!({
            "transcription_config": Value::Object(transcription_config)
        });
    }
    body
}

fn build_vercel_gemini_request(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    body: &Value,
) -> reqwest::RequestBuilder {
    client
        .post(format!("{}/transcription-model", base_url))
        .bearer_auth(api_key)
        .header(
            "ai-gateway-protocol-version",
            VERCEL_GATEWAY_PROTOCOL_VERSION,
        )
        .header("ai-gateway-auth-method", "api-key")
        .header(
            "ai-transcription-model-specification-version",
            VERCEL_TRANSCRIPTION_SPECIFICATION_VERSION,
        )
        .header("ai-model-id", REMOTE_STT_VERCEL_DEFAULT_MODEL)
        .json(body)
}

fn build_google_gemini_request(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    body: &Value,
) -> reqwest::RequestBuilder {
    client
        .post(format!("{}/interactions", base_url))
        .header("x-goog-api-key", api_key)
        .json(body)
}

fn parse_google_offset_seconds(offset: &str) -> Option<f32> {
    let seconds = offset.trim().strip_suffix('s').unwrap_or(offset.trim());
    seconds.parse::<f32>().ok().filter(|value| value.is_finite())
}

fn timed_tokens_from_vercel_segments(
    segments: Vec<VercelTranscriptionSegment>,
) -> Vec<TimedTranscriptToken> {
    segments
        .into_iter()
        .map(|segment| TimedTranscriptToken {
            start: segment.start_second,
            end: segment.end_second,
            text: segment.text,
            prepend_space: true,
        })
        .collect()
}

fn parse_google_gemini_response(
    response: GoogleInteractionsTranscriptionResponse,
) -> RemoteFileTranscription {
    let mut text = String::new();
    let mut timed_tokens = Vec::new();

    for step in response.steps.unwrap_or_default() {
        for content in step.content.unwrap_or_default() {
            if content.content_type.as_deref() != Some("text") {
                continue;
            }
            if let Some(content_text) = content.text {
                text.push_str(&content_text);
            }
            for annotation in content.annotations.unwrap_or_default() {
                if annotation.annotation_type.as_deref() != Some("word_info") {
                    continue;
                }
                let Some(annotation_text) = annotation.text else {
                    continue;
                };
                let Some(start) = annotation
                    .start_offset
                    .as_deref()
                    .and_then(parse_google_offset_seconds)
                else {
                    continue;
                };
                let Some(end) = annotation
                    .end_offset
                    .as_deref()
                    .and_then(parse_google_offset_seconds)
                else {
                    continue;
                };
                timed_tokens.push(TimedTranscriptToken {
                    start,
                    end,
                    text: annotation_text,
                    prepend_space: true,
                });
            }
        }
    }

    RemoteFileTranscription {
        text,
        segments: timed_tokens_to_subtitle_segments(&timed_tokens),
    }
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

fn build_openai_realtime_agent_session_update(model_id: &str, instructions: &str) -> Value {
    serde_json::json!({
        "type": "session.update",
        "session": {
            "type": "realtime",
            "model": model_id.trim(),
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
    })
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
    /// Cancellation tokens for requests that are currently awaiting remote I/O.
    active_requests: Mutex<HashMap<u64, CancellationToken>>,
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
            active_requests: Mutex::new(HashMap::new()),
        })
    }

    /// Returns a new operation ID for tracking cancellation.
    pub fn start_operation(&self) -> u64 {
        self.current_operation_id.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Marks all operations started before now as cancelled.
    pub fn cancel(&self) {
        let current = self.current_operation_id.load(Ordering::SeqCst);
        let cancelled_before_id = current + 1;
        self.cancelled_before_id
            .store(cancelled_before_id, Ordering::SeqCst);
        for (operation_id, token) in self.active_requests.lock().unwrap().iter() {
            if *operation_id < cancelled_before_id {
                token.cancel();
            }
        }
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

    pub async fn transcribe_with_operation(
        &self,
        operation_id: u64,
        settings: &RemoteSttSettings,
        audio_samples: &[f32],
        prompt: Option<String>,
        language: Option<String>,
        translate_to_english: bool,
    ) -> Result<String> {
        self.transcribe_with_operation_inner(
            operation_id,
            settings,
            audio_samples,
            prompt,
            language,
            translate_to_english,
            false,
        )
        .await
        .map(|result| result.text)
    }

    pub async fn transcribe_file_with_operation(
        &self,
        operation_id: u64,
        settings: &RemoteSttSettings,
        audio_samples: &[f32],
        prompt: Option<String>,
        language: Option<String>,
        translate_to_english: bool,
        request_segments: bool,
    ) -> Result<RemoteFileTranscription> {
        self.transcribe_with_operation_inner(
            operation_id,
            settings,
            audio_samples,
            prompt,
            language,
            translate_to_english,
            request_segments,
        )
        .await
    }

    async fn transcribe_with_operation_inner(
        &self,
        operation_id: u64,
        settings: &RemoteSttSettings,
        audio_samples: &[f32],
        prompt: Option<String>,
        language: Option<String>,
        translate_to_english: bool,
        request_segments: bool,
    ) -> Result<RemoteFileTranscription> {
        let cancel_token = CancellationToken::new();
        self.active_requests
            .lock()
            .unwrap()
            .insert(operation_id, cancel_token.clone());
        if self.is_cancelled(operation_id) {
            cancel_token.cancel();
        }

        let result = tokio::select! {
            biased;
            _ = cancel_token.cancelled() => Err(anyhow!("Remote STT operation was cancelled")),
            result = self.transcribe_inner(
                settings,
                audio_samples,
                prompt,
                language,
                translate_to_english,
                request_segments,
            ) => result,
        };

        self.active_requests.lock().unwrap().remove(&operation_id);
        result
    }

    async fn transcribe_inner(
        &self,
        settings: &RemoteSttSettings,
        audio_samples: &[f32],
        prompt: Option<String>,
        language: Option<String>,
        translate_to_english: bool,
        request_segments: bool,
    ) -> Result<RemoteFileTranscription> {
        if audio_samples.is_empty() {
            return Ok(RemoteFileTranscription::default());
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

        let gemini_expected_model = match settings.provider_preset.as_str() {
            REMOTE_STT_PRESET_VERCEL => Some(REMOTE_STT_VERCEL_DEFAULT_MODEL),
            REMOTE_STT_PRESET_GOOGLE => Some(REMOTE_STT_GOOGLE_DEFAULT_MODEL),
            _ => None,
        };
        if let Some(expected_model) = gemini_expected_model {
            if settings.model_id.trim() != expected_model {
                let message = format!(
                    "The {} Gemini route requires model '{}', but settings contain '{}'. Select Gemini 3.5 Transcribe again in Settings -> Models.",
                    remote_stt_api_key_provider_label(settings),
                    expected_model,
                    settings.model_id.trim()
                );
                self.record_error(settings, message.clone());
                return Err(anyhow!(message));
            }
            let max_audio_samples = if request_segments {
                GEMINI_WORD_TIMESTAMP_MAX_AUDIO_SAMPLES
            } else {
                GEMINI_MAX_AUDIO_SAMPLES
            };
            if audio_samples.len() > max_audio_samples {
                let limit_minutes = if request_segments { 30 } else { 60 };
                let output_hint = if request_segments {
                    " Select Text output or use a file no longer than 30 minutes."
                } else {
                    " Split the recording into files no longer than 60 minutes."
                };
                let message = format!(
                    "Gemini 3.5 Transcribe supports at most {limit_minutes} minutes for this output type.{output_hint}"
                );
                self.record_error(settings, message.clone());
                return Err(anyhow!(message));
            }
            if translate_to_english {
                let message = format!(
                    "{} does not support AivoRelay's Translate to English option. Disable translation before using Gemini 3.5 Transcribe.",
                    expected_model
                );
                self.record_error(settings, message.clone());
                return Err(anyhow!(message));
            }
        }

        if request_segments && !supports_subtitle_timestamps(&settings.model_id) {
            return Err(anyhow!(
                "Model '{}' does not expose segment timestamps through its OpenAI-compatible transcription endpoint. Select Text output or a timestamp-capable Whisper model.",
                settings.model_id
            ));
        }

        let api_key = get_remote_stt_api_key_for_request(settings).map_err(|e| {
            let message = e.to_string();
            self.record_error(settings, message.clone());
            anyhow!(message)
        })?;

        if uses_plural_language_hints(&settings.model_id) && translate_to_english {
            let message = format!(
                "{} does not support Translate to English. Disable translation or select whisper-1.",
                settings.model_id
            );
            self.record_error(settings, message.clone());
            return Err(anyhow!(message));
        }

        if OpenAiRealtimeWhisperManager::is_realtime_model(&settings.model_id) {
            if base_url != REMOTE_STT_OPENAI_BASE_URL {
                let message = format!(
                    "{} requires the OpenAI Remote STT preset at {}.",
                    settings.model_id, REMOTE_STT_OPENAI_BASE_URL
                );
                self.record_error(settings, message.clone());
                return Err(anyhow!(message));
            }
            if translate_to_english {
                let message = format!(
                    "{} does not support Translate to English. Disable translation or select whisper-1.",
                    settings.model_id
                );
                self.record_error(settings, message.clone());
                return Err(anyhow!(message));
            }

            let app_settings = crate::settings::get_settings(&self.app_handle);
            let manager = OpenAiRealtimeWhisperManager::new(&self.app_handle)?;
            let result = manager
                .transcribe_flattened(
                    audio_samples,
                    &api_key.value,
                    OpenAiRealtimeWhisperOptions {
                        model: settings.model_id.clone(),
                        language,
                        prompt,
                        keywords: crate::actions::parse_openai_realtime_keywords(
                            &app_settings.openai_realtime_whisper_keywords,
                        ),
                        delay: app_settings.openai_realtime_whisper_delay,
                    },
                )
                .await;
            if let Err(error) = &result {
                self.record_error(settings, error.to_string());
            }
            return self
                .migrate_legacy_api_key_after_success(settings, &api_key, result)
                .map(|text| RemoteFileTranscription {
                    text,
                    segments: Vec::new(),
                });
        }

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
            return self
                .migrate_legacy_api_key_after_success(settings, &api_key, result)
                .map(|text| RemoteFileTranscription {
                    text,
                    segments: Vec::new(),
                });
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
            return self
                .migrate_legacy_api_key_after_success(settings, &api_key, result)
                .map(|text| RemoteFileTranscription {
                    text,
                    segments: Vec::new(),
                });
        }

        let wav_bytes = encode_wav_bytes(audio_samples).map_err(|e| {
            let message = format!("Failed to encode WAV: {}", e);
            self.record_error(settings, message.clone());
            anyhow!(message)
        })?;

        let file_size = wav_bytes.len();

        if gemini_expected_model.is_some() {
            if prompt.as_deref().is_some_and(|value| !value.trim().is_empty()) {
                self.record_info(
                    settings,
                    "Gemini 3.5 Transcribe does not accept a free-form STT prompt; AivoRelay omitted it from the request.".to_string(),
                );
            }
            let language = resolve_gemini_language(language);
            let result = match settings.provider_preset.as_str() {
                REMOTE_STT_PRESET_VERCEL => {
                    self.transcribe_gemini_via_vercel(
                        settings,
                        &base_url,
                        &wav_bytes,
                        language.as_deref(),
                        request_segments,
                        &api_key.value,
                    )
                    .await
                }
                REMOTE_STT_PRESET_GOOGLE => {
                    self.transcribe_gemini_via_google(
                        settings,
                        &base_url,
                        &wav_bytes,
                        language.as_deref(),
                        request_segments,
                        &api_key.value,
                    )
                    .await
                }
                _ => unreachable!("Gemini route was checked above"),
            };
            return self.migrate_legacy_api_key_after_success(settings, &api_key, result);
        }

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

        let response_format = if request_segments {
            "verbose_json"
        } else {
            "json"
        };
        let mut form = reqwest::multipart::Form::new()
            .text("model", settings.model_id.clone())
            .text("response_format", response_format.to_string())
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
                    let field = if uses_plural_language_hints(&settings.model_id) {
                        "languages[]"
                    } else {
                        "language"
                    };
                    form = form.text(field, lang);
                }
            }
        }

        // Remote prompt limits may be token-based and vary by provider/model.
        // Pass the prompt through so the provider can apply its authoritative
        // tokenizer and return its normal API error when the limit is exceeded.
        if let Some(p) = prompt {
            let trimmed = p.trim();
            if !trimmed.is_empty() {
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
            let snippet = remote_stt_log_snippet(
                &String::from_utf8_lossy(&body),
                &api_key.value,
                500,
                settings.unsafe_log_secrets,
            );
            let diagnostic = format!(
                "Remote STT failed: status={} elapsed_ms={} body_snippet={}",
                status, elapsed_ms, snippet
            );
            self.record_error(settings, diagnostic);
            return Err(anyhow!(redact_remote_stt_api_key(
                &parse_provider_error(&body, status),
                &api_key.value,
            )));
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
        Ok(RemoteFileTranscription {
            text: parsed.text,
            segments: parsed
                .segments
                .into_iter()
                .filter(|segment| {
                    segment.start.is_finite()
                        && segment.end.is_finite()
                        && segment.start >= 0.0
                        && segment.end >= segment.start
                        && !segment.text.trim().is_empty()
                })
                .collect(),
        })
    }

    async fn send_gemini_json_request(
        &self,
        settings: &RemoteSttSettings,
        route: &str,
        file_size: usize,
        api_key: &str,
        request: reqwest::RequestBuilder,
    ) -> Result<Vec<u8>> {
        if settings.debug_mode == RemoteSttDebugMode::Verbose {
            self.record_info(
                settings,
                format!(
                    "Gemini STT request route={} model={} wav_bytes={}",
                    route, settings.model_id, file_size
                ),
            );
        }

        let start = Instant::now();
        let response = request.send().await.map_err(|error| {
            let message = format!("Gemini STT request via {} failed: {}", route, error);
            self.record_error(settings, message.clone());
            anyhow!(message)
        })?;
        let status = response.status();
        let body = response.bytes().await.map_err(|error| {
            let message = format!("Gemini STT response via {} could not be read: {}", route, error);
            self.record_error(settings, message.clone());
            anyhow!(message)
        })?;
        let elapsed_ms = start.elapsed().as_millis();

        if settings.debug_mode == RemoteSttDebugMode::Verbose {
            self.record_info(
                settings,
                format!(
                    "Gemini STT response route={} status={} elapsed_ms={}",
                    route, status, elapsed_ms
                ),
            );
        }

        if !status.is_success() {
            let snippet = remote_stt_log_snippet(
                &String::from_utf8_lossy(&body),
                api_key,
                500,
                settings.unsafe_log_secrets,
            );
            self.record_error(
                settings,
                format!(
                    "Gemini STT failed: route={} status={} elapsed_ms={} body_snippet={}",
                    route, status, elapsed_ms, snippet
                ),
            );
            return Err(anyhow!(redact_remote_stt_api_key(
                &parse_provider_error(&body, status),
                api_key,
            )));
        }

        Ok(body.to_vec())
    }

    async fn transcribe_gemini_via_vercel(
        &self,
        settings: &RemoteSttSettings,
        base_url: &str,
        wav_bytes: &[u8],
        language: Option<&str>,
        request_segments: bool,
        api_key: &str,
    ) -> Result<RemoteFileTranscription> {
        let body = build_vercel_gemini_request_body(
            BASE64_STANDARD.encode(wav_bytes),
            language,
            request_segments,
        );
        let request = build_vercel_gemini_request(&self.client, base_url, api_key, &body);
        let response_body = self
            .send_gemini_json_request(
                settings,
                "Vercel AI Gateway",
                wav_bytes.len(),
                api_key,
                request,
            )
            .await?;
        let parsed: VercelTranscriptionResponse = serde_json::from_slice(&response_body)
            .map_err(|error| {
                let message = format!(
                    "Vercel Gemini transcription response could not be parsed: {}",
                    error
                );
                self.record_error(settings, message.clone());
                anyhow!(message)
            })?;
        let timed_tokens = timed_tokens_from_vercel_segments(parsed.segments);
        let result = RemoteFileTranscription {
            text: parsed.text,
            segments: timed_tokens_to_subtitle_segments(&timed_tokens),
        };
        if settings.debug_mode == RemoteSttDebugMode::Verbose {
            self.record_info(
                settings,
                format!("Gemini STT success output_len={}", result.text.len()),
            );
        }
        Ok(result)
    }

    async fn transcribe_gemini_via_google(
        &self,
        settings: &RemoteSttSettings,
        base_url: &str,
        wav_bytes: &[u8],
        language: Option<&str>,
        request_segments: bool,
        api_key: &str,
    ) -> Result<RemoteFileTranscription> {
        let body = build_google_gemini_request_body(
            BASE64_STANDARD.encode(wav_bytes),
            language,
            request_segments,
        );
        let request = build_google_gemini_request(&self.client, base_url, api_key, &body);
        let response_body = self
            .send_gemini_json_request(
                settings,
                "Google Gemini API",
                wav_bytes.len(),
                api_key,
                request,
            )
            .await?;
        let parsed: GoogleInteractionsTranscriptionResponse =
            serde_json::from_slice(&response_body).map_err(|error| {
                let message = format!(
                    "Google Gemini transcription response could not be parsed: {}",
                    error
                );
                self.record_error(settings, message.clone());
                anyhow!(message)
            })?;
        let result = parse_google_gemini_response(parsed);
        if settings.debug_mode == RemoteSttDebugMode::Verbose {
            self.record_info(
                settings,
                format!("Gemini STT success output_len={}", result.text.len()),
            );
        }
        Ok(result)
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

        let mut request = openai_realtime_ws_url(&settings.model_id)
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

        let session_update =
            build_openai_realtime_agent_session_update(&settings.model_id, &instructions);
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
            api_key,
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
            api_key,
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
                let preview = redacted_remote_stt_snippet(text.as_ref(), api_key, 200);
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
                let raw_message =
                    parse_provider_error_value(&payload, "OpenAI Realtime returned an error");
                self.record_error(
                    settings,
                    remote_stt_log_value(
                        &raw_message,
                        api_key,
                        settings.unsafe_log_secrets,
                    ),
                );
                return Err(anyhow!(redact_remote_stt_api_key(
                    &raw_message,
                    api_key,
                )));
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

        // Translation sessions use session.close as their end-of-input signal.
        // It flushes pending audio and must be followed by reads through session.closed.
        write
            .send(Message::Text(
                serde_json::json!({ "type": "session.close" })
                    .to_string()
                    .into(),
            ))
            .await
            .map_err(|e| anyhow!("Failed to close OpenAI Realtime Translate session: {}", e))?;
        write
            .flush()
            .await
            .map_err(|e| anyhow!("Failed to flush OpenAI Realtime Translate stream: {}", e))?;

        let mut output_text = String::new();
        let mut input_text = String::new();
        let close_deadline = Instant::now() + Duration::from_secs(DEFAULT_REQUEST_TIMEOUT_SECS);

        loop {
            let now = Instant::now();
            if now >= close_deadline {
                return Err(anyhow!(
                    "OpenAI Realtime Translate timed out waiting for session.closed"
                ));
            }
            let wait = close_deadline.saturating_duration_since(now);
            let frame = match timeout(wait, read.next()).await {
                Ok(Some(frame)) => frame,
                Ok(None) => {
                    return Err(anyhow!(
                        "OpenAI Realtime Translate WebSocket closed before session.closed"
                    ));
                }
                Err(_) => {
                    return Err(anyhow!(
                        "OpenAI Realtime Translate timed out waiting for session.closed"
                    ))
                }
            };
            let frame = frame
                .map_err(|e| anyhow!("OpenAI Realtime Translate WebSocket read failed: {}", e))?;

            let Message::Text(text) = frame else {
                continue;
            };
            let payload: Value = serde_json::from_str(text.as_ref()).map_err(|e| {
                let preview = redacted_remote_stt_snippet(text.as_ref(), api_key, 200);
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
                let raw_message = parse_provider_error_value(
                    &payload,
                    "OpenAI Realtime Translate returned an error",
                );
                self.record_error(
                    settings,
                    remote_stt_log_value(
                        &raw_message,
                        api_key,
                        settings.unsafe_log_secrets,
                    ),
                );
                return Err(anyhow!(redact_remote_stt_api_key(
                    &raw_message,
                    api_key,
                )));
            }

            if msg_type == "session.output_transcript.delta" {
                if let Some(delta) = payload.get("delta").and_then(|v| v.as_str()) {
                    output_text.push_str(delta);
                }
            } else if msg_type == "session.input_transcript.delta" {
                if let Some(delta) = payload.get("delta").and_then(|v| v.as_str()) {
                    input_text.push_str(delta);
                }
            } else if msg_type == "session.closed" {
                break;
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
        api_key: &str,
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
                let preview = redacted_remote_stt_snippet(text.as_ref(), api_key, 200);
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
                let raw_message =
                    parse_provider_error_value(&payload, "OpenAI Realtime returned an error");
                self.record_error(
                    settings,
                    remote_stt_log_value(
                        &raw_message,
                        api_key,
                        settings.unsafe_log_secrets,
                    ),
                );
                return Err(anyhow!(redact_remote_stt_api_key(
                    &raw_message,
                    api_key,
                )));
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
            let message = e.to_string();
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
            let snippet = remote_stt_log_snippet(
                &String::from_utf8_lossy(&body),
                &api_key.value,
                500,
                settings.unsafe_log_secrets,
            );
            let diagnostic = format!(
                "Remote STT test failed: status={} elapsed_ms={} body_snippet={}",
                status, elapsed_ms, snippet
            );
            self.record_error(settings, diagnostic);
            return Err(anyhow!(redact_remote_stt_api_key(
                &parse_provider_error(&body, status),
                &api_key.value,
            )));
        }

        self.migrate_legacy_api_key_after_success(settings, &api_key, Ok(()))?;
        Ok(())
    }
}

fn remote_stt_api_key_scope(settings: &RemoteSttSettings) -> &'static str {
    match settings.provider_preset.as_str() {
        REMOTE_STT_PRESET_GROQ => REMOTE_STT_PRESET_GROQ,
        REMOTE_STT_PRESET_OPENAI => REMOTE_STT_PRESET_OPENAI,
        REMOTE_STT_PRESET_VERCEL => REMOTE_STT_PRESET_VERCEL,
        REMOTE_STT_PRESET_GOOGLE => REMOTE_STT_PRESET_GOOGLE,
        REMOTE_STT_PRESET_CUSTOM => REMOTE_STT_PRESET_CUSTOM,
        _ => infer_remote_stt_preset(&settings.base_url),
    }
}

fn remote_stt_allows_legacy_api_key_fallback(settings: &RemoteSttSettings) -> bool {
    !matches!(
        remote_stt_api_key_scope(settings),
        REMOTE_STT_PRESET_VERCEL | REMOTE_STT_PRESET_GOOGLE
    )
}

fn remote_stt_api_key_provider_label(settings: &RemoteSttSettings) -> &'static str {
    match remote_stt_api_key_scope(settings) {
        REMOTE_STT_PRESET_GROQ => "Groq",
        REMOTE_STT_PRESET_OPENAI => "GPT Realtime",
        REMOTE_STT_PRESET_VERCEL => "Vercel AI Gateway",
        REMOTE_STT_PRESET_GOOGLE => "Google Gemini API",
        REMOTE_STT_PRESET_CUSTOM => "Custom API",
        _ => "Remote API",
    }
}

fn missing_remote_stt_api_key_message(settings: &RemoteSttSettings) -> String {
    format!(
        "Remote API key is missing for {}. Add it in Settings -> Models -> Remote via {}.",
        remote_stt_api_key_provider_label(settings),
        remote_stt_api_key_provider_label(settings)
    )
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
    let legacy_key = if scoped_key.trim().is_empty()
        && remote_stt_allows_legacy_api_key_fallback(settings)
    {
        Some(read_remote_stt_api_key_user(
            legacy_remote_stt_api_key_user(),
        )?)
    } else {
        None
    };

    select_remote_stt_api_key(Some(scoped_key), legacy_key)
        .ok_or_else(|| anyhow!(missing_remote_stt_api_key_message(settings)))
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
    let legacy_key = if remote_stt_allows_legacy_api_key_fallback(settings) {
        read_remote_stt_api_key_user(legacy_user)?
    } else {
        String::new()
    };
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
        build_google_gemini_request, build_google_gemini_request_body,
        build_openai_realtime_agent_session_update, build_vercel_gemini_request,
        build_vercel_gemini_request_body, normalize_gemini_language_code,
        parse_google_gemini_response,
        remote_stt_allows_legacy_api_key_fallback, remote_stt_api_key_clear_targets,
        redact_remote_stt_api_key,
        remote_stt_api_key_redaction_marker, remote_stt_log_value, select_remote_stt_api_key,
        supports_subtitle_timestamps, supports_translation, uses_plural_language_hints,
        GoogleInteractionsTranscriptionResponse, RemoteSttApiKeySource, TranscriptionResponse,
    };
    use crate::url_security::{REMOTE_STT_GOOGLE_BASE_URL, REMOTE_STT_VERCEL_BASE_URL};

    fn remote_settings(provider_preset: &str) -> crate::settings::RemoteSttSettings {
        crate::settings::RemoteSttSettings {
            base_url: "https://example.com".to_string(),
            provider_preset: provider_preset.to_string(),
            allow_insecure_http: false,
            model_id: "test-model".to_string(),
            debug_capture: false,
            debug_mode: crate::settings::RemoteSttDebugMode::Normal,
            unsafe_log_secrets: false,
        }
    }

    #[test]
    fn realtime_agent_session_update_uses_selected_model() {
        let legacy = build_openai_realtime_agent_session_update(" gpt-realtime-2 ", "test");
        assert_eq!(legacy["session"]["model"], "gpt-realtime-2");

        let latest = build_openai_realtime_agent_session_update("gpt-realtime-2.1", "test");
        assert_eq!(latest["session"]["model"], "gpt-realtime-2.1");
    }

    #[test]
    fn gpt_realtime_2_supports_remote_stt_translation() {
        assert!(supports_translation("gpt-realtime-2"));
    }

    #[test]
    fn gpt_realtime_2_1_supports_remote_stt_translation() {
        assert!(supports_translation("gpt-realtime-2.1"));
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
    fn gpt_transcribe_uses_plural_language_hints_without_translation() {
        assert!(uses_plural_language_hints("gpt-transcribe"));
        assert!(!uses_plural_language_hints("gpt-live-transcribe"));
        assert!(!supports_translation("gpt-transcribe"));
        assert!(!supports_translation("gpt-live-transcribe"));
        assert!(!supports_translation("gpt-realtime-whisper"));
    }

    #[test]
    fn subtitle_timestamps_require_non_realtime_whisper_endpoint() {
        assert!(supports_subtitle_timestamps("whisper-1"));
        assert!(supports_subtitle_timestamps("whisper-large-v3"));
        assert!(supports_subtitle_timestamps(
            "google/gemini-3.5-transcribe"
        ));
        assert!(supports_subtitle_timestamps("gemini-3.5-transcribe"));
        assert!(!supports_subtitle_timestamps("gpt-transcribe"));
        assert!(!supports_subtitle_timestamps("gpt-realtime-whisper"));
        assert!(!supports_subtitle_timestamps("gpt-realtime-2.1"));
    }

    #[test]
    fn gemini_language_hints_use_documented_bcp47_locales() {
        assert_eq!(normalize_gemini_language_code("ru"), Some("ru-RU"));
        assert_eq!(normalize_gemini_language_code("en-GB"), Some("en-GB"));
        assert_eq!(
            normalize_gemini_language_code("zh-Hans"),
            Some("cmn-Hans-CN")
        );
        assert_eq!(normalize_gemini_language_code("en"), None);
        assert_eq!(normalize_gemini_language_code("cy"), None);
    }

    #[test]
    fn vercel_gemini_request_matches_gateway_transcription_v4_contract() {
        let body = build_vercel_gemini_request_body(
            "AQIDBA==".to_string(),
            Some("ru-RU"),
            true,
        );
        assert_eq!(
            body,
            serde_json::json!({
                "audio": "AQIDBA==",
                "mediaType": "audio/wav",
                "providerOptions": {
                    "google": {
                        "languageCodes": ["ru-RU"],
                        "wordTimestamp": true
                    }
                }
            })
        );

        let request = build_vercel_gemini_request(
            &reqwest::Client::new(),
            REMOTE_STT_VERCEL_BASE_URL,
            "test-vercel-key",
            &body,
        )
        .build()
        .unwrap();
        assert_eq!(request.method(), reqwest::Method::POST);
        assert_eq!(
            request.url().as_str(),
            "https://ai-gateway.vercel.sh/v4/ai/transcription-model"
        );
        assert_eq!(request.headers()["authorization"], "Bearer test-vercel-key");
        assert_eq!(request.headers()["ai-gateway-protocol-version"], "0.0.1");
        assert_eq!(request.headers()["ai-gateway-auth-method"], "api-key");
        assert_eq!(
            request.headers()["ai-transcription-model-specification-version"],
            "4"
        );
        assert_eq!(
            request.headers()["ai-model-id"],
            "google/gemini-3.5-transcribe"
        );
        assert_eq!(request.headers()["content-type"], "application/json");
        let wire_body: serde_json::Value = serde_json::from_slice(
            request.body().unwrap().as_bytes().unwrap(),
        )
        .unwrap();
        assert_eq!(wire_body, body);
    }

    #[test]
    fn direct_google_gemini_request_matches_interactions_contract() {
        let body = build_google_gemini_request_body(
            "AQIDBA==".to_string(),
            Some("ru-RU"),
            true,
        );
        assert_eq!(
            body,
            serde_json::json!({
                "model": "gemini-3.5-transcribe",
                "input": [{
                    "type": "audio",
                    "data": "AQIDBA==",
                    "mime_type": "audio/wav"
                }],
                "generation_config": {
                    "transcription_config": {
                        "language_codes": ["ru-RU"],
                        "mode": {
                            "type": "verbatim",
                            "timestamp_granularities": ["word"]
                        }
                    }
                }
            })
        );

        let request = build_google_gemini_request(
            &reqwest::Client::new(),
            REMOTE_STT_GOOGLE_BASE_URL,
            "test-google-key",
            &body,
        )
        .build()
        .unwrap();
        assert_eq!(request.method(), reqwest::Method::POST);
        assert_eq!(
            request.url().as_str(),
            "https://generativelanguage.googleapis.com/v1beta/interactions"
        );
        assert_eq!(request.headers()["x-goog-api-key"], "test-google-key");
        assert!(request.headers().get("authorization").is_none());
        assert_eq!(request.headers()["content-type"], "application/json");
        let wire_body: serde_json::Value = serde_json::from_slice(
            request.body().unwrap().as_bytes().unwrap(),
        )
        .unwrap();
        assert_eq!(wire_body, body);
    }

    #[test]
    fn gemini_request_omits_optional_configuration_when_not_needed() {
        let vercel = build_vercel_gemini_request_body("AQID".to_string(), None, false);
        assert!(vercel.get("providerOptions").is_none());

        let google = build_google_gemini_request_body("AQID".to_string(), None, false);
        assert!(google.get("generation_config").is_none());
    }

    #[test]
    fn direct_google_response_extracts_text_and_word_timestamps() {
        let response: GoogleInteractionsTranscriptionResponse =
            serde_json::from_value(serde_json::json!({
                "status": "completed",
                "steps": [{
                    "type": "model_output",
                    "content": [{
                        "type": "text",
                        "text": "The quick fox.",
                        "annotations": [
                            {
                                "type": "word_info",
                                "text": "The",
                                "start_offset": "0.100s",
                                "end_offset": "0.300s"
                            },
                            {
                                "type": "word_info",
                                "text": "quick",
                                "start_offset": "0.300s",
                                "end_offset": "0.600s"
                            },
                            {
                                "type": "word_info",
                                "text": "fox.",
                                "start_offset": "0.600s",
                                "end_offset": "1s"
                            }
                        ]
                    }]
                }]
            }))
            .unwrap();
        let parsed = parse_google_gemini_response(response);
        assert_eq!(parsed.text, "The quick fox.");
        assert_eq!(parsed.segments.len(), 1);
        assert_eq!(parsed.segments[0].text, "The quick fox.");
        assert_eq!(parsed.segments[0].start, 0.1);
        assert_eq!(parsed.segments[0].end, 1.0);
    }

    #[test]
    fn verbose_json_segments_deserialize_with_real_timestamps() {
        let response: TranscriptionResponse = serde_json::from_value(serde_json::json!({
            "text": "Hello world.",
            "segments": [{ "start": 0.25, "end": 1.75, "text": "Hello world.", "id": 0 }]
        }))
        .unwrap();

        assert_eq!(response.segments.len(), 1);
        assert_eq!(response.segments[0].start, 0.25);
        assert_eq!(response.segments[0].end, 1.75);
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
    fn gemini_routes_never_reuse_a_legacy_provider_key() {
        assert!(!remote_stt_allows_legacy_api_key_fallback(
            &remote_settings("vercel")
        ));
        assert!(!remote_stt_allows_legacy_api_key_fallback(
            &remote_settings("google")
        ));
        assert!(remote_stt_allows_legacy_api_key_fallback(
            &remote_settings("openai")
        ));
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
    fn provider_errors_redact_every_api_key_occurrence() {
        let key = "stt-secret-key";
        let safe = redact_remote_stt_api_key(
            &format!("Authorization: Bearer {key}; echoed again: {key}"),
            key,
        );

        assert!(!safe.contains(key));
        let marker = remote_stt_api_key_redaction_marker(key);
        assert_eq!(marker, "[redacted key, SHA-256: 384a8ae6f981e822]");
        assert_eq!(safe.matches(&marker).count(), 2);
    }

    #[test]
    fn unsafe_secret_logging_only_bypasses_log_redaction() {
        let key = "stt-secret-key";
        let provider_error = format!("Provider echoed {key}");

        assert!(!remote_stt_log_value(&provider_error, key, false).contains(key));
        assert_eq!(
            remote_stt_log_value(&provider_error, key, true),
            provider_error
        );
        assert!(!redact_remote_stt_api_key(&provider_error, key).contains(key));
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
