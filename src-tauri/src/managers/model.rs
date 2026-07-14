use super::model_capabilities::{
    CapabilityProbe, CapabilityProber, Compatibility, GgufHeaderProber,
};
use crate::settings::{get_settings, write_settings};
use anyhow::Result;
use flate2::read::GzDecoder;
use futures_util::StreamExt;
use hf_hub::api::tokio::{ApiBuilder, ApiError, Progress};
use hf_hub::{Cache, Repo, RepoType};
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use specta::Type;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tar::Archive;
use tauri::{AppHandle, Emitter, Manager};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum EngineType {
    TranscribeCpp,
    Whisper,
    Parakeet,
    Moonshine,
    MoonshineStreaming,
    SenseVoice,
    GigaAM,
    Canary,
    Cohere,
}

const HF_SOURCE_PREFIX: &str = "hf://";

fn hf_source_url(repo_id: &str, revision: &str, filename: &str) -> String {
    format!("{HF_SOURCE_PREFIX}{repo_id}|{revision}|{filename}")
}

fn parse_hf_source_url(url: &str) -> Option<(String, String, String)> {
    let value = url.strip_prefix(HF_SOURCE_PREFIX)?;
    let mut parts = value.splitn(3, '|');
    let repo_id = parts.next()?.to_string();
    let revision = parts.next()?.to_string();
    let filename = parts.next()?.to_string();
    Some((repo_id, revision, filename))
}

fn model_hf_source(model: &ModelInfo) -> Option<(String, String, String)> {
    model.url.as_deref().and_then(parse_hf_source_url)
}

fn hf_cached_path(repo_id: &str, revision: &str, filename: &str) -> Option<PathBuf> {
    Cache::from_env()
        .repo(Repo::with_revision(
            repo_id.to_string(),
            RepoType::Model,
            revision.to_string(),
        ))
        .get(filename)
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub filename: String,
    pub url: Option<String>,
    pub sha256: Option<String>,
    pub size_mb: u64,
    pub is_downloaded: bool,
    pub is_downloading: bool,
    pub partial_size: u64,
    pub is_directory: bool,
    pub engine_type: EngineType,
    pub accuracy_score: f32,        // 0.0 to 1.0, higher is more accurate
    pub speed_score: f32,           // 0.0 to 1.0, higher is faster
    pub supports_translation: bool, // Whether the model supports translating to English
    pub supports_streaming: bool,   // Whether the model supports native realtime streaming
    pub supports_language_detection: bool, // Whether the model can auto-detect language
    pub is_recommended: bool,       // Whether this is the recommended model for new users
    pub supported_languages: Vec<String>, // Languages this model can transcribe
    pub is_custom: bool,            // Whether this is a user-provided custom model
}

const CHINESE_LANGUAGE_CODE: &str = "zh";

fn recognition_language(language: &str) -> &str {
    match language {
        "zh-Hans" | "zh-Hant" => CHINESE_LANGUAGE_CODE,
        other => other,
    }
}

fn base_language(language: &str) -> &str {
    match language.split_once('-') {
        Some((base, _)) => base,
        None => language,
    }
}

fn canonicalize_supported_languages(languages: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut canonical = Vec::with_capacity(languages.len());

    for language in languages {
        let language = recognition_language(base_language(&language)).to_string();
        if seen.insert(language.clone()) {
            canonical.push(language);
        }
    }

    if seen.contains(CHINESE_LANGUAGE_CODE) {
        for language in ["zh-Hans", "zh-Hant"] {
            if seen.insert(language.to_string()) {
                canonical.push(language.to_string());
            }
        }
    }

    canonical
}

pub fn effective_language(
    requested_language: &str,
    supported_languages: &[String],
    supports_language_detection: bool,
) -> String {
    let intent = requested_language.trim();
    if intent.is_empty() {
        return "auto".to_string();
    }
    if supported_languages.is_empty() {
        return intent.to_string();
    }

    if intent != "auto" && intent != "os_input" {
        if let Some(code) = supported_languages
            .iter()
            .find(|language| base_language(language) == base_language(intent))
        {
            if matches!(intent, "zh-Hans" | "zh-Hant") && base_language(code) == "zh" {
                return intent.to_string();
            }
            return recognition_language(code).to_string();
        }
        return intent.to_string();
    }

    if supports_language_detection {
        return "auto".to_string();
    }

    if let Some(en) = supported_languages
        .iter()
        .find(|language| base_language(language) == "en")
    {
        return recognition_language(en).to_string();
    }

    recognition_language(&supported_languages[0]).to_string()
}

struct LocalCaps {
    supports_streaming: bool,
    supports_translation: bool,
    supports_language_detection: bool,
    supported_languages: Vec<String>,
}

fn local_caps(probe: &CapabilityProbe) -> LocalCaps {
    LocalCaps {
        supports_streaming: probe.supports_streaming.unwrap_or(false),
        supports_translation: probe.supports_translation.unwrap_or(false),
        supports_language_detection: probe.supports_language_detect.unwrap_or(false),
        supported_languages: canonicalize_supported_languages(
            probe.languages.clone().unwrap_or_default(),
        ),
    }
}

fn probed_display_name(probe: &CapabilityProbe) -> Option<String> {
    probe
        .display_name
        .as_ref()
        .or(probe.variant.as_ref())
        .filter(|name| !name.trim().is_empty())
        .cloned()
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DownloadProgress {
    pub model_id: String,
    pub downloaded: u64,
    pub total: u64,
    pub percentage: f64,
}

#[derive(Clone)]
struct HfDownloadProgress {
    app_handle: AppHandle,
    model_id: String,
    state: Arc<Mutex<HfDownloadProgressState>>,
}

struct HfDownloadProgressState {
    downloaded: u64,
    total: u64,
}

impl HfDownloadProgress {
    fn new(app_handle: AppHandle, model_id: String, fallback_total: u64) -> Self {
        Self {
            app_handle,
            model_id,
            state: Arc::new(Mutex::new(HfDownloadProgressState {
                downloaded: 0,
                total: fallback_total,
            })),
        }
    }

    fn emit(&self, downloaded: u64, total: u64) {
        let percentage = if total > 0 {
            (downloaded as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        let _ = self.app_handle.emit(
            "model-download-progress",
            &DownloadProgress {
                model_id: self.model_id.clone(),
                downloaded,
                total,
                percentage,
            },
        );
    }
}

impl Progress for HfDownloadProgress {
    async fn init(&mut self, size: usize, _filename: &str) {
        let total = size as u64;
        {
            let mut state = self.state.lock().unwrap();
            state.downloaded = 0;
            state.total = total;
        }
        self.emit(0, total);
    }

    async fn update(&mut self, size: usize) {
        let (downloaded, total) = {
            let mut state = self.state.lock().unwrap();
            state.downloaded = state.downloaded.saturating_add(size as u64);
            (state.downloaded, state.total)
        };
        self.emit(downloaded, total);
    }

    async fn finish(&mut self) {
        let total = {
            let mut state = self.state.lock().unwrap();
            state.downloaded = state.total;
            state.total
        };
        self.emit(total, total);
    }
}

pub struct ModelManager {
    app_handle: AppHandle,
    models_dir: PathBuf,
    available_models: Mutex<HashMap<String, ModelInfo>>,
    /// Cancellation tokens for active downloads, keyed by model_id
    cancellation_tokens: Mutex<HashMap<String, CancellationToken>>,
}

impl ModelManager {
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        // Create models directory in app data
        let models_dir = crate::portable::app_data_dir(app_handle)
            .map_err(|e| anyhow::anyhow!("Failed to get app data dir: {}", e))?
            .join("models");

        if !models_dir.exists() {
            fs::create_dir_all(&models_dir)?;
        }

        let mut available_models = HashMap::new();

        // Whisper supported languages (99 languages from tokenizer)
        // Including zh-Hans and zh-Hant variants to match frontend language codes
        let whisper_languages: Vec<String> = vec![
            "en", "zh", "zh-Hans", "zh-Hant", "de", "es", "ru", "ko", "fr", "ja", "pt", "tr", "pl",
            "ca", "nl", "ar", "sv", "it", "id", "hi", "fi", "vi", "he", "uk", "el", "ms", "cs",
            "ro", "da", "hu", "ta", "no", "th", "ur", "hr", "bg", "lt", "la", "mi", "ml", "cy",
            "sk", "te", "fa", "lv", "bn", "sr", "az", "sl", "kn", "et", "mk", "br", "eu", "is",
            "hy", "ne", "mn", "bs", "kk", "sq", "sw", "gl", "mr", "pa", "si", "km", "sn", "yo",
            "so", "af", "oc", "ka", "be", "tg", "sd", "gu", "am", "yi", "lo", "uz", "fo", "ht",
            "ps", "tk", "nn", "mt", "sa", "lb", "my", "bo", "tl", "mg", "as", "tt", "haw", "ln",
            "ha", "ba", "jw", "su", "yue",
        ]
        .into_iter()
        .map(String::from)
        .collect();

        // Parakeet V3 supported languages (25 EU languages + Russian/Ukrainian):
        // bg, hr, cs, da, nl, en, et, fi, fr, de, el, hu, it, lv, lt, mt, pl, pt, ro, sk, sl, es, sv, ru, uk
        let parakeet_v3_languages: Vec<String> = vec![
            "bg", "hr", "cs", "da", "nl", "en", "et", "fi", "fr", "de", "el", "hu", "it", "lv",
            "lt", "mt", "pl", "pt", "ro", "sk", "sl", "es", "sv", "ru", "uk",
        ]
        .into_iter()
        .map(String::from)
        .collect();

        // SenseVoice supported languages
        let sense_voice_languages: Vec<String> =
            vec!["zh", "zh-Hans", "zh-Hant", "en", "yue", "ja", "ko"]
                .into_iter()
                .map(String::from)
                .collect();

        // TODO this should be read from a JSON file or something..
        available_models.insert(
            "small".to_string(),
            ModelInfo {
                id: "small".to_string(),
                name: "Whisper Small".to_string(),
                description: "Fast and fairly accurate.".to_string(),
                filename: "ggml-small.bin".to_string(),
                url: Some("https://blob.handy.computer/ggml-small.bin".to_string()),
                sha256: Some(
                    "1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b".to_string(),
                ),
                size_mb: 465,
                is_downloaded: false,
                is_downloading: false,
                partial_size: 0,
                is_directory: false,
                engine_type: EngineType::Whisper,
                accuracy_score: 0.60,
                speed_score: 0.85,
                supports_translation: true,
                supports_streaming: false,
                supports_language_detection: true,
                is_recommended: false,
                supported_languages: whisper_languages.clone(),
                is_custom: false,
            },
        );

        // Add downloadable models
        available_models.insert(
            "medium".to_string(),
            ModelInfo {
                id: "medium".to_string(),
                name: "Whisper Medium".to_string(),
                description: "Good accuracy, medium speed".to_string(),
                filename: "whisper-medium-q4_1.bin".to_string(),
                url: Some("https://blob.handy.computer/whisper-medium-q4_1.bin".to_string()),
                sha256: Some(
                    "79283fc1f9fe12ca3248543fbd54b73292164d8df5a16e095e2bceeaaabddf57".to_string(),
                ),
                size_mb: 469,
                is_downloaded: false,
                is_downloading: false,
                partial_size: 0,
                is_directory: false,
                engine_type: EngineType::Whisper,
                accuracy_score: 0.75,
                speed_score: 0.60,
                supports_translation: true,
                supports_streaming: false,
                supports_language_detection: true,
                is_recommended: false,
                supported_languages: whisper_languages.clone(),
                is_custom: false,
            },
        );

        available_models.insert(
            "turbo".to_string(),
            ModelInfo {
                id: "turbo".to_string(),
                name: "Whisper Turbo".to_string(),
                description: "Balanced accuracy and speed.".to_string(),
                filename: "ggml-large-v3-turbo.bin".to_string(),
                url: Some("https://blob.handy.computer/ggml-large-v3-turbo.bin".to_string()),
                sha256: Some(
                    "1fc70f774d38eb169993ac391eea357ef47c88757ef72ee5943879b7e8e2bc69".to_string(),
                ),
                size_mb: 1549,
                is_downloaded: false,
                is_downloading: false,
                partial_size: 0,
                is_directory: false,
                engine_type: EngineType::Whisper,
                accuracy_score: 0.80,
                speed_score: 0.40,
                supports_translation: false, // Turbo doesn't support translation
                supports_streaming: false,
                supports_language_detection: true,
                is_recommended: false,
                supported_languages: whisper_languages.clone(),
                is_custom: false,
            },
        );

        available_models.insert(
            "large".to_string(),
            ModelInfo {
                id: "large".to_string(),
                name: "Whisper Large".to_string(),
                description: "Good accuracy, but slow.".to_string(),
                filename: "ggml-large-v3-q5_0.bin".to_string(),
                url: Some("https://blob.handy.computer/ggml-large-v3-q5_0.bin".to_string()),
                sha256: Some(
                    "d75795ecff3f83b5faa89d1900604ad8c780abd5739fae406de19f23ecd98ad1".to_string(),
                ),
                size_mb: 1031,
                is_downloaded: false,
                is_downloading: false,
                partial_size: 0,
                is_directory: false,
                engine_type: EngineType::Whisper,
                accuracy_score: 0.85,
                speed_score: 0.30,
                supports_translation: true,
                supports_streaming: false,
                supports_language_detection: true,
                is_recommended: false,
                supported_languages: whisper_languages.clone(),
                is_custom: false,
            },
        );

        available_models.insert(
            "breeze-asr".to_string(),
            ModelInfo {
                id: "breeze-asr".to_string(),
                name: "Breeze ASR".to_string(),
                description: "Optimized for Taiwanese Mandarin. Code-switching support."
                    .to_string(),
                filename: "breeze-asr-q5_k.bin".to_string(),
                url: Some("https://blob.handy.computer/breeze-asr-q5_k.bin".to_string()),
                sha256: Some(
                    "8efbf0ce8a3f50fe332b7617da787fb81354b358c288b008d3bdef8359df64c6".to_string(),
                ),
                size_mb: 1030,
                is_downloaded: false,
                is_downloading: false,
                partial_size: 0,
                is_directory: false,
                engine_type: EngineType::Whisper,
                accuracy_score: 0.85,
                speed_score: 0.35,
                supports_translation: false,
                supports_streaming: false,
                supports_language_detection: true,
                is_recommended: false,
                // Official model card positions Breeze ASR as optimized for
                // Taiwanese Mandarin and Mandarin-English code-switching.
                supported_languages: vec!["zh", "zh-Hant", "en"]
                    .into_iter()
                    .map(String::from)
                    .collect(),
                is_custom: false,
            },
        );

        // Add NVIDIA Parakeet models (directory-based)
        available_models.insert(
            "parakeet-tdt-0.6b-v2".to_string(),
            ModelInfo {
                id: "parakeet-tdt-0.6b-v2".to_string(),
                name: "Parakeet V2".to_string(),
                description: "English only. The best model for English speakers.".to_string(),
                filename: "parakeet-tdt-0.6b-v2-int8".to_string(), // Directory name
                url: Some("https://blob.handy.computer/parakeet-v2-int8.tar.gz".to_string()),
                sha256: Some(
                    "ac9b9429984dd565b25097337a887bb7f0f8ac393573661c651f0e7d31563991".to_string(),
                ),
                size_mb: 451,
                is_downloaded: false,
                is_downloading: false,
                partial_size: 0,
                is_directory: true,
                engine_type: EngineType::Parakeet,
                accuracy_score: 0.85,
                speed_score: 0.85,
                supports_translation: false,
                supports_streaming: false,
                supports_language_detection: true,
                is_recommended: false,
                supported_languages: vec!["en".to_string()],
                is_custom: false,
            },
        );

        available_models.insert(
            "parakeet-tdt-0.6b-v3".to_string(),
            ModelInfo {
                id: "parakeet-tdt-0.6b-v3".to_string(),
                name: "Parakeet V3".to_string(),
                description: "Fast and accurate. Supports 25 European languages.".to_string(),
                filename: "parakeet-tdt-0.6b-v3-int8".to_string(), // Directory name
                url: Some("https://blob.handy.computer/parakeet-v3-int8.tar.gz".to_string()),
                sha256: Some(
                    "43d37191602727524a7d8c6da0eef11c4ba24320f5b4730f1a2497befc2efa77".to_string(),
                ),
                size_mb: 456,
                is_downloaded: false,
                is_downloading: false,
                partial_size: 0,
                is_directory: true,
                engine_type: EngineType::Parakeet,
                accuracy_score: 0.80,
                speed_score: 0.85,
                supports_translation: false,
                supports_streaming: false,
                supports_language_detection: true,
                is_recommended: true,
                supported_languages: parakeet_v3_languages,
                is_custom: false,
            },
        );

        available_models.insert(
            "moonshine-base".to_string(),
            ModelInfo {
                id: "moonshine-base".to_string(),
                name: "Moonshine Base".to_string(),
                description: "Very fast, English only. Handles accents well.".to_string(),
                filename: "moonshine-base".to_string(),
                url: Some("https://blob.handy.computer/moonshine-base.tar.gz".to_string()),
                sha256: Some(
                    "04bf6ab012cfceebd4ac7cf88c1b31d027bbdd3cd704649b692e2e935236b7e8".to_string(),
                ),
                size_mb: 55,
                is_downloaded: false,
                is_downloading: false,
                partial_size: 0,
                is_directory: true,
                engine_type: EngineType::Moonshine,
                accuracy_score: 0.70,
                speed_score: 0.90,
                supports_translation: false,
                supports_streaming: false,
                supports_language_detection: true,
                is_recommended: false,
                supported_languages: vec!["en".to_string()],
                is_custom: false,
            },
        );

        available_models.insert(
            "moonshine-tiny-streaming-en".to_string(),
            ModelInfo {
                id: "moonshine-tiny-streaming-en".to_string(),
                name: "Moonshine V2 Tiny".to_string(),
                description: "Ultra-fast, English only".to_string(),
                filename: "moonshine-tiny-streaming-en".to_string(),
                url: Some(
                    "https://blob.handy.computer/moonshine-tiny-streaming-en.tar.gz".to_string(),
                ),
                sha256: Some(
                    "465addcfca9e86117415677dfdc98b21edc53537210333a3ecdb58509a80abaf".to_string(),
                ),
                size_mb: 31,
                is_downloaded: false,
                is_downloading: false,
                partial_size: 0,
                is_directory: true,
                engine_type: EngineType::MoonshineStreaming,
                accuracy_score: 0.55,
                speed_score: 0.95,
                supports_translation: false,
                supports_streaming: false,
                supports_language_detection: true,
                is_recommended: false,
                supported_languages: vec!["en".to_string()],
                is_custom: false,
            },
        );

        available_models.insert(
            "moonshine-small-streaming-en".to_string(),
            ModelInfo {
                id: "moonshine-small-streaming-en".to_string(),
                name: "Moonshine V2 Small".to_string(),
                description: "Fast, English only. Good balance of speed and accuracy.".to_string(),
                filename: "moonshine-small-streaming-en".to_string(),
                url: Some(
                    "https://blob.handy.computer/moonshine-small-streaming-en.tar.gz".to_string(),
                ),
                sha256: Some(
                    "dbb3e1c1832bd88a4ac712f7449a136cc2c9a18c5fe33a12ed1b7cb1cfe9cdd5".to_string(),
                ),
                size_mb: 99,
                is_downloaded: false,
                is_downloading: false,
                partial_size: 0,
                is_directory: true,
                engine_type: EngineType::MoonshineStreaming,
                accuracy_score: 0.65,
                speed_score: 0.90,
                supports_translation: false,
                supports_streaming: false,
                supports_language_detection: true,
                is_recommended: false,
                supported_languages: vec!["en".to_string()],
                is_custom: false,
            },
        );

        available_models.insert(
            "moonshine-medium-streaming-en".to_string(),
            ModelInfo {
                id: "moonshine-medium-streaming-en".to_string(),
                name: "Moonshine V2 Medium".to_string(),
                description: "English only. High quality.".to_string(),
                filename: "moonshine-medium-streaming-en".to_string(),
                url: Some(
                    "https://blob.handy.computer/moonshine-medium-streaming-en.tar.gz".to_string(),
                ),
                sha256: Some(
                    "07a66f3bff1c77e75a2f637e5a263928a08baae3c29c4c053fc968a9a9373d13".to_string(),
                ),
                size_mb: 192,
                is_downloaded: false,
                is_downloading: false,
                partial_size: 0,
                is_directory: true,
                engine_type: EngineType::MoonshineStreaming,
                accuracy_score: 0.75,
                speed_score: 0.80,
                supports_translation: false,
                supports_streaming: false,
                supports_language_detection: true,
                is_recommended: false,
                supported_languages: vec!["en".to_string()],
                is_custom: false,
            },
        );

        available_models.insert(
            "sense-voice-int8".to_string(),
            ModelInfo {
                id: "sense-voice-int8".to_string(),
                name: "SenseVoice".to_string(),
                description: "Very fast. Chinese, English, Japanese, Korean, Cantonese."
                    .to_string(),
                filename: "sense-voice-int8".to_string(),
                url: Some("https://blob.handy.computer/sense-voice-int8.tar.gz".to_string()),
                sha256: Some(
                    "171d611fe5d353a50bbb741b6f3ef42559b1565685684e9aa888ef563ba3e8a4".to_string(),
                ),
                size_mb: 152,
                is_downloaded: false,
                is_downloading: false,
                partial_size: 0,
                is_directory: true,
                engine_type: EngineType::SenseVoice,
                accuracy_score: 0.65,
                speed_score: 0.95,
                supports_translation: false,
                supports_streaming: false,
                supports_language_detection: true,
                is_recommended: false,
                supported_languages: sense_voice_languages,
                is_custom: false,
            },
        );

        // GigaAM v3 supported languages
        let gigaam_languages: Vec<String> = vec!["ru"].into_iter().map(String::from).collect();

        available_models.insert(
            "gigaam-v3-e2e-ctc".to_string(),
            ModelInfo {
                id: "gigaam-v3-e2e-ctc".to_string(),
                name: "GigaAM v3".to_string(),
                description: "Russian speech recognition. Fast and accurate.".to_string(),
                filename: "giga-am-v3-int8".to_string(),
                url: Some("https://blob.handy.computer/giga-am-v3-int8.tar.gz".to_string()),
                sha256: Some(
                    "d872462268430db140b69b72e0fc4b787b194c1dbe51b58de39444d55b6da45b".to_string(),
                ),
                size_mb: 151,
                is_downloaded: false,
                is_downloading: false,
                partial_size: 0,
                is_directory: true,
                engine_type: EngineType::GigaAM,
                accuracy_score: 0.85,
                speed_score: 0.75,
                supports_translation: false,
                supports_streaming: false,
                supports_language_detection: true,
                is_recommended: false,
                supported_languages: gigaam_languages,
                is_custom: false,
            },
        );

        let canary_flash_languages: Vec<String> = vec!["en", "de", "es", "fr"]
            .into_iter()
            .map(String::from)
            .collect();

        available_models.insert(
            "canary-180m-flash".to_string(),
            ModelInfo {
                id: "canary-180m-flash".to_string(),
                name: "Canary 180M Flash".to_string(),
                description: "Very fast. English, German, Spanish, French. Supports translation."
                    .to_string(),
                filename: "canary-180m-flash".to_string(),
                url: Some("https://blob.handy.computer/canary-180m-flash.tar.gz".to_string()),
                sha256: Some(
                    "6d9cfca6118b296e196eaedc1c8fa9788305a7b0f1feafdb6dc91932ab6e53f7".to_string(),
                ),
                size_mb: 146,
                is_downloaded: false,
                is_downloading: false,
                partial_size: 0,
                is_directory: true,
                engine_type: EngineType::Canary,
                accuracy_score: 0.75,
                speed_score: 0.85,
                supports_translation: true,
                supports_streaming: false,
                supports_language_detection: false,
                is_recommended: false,
                supported_languages: canary_flash_languages,
                is_custom: false,
            },
        );

        let canary_1b_languages: Vec<String> = vec![
            "bg", "hr", "cs", "da", "nl", "en", "et", "fi", "fr", "de", "el", "hu", "it", "lv",
            "lt", "mt", "pl", "pt", "ro", "sk", "sl", "es", "sv", "ru", "uk",
        ]
        .into_iter()
        .map(String::from)
        .collect();

        available_models.insert(
            "canary-1b-v2".to_string(),
            ModelInfo {
                id: "canary-1b-v2".to_string(),
                name: "Canary 1B v2".to_string(),
                description: "Accurate multilingual. 25 European languages. Supports translation."
                    .to_string(),
                filename: "canary-1b-v2".to_string(),
                url: Some("https://blob.handy.computer/canary-1b-v2.tar.gz".to_string()),
                sha256: Some(
                    "02305b2a25f9cf3e7deaffa7f94df00efa44f442cd55c101c2cb9c000f904666".to_string(),
                ),
                size_mb: 691,
                is_downloaded: false,
                is_downloading: false,
                partial_size: 0,
                is_directory: true,
                engine_type: EngineType::Canary,
                accuracy_score: 0.85,
                speed_score: 0.70,
                supports_translation: true,
                supports_streaming: false,
                supports_language_detection: false,
                is_recommended: false,
                supported_languages: canary_1b_languages,
                is_custom: false,
            },
        );

        let cohere_languages: Vec<String> = vec![
            "en", "fr", "de", "it", "es", "pt", "el", "nl", "pl", "zh", "zh-Hans", "zh-Hant", "ja",
            "ko", "vi", "ar",
        ]
        .into_iter()
        .map(String::from)
        .collect();

        available_models.insert(
            "cohere-int8".to_string(),
            ModelInfo {
                id: "cohere-int8".to_string(),
                name: "Cohere".to_string(),
                description: "A large, slower, but very accurate multilingual model.".to_string(),
                filename: "cohere-int8".to_string(),
                url: Some("https://blob.handy.computer/cohere-int8.tar.gz".to_string()),
                sha256: Some(
                    "ea2257d52434f3644574f187dcdcf666e302cd11b92866116ab8e14cd9c887f0".to_string(),
                ),
                size_mb: 1708,
                is_downloaded: false,
                is_downloading: false,
                partial_size: 0,
                is_directory: true,
                engine_type: EngineType::Cohere,
                accuracy_score: 0.90,
                speed_score: 0.60,
                supports_translation: false,
                supports_streaming: false,
                supports_language_detection: true,
                is_recommended: false,
                supported_languages: cohere_languages,
                is_custom: false,
            },
        );

        Self::seed_catalog_models(&mut available_models);

        // Auto-discover custom transcribe.cpp models (.bin / .gguf) in the models directory.
        if let Err(e) = Self::discover_custom_transcribe_models(&models_dir, &mut available_models)
        {
            warn!("Failed to discover custom models: {}", e);
        }

        // Auto-discover transcribe.cpp GGUF models already in the shared HF cache.
        Self::discover_hf_cache_models(&mut available_models);

        let manager = Self {
            app_handle: app_handle.clone(),
            models_dir,
            available_models: Mutex::new(available_models),
            cancellation_tokens: Mutex::new(HashMap::new()),
        };

        // Migrate any bundled models to user directory
        manager.migrate_bundled_models()?;

        // Migrate GigaAM from its old single-file format to the new directory layout
        manager.migrate_gigaam_to_directory()?;

        // Check which models are already downloaded
        manager.update_download_status()?;

        // Auto-select a model if none is currently selected
        manager.auto_select_model_if_needed()?;

        Ok(manager)
    }

    pub fn get_available_models(&self) -> Vec<ModelInfo> {
        let models = self.available_models.lock().unwrap();
        models.values().cloned().collect()
    }

    pub fn get_model_info(&self, model_id: &str) -> Option<ModelInfo> {
        let models = self.available_models.lock().unwrap();
        models.get(model_id).cloned()
    }

    pub fn set_runtime_capabilities(
        &self,
        model_id: &str,
        supports_streaming: bool,
        supports_translation: bool,
        supports_language_detection: bool,
        supported_languages: Vec<String>,
    ) {
        let supported_languages = canonicalize_supported_languages(supported_languages);
        let mut models = self.available_models.lock().unwrap();
        if let Some(model) = models.get_mut(model_id) {
            model.supports_streaming = supports_streaming;
            model.supports_translation = supports_translation;
            model.supports_language_detection = supports_language_detection;
            if !supported_languages.is_empty() {
                model.supported_languages = supported_languages;
            }
        }
    }

    pub fn rescan_local_models(&self) -> Result<()> {
        let mut snapshot = self.available_models.lock().unwrap().clone();
        if let Err(e) = Self::discover_custom_transcribe_models(&self.models_dir, &mut snapshot) {
            warn!("Rescan: failed to discover custom models: {}", e);
        }
        Self::discover_hf_cache_models(&mut snapshot);

        let mut added = 0usize;
        {
            let mut live = self.available_models.lock().unwrap();
            for (id, info) in snapshot {
                if let std::collections::hash_map::Entry::Vacant(entry) = live.entry(id) {
                    entry.insert(info);
                    added += 1;
                }
            }
        }

        self.update_download_status()?;
        self.auto_select_model_if_needed()?;
        if added > 0 {
            info!("Model rescan discovered {} new model(s)", added);
        }
        let _ = self.app_handle.emit("models-updated", ());
        Ok(())
    }

    fn seed_catalog_models(available_models: &mut HashMap<String, ModelInfo>) {
        for catalog_model in crate::catalog::CATALOG.iter() {
            let Some(file) = catalog_model.default_file() else {
                continue;
            };

            let model_id = format!("{}/{}", catalog_model.id, file.filename);
            if available_models.contains_key(&model_id) {
                continue;
            }

            let mut supported_languages = catalog_model.languages.clone();
            if supported_languages.iter().any(|language| language == "zh") {
                if !supported_languages
                    .iter()
                    .any(|language| language == "zh-Hans")
                {
                    supported_languages.push("zh-Hans".to_string());
                }
                if !supported_languages
                    .iter()
                    .any(|language| language == "zh-Hant")
                {
                    supported_languages.push("zh-Hant".to_string());
                }
            }

            available_models.insert(
                model_id.clone(),
                ModelInfo {
                    id: model_id,
                    name: catalog_model.name.clone(),
                    description: catalog_model.description.clone(),
                    filename: file.filename.clone(),
                    url: Some(hf_source_url(&catalog_model.id, "main", &file.filename)),
                    sha256: None,
                    size_mb: file.size_bytes / (1024 * 1024),
                    is_downloaded: false,
                    is_downloading: false,
                    partial_size: 0,
                    is_directory: false,
                    engine_type: EngineType::TranscribeCpp,
                    accuracy_score: catalog_model.accuracy_score.unwrap_or(0.0) / 100.0,
                    speed_score: catalog_model.speed_score.unwrap_or(0.0) / 100.0,
                    supports_translation: catalog_model.capabilities.translate,
                    supports_streaming: catalog_model.capabilities.streaming,
                    supports_language_detection: catalog_model.capabilities.lang_detect,
                    is_recommended: catalog_model.recommended,
                    supported_languages: canonicalize_supported_languages(supported_languages),
                    is_custom: false,
                },
            );
        }
    }

    fn clear_download_state(&self, model_id: &str, partial_path: &Path) {
        let partial_size = partial_path
            .metadata()
            .map(|metadata| metadata.len())
            .unwrap_or(0);

        {
            let mut models = self.available_models.lock().unwrap();
            if let Some(model) = models.get_mut(model_id) {
                model.is_downloading = false;
                model.partial_size = partial_size;
            }
        }

        self.cancellation_tokens.lock().unwrap().remove(model_id);
    }

    fn verify_sha256(path: &Path, expected_sha256: Option<&str>, model_id: &str) -> Result<()> {
        let Some(expected) = expected_sha256 else {
            return Ok(());
        };

        match Self::compute_sha256(path) {
            Ok(actual) if actual == expected => {
                info!("SHA256 verified for model {}", model_id);
                Ok(())
            }
            Ok(actual) => {
                warn!(
                    "SHA256 mismatch for model {}: expected {}, got {}",
                    model_id, expected, actual
                );
                let _ = fs::remove_file(path);
                Err(anyhow::anyhow!(
                    "Download verification failed for model {}: file is corrupt. Please retry.",
                    model_id
                ))
            }
            Err(error) => {
                let _ = fs::remove_file(path);
                Err(anyhow::anyhow!(
                    "Failed to verify download for model {}: {}. Please retry.",
                    model_id,
                    error
                ))
            }
        }
    }

    fn compute_sha256(path: &Path) -> Result<String> {
        let mut file = File::open(path)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 65536];

        loop {
            let bytes_read = file.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        Ok(format!("{:x}", hasher.finalize()))
    }

    fn migrate_bundled_models(&self) -> Result<()> {
        // Check for bundled models and copy them to user directory
        let bundled_models = ["ggml-small.bin"]; // Add other bundled models here if any

        for filename in &bundled_models {
            let bundled_path = self.app_handle.path().resolve(
                &format!("resources/models/{}", filename),
                tauri::path::BaseDirectory::Resource,
            );

            if let Ok(bundled_path) = bundled_path {
                if bundled_path.exists() {
                    let user_path = self.models_dir.join(filename);

                    // Only copy if user doesn't already have the model
                    if !user_path.exists() {
                        info!("Migrating bundled model {} to user directory", filename);
                        fs::copy(&bundled_path, &user_path)?;
                        info!("Successfully migrated {}", filename);
                    }
                }
            }
        }

        Ok(())
    }

    /// Migrate GigaAM from the old single-file format (`giga-am-v3.int8.onnx`)
    /// to the new directory format expected by transcribe-rs 0.3.x
    /// (`giga-am-v3-int8/model.int8.onnx` + `vocab.txt`).
    fn migrate_gigaam_to_directory(&self) -> Result<()> {
        let old_file = self.models_dir.join("giga-am-v3.int8.onnx");
        let new_dir = self.models_dir.join("giga-am-v3-int8");

        if !old_file.exists() || new_dir.exists() {
            return Ok(());
        }

        info!("Migrating GigaAM from single-file to directory format");

        let vocab_path = self
            .app_handle
            .path()
            .resolve(
                "resources/models/gigaam_vocab.txt",
                tauri::path::BaseDirectory::Resource,
            )
            .map_err(|e| anyhow::anyhow!("Failed to resolve GigaAM vocab path: {}", e))?;

        fs::create_dir_all(&new_dir)?;
        fs::rename(&old_file, new_dir.join("model.int8.onnx"))?;
        fs::copy(&vocab_path, new_dir.join("vocab.txt"))?;

        let old_partial = self.models_dir.join("giga-am-v3.int8.onnx.partial");
        if old_partial.exists() {
            let _ = fs::remove_file(&old_partial);
        }

        info!("GigaAM migration complete");
        Ok(())
    }

    fn update_download_status(&self) -> Result<()> {
        let mut models = self.available_models.lock().unwrap();

        for model in models.values_mut() {
            if let Some((repo_id, revision, filename)) = model_hf_source(model) {
                model.is_downloaded = hf_cached_path(&repo_id, &revision, &filename).is_some();
                model.is_downloading = false;
                model.partial_size = 0;
                continue;
            }

            if model.is_directory {
                // For directory-based models, check if the directory exists
                let model_path = self.models_dir.join(&model.filename);
                let partial_path = self.models_dir.join(format!("{}.partial", &model.filename));
                let extracting_path = self
                    .models_dir
                    .join(format!("{}.extracting", &model.filename));

                // Clean up any leftover .extracting directories from interrupted extractions
                if extracting_path.exists() {
                    warn!("Cleaning up interrupted extraction for model: {}", model.id);
                    let _ = fs::remove_dir_all(&extracting_path);
                }

                model.is_downloaded = model_path.exists() && model_path.is_dir();
                model.is_downloading = false;

                // Get partial file size if it exists (for the .tar.gz being downloaded)
                if partial_path.exists() {
                    model.partial_size = partial_path.metadata().map(|m| m.len()).unwrap_or(0);
                } else {
                    model.partial_size = 0;
                }
            } else {
                // For file-based models (existing logic)
                let model_path = self.models_dir.join(&model.filename);
                let partial_path = self.models_dir.join(format!("{}.partial", &model.filename));

                model.is_downloaded = model_path.exists();
                model.is_downloading = false;

                // Get partial file size if it exists
                if partial_path.exists() {
                    model.partial_size = partial_path.metadata().map(|m| m.len()).unwrap_or(0);
                } else {
                    model.partial_size = 0;
                }
            }
        }

        Ok(())
    }

    fn auto_select_model_if_needed(&self) -> Result<()> {
        let mut settings = get_settings(&self.app_handle);

        // Clear stale selection when selected model no longer exists in available list.
        if !settings.selected_model.is_empty() {
            let models = self.available_models.lock().unwrap();
            let exists = models.contains_key(&settings.selected_model);
            drop(models);

            if !exists {
                info!(
                    "Selected model '{}' not found in available models, clearing selection",
                    settings.selected_model
                );
                settings.selected_model = String::new();
                write_settings(&self.app_handle, settings.clone());
            }
        }

        let models = self.available_models.lock().unwrap();
        let selected_model_available = if settings.selected_model.is_empty() {
            false
        } else {
            models
                .get(&settings.selected_model)
                .map(|model| model.is_downloaded)
                .unwrap_or(false)
        };

        // If no model is selected or selected model is unavailable, auto-select the first downloaded model.
        if settings.selected_model.is_empty() || !selected_model_available {
            // Find the first available (downloaded) model
            if let Some(available_model) = models.values().find(|model| model.is_downloaded) {
                info!(
                    "Auto-selecting model: {} ({})",
                    available_model.id, available_model.name
                );

                // Update settings with the selected model
                let mut updated_settings = settings;
                updated_settings.selected_model = available_model.id.clone();
                write_settings(&self.app_handle, updated_settings);

                info!("Successfully auto-selected model: {}", available_model.id);
            } else if !settings.selected_model.is_empty() {
                warn!(
                    "Selected model {} is unavailable and no downloaded models were found.",
                    settings.selected_model
                );
            }
        }

        Ok(())
    }

    /// Discover custom transcribe.cpp models (.bin / .gguf files) in the models directory.
    /// Skips files that match predefined model filenames.
    fn discover_custom_transcribe_models(
        models_dir: &Path,
        available_models: &mut HashMap<String, ModelInfo>,
    ) -> Result<()> {
        if !models_dir.exists() {
            return Ok(());
        }

        // Collect predefined file model names to avoid duplicate entries. The fork still
        // ships legacy Whisper `.bin` entries alongside transcribe.cpp catalog models.
        let predefined_filenames: HashSet<String> = available_models
            .values()
            .filter(|m| !m.is_directory)
            .map(|m| m.filename.clone())
            .collect();

        for entry in fs::read_dir(models_dir)? {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    warn!("Failed to read models directory entry: {}", e);
                    continue;
                }
            };

            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let filename = match path.file_name().and_then(|name| name.to_str()) {
                Some(name) => name.to_string(),
                None => continue,
            };

            // Skip hidden files.
            if filename.starts_with('.') {
                continue;
            }

            let (model_id, is_gguf) = if let Some(stem) = filename.strip_suffix(".bin") {
                (stem.to_string(), false)
            } else if let Some(stem) = filename.strip_suffix(".gguf") {
                (stem.to_string(), true)
            } else {
                continue;
            };

            if predefined_filenames.contains(&filename) {
                continue;
            }

            if available_models.contains_key(&model_id) {
                continue;
            }

            let display_name = model_id
                .replace(['-', '_'], " ")
                .split_whitespace()
                .map(|word| {
                    let mut chars = word.chars();
                    match chars.next() {
                        None => String::new(),
                        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");

            let size_mb = match path.metadata() {
                Ok(metadata) => metadata.len() / (1024 * 1024),
                Err(e) => {
                    warn!("Failed to read metadata for {}: {}", filename, e);
                    0
                }
            };

            let probe = if is_gguf {
                GgufHeaderProber.probe_file(&path)
            } else {
                CapabilityProbe::default()
            };
            let caps = local_caps(&probe);
            let display_name = probed_display_name(&probe).unwrap_or(display_name);

            info!(
                "Discovered custom transcribe.cpp model: {} ({}, {} MB, streaming={})",
                model_id, filename, size_mb, caps.supports_streaming
            );

            available_models.insert(
                model_id.clone(),
                ModelInfo {
                    id: model_id,
                    name: display_name,
                    description: "Not officially supported".to_string(),
                    filename,
                    url: None,
                    sha256: None,
                    size_mb,
                    is_downloaded: true,
                    is_downloading: false,
                    partial_size: 0,
                    is_directory: false,
                    engine_type: EngineType::TranscribeCpp,
                    accuracy_score: 0.0,
                    speed_score: 0.0,
                    supports_translation: caps.supports_translation,
                    supports_streaming: caps.supports_streaming,
                    supports_language_detection: caps.supports_language_detection,
                    is_recommended: false,
                    supported_languages: caps.supported_languages,
                    is_custom: true,
                },
            );
        }

        Ok(())
    }

    fn discover_hf_cache_models(available_models: &mut HashMap<String, ModelInfo>) {
        Self::discover_hf_cache_models_in(Cache::from_env().path(), available_models);
    }

    fn discover_hf_cache_models_in(
        cache_root: &Path,
        available_models: &mut HashMap<String, ModelInfo>,
    ) {
        if !cache_root.is_dir() {
            return;
        }

        let known_hf: HashSet<(String, String)> = available_models
            .values()
            .filter_map(model_hf_source)
            .map(|(repo_id, _revision, filename)| (repo_id, filename))
            .collect();
        let prober = GgufHeaderProber;

        let entries = match fs::read_dir(cache_root) {
            Ok(entries) => entries,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            let folder = entry.file_name();
            let folder = folder.to_string_lossy();
            let Some(rest) = folder.strip_prefix("models--") else {
                continue;
            };
            let repo_id = rest.replace("--", "/");

            let refs_dir = entry.path().join("refs");
            let Some(revision) = Self::pick_hf_revision(&refs_dir) else {
                continue;
            };
            let Ok(commit) = fs::read_to_string(refs_dir.join(&revision)) else {
                continue;
            };
            let snapshot = entry.path().join("snapshots").join(commit.trim());
            let Ok(files) = fs::read_dir(&snapshot) else {
                continue;
            };

            for file in files.flatten() {
                let fname = file.file_name().to_string_lossy().to_string();
                if !fname.ends_with(".gguf") {
                    continue;
                }
                if known_hf.contains(&(repo_id.clone(), fname.clone())) {
                    continue;
                }

                let model_id = format!("{}/{}", repo_id, fname);
                if available_models.contains_key(&model_id) {
                    continue;
                }

                let path = snapshot.join(&fname);
                let probe = prober.probe_file(&path);
                if probe.verdict != Compatibility::Compatible {
                    continue;
                }
                let caps = local_caps(&probe);
                let size_mb = path
                    .metadata()
                    .map(|metadata| metadata.len() / (1024 * 1024))
                    .unwrap_or(0);
                let display_name = probed_display_name(&probe)
                    .unwrap_or_else(|| fname.trim_end_matches(".gguf").to_string());

                info!("Discovered HF cache model: {} ({})", model_id, repo_id);
                available_models.insert(
                    model_id.clone(),
                    ModelInfo {
                        id: model_id,
                        name: display_name,
                        description: format!("From Hugging Face cache: {}", repo_id),
                        filename: fname.clone(),
                        url: Some(hf_source_url(&repo_id, &revision, &fname)),
                        sha256: None,
                        size_mb,
                        is_downloaded: true,
                        is_downloading: false,
                        partial_size: 0,
                        is_directory: false,
                        engine_type: EngineType::TranscribeCpp,
                        accuracy_score: 0.0,
                        speed_score: 0.0,
                        supports_translation: caps.supports_translation,
                        supports_streaming: caps.supports_streaming,
                        supports_language_detection: caps.supports_language_detection,
                        is_recommended: false,
                        supported_languages: caps.supported_languages,
                        is_custom: false,
                    },
                );
            }
        }
    }

    fn pick_hf_revision(refs_dir: &Path) -> Option<String> {
        if refs_dir.join("main").is_file() {
            return Some("main".to_string());
        }
        fs::read_dir(refs_dir).ok()?.flatten().find_map(|entry| {
            if entry.path().is_file() {
                entry.file_name().to_str().map(str::to_string)
            } else {
                None
            }
        })
    }

    async fn download_hf_model(
        &self,
        model_info: &ModelInfo,
        repo_id: String,
        revision: String,
        filename: String,
    ) -> Result<()> {
        if hf_cached_path(&repo_id, &revision, &filename).is_some() {
            self.update_download_status()?;
            let _ = self
                .app_handle
                .emit("model-download-complete", &model_info.id);
            return Ok(());
        }

        {
            let mut models = self.available_models.lock().unwrap();
            if let Some(model) = models.get_mut(&model_info.id) {
                model.is_downloading = true;
                model.partial_size = 0;
            }
        }

        let cancel_token = CancellationToken::new();
        self.cancellation_tokens
            .lock()
            .unwrap()
            .insert(model_info.id.clone(), cancel_token.clone());

        // `true` means the download completed; `false` is a user cancellation.
        // Keeping cancellation distinct lets cleanup run without turning it
        // into a model-download-failed event in the command wrapper.
        let result: Result<bool> = async {
            let _ = self.app_handle.emit(
                "model-download-progress",
                &DownloadProgress {
                    model_id: model_info.id.clone(),
                    downloaded: 0,
                    total: model_info.size_mb.saturating_mul(1024 * 1024),
                    percentage: 0.0,
                },
            );

            let api = ApiBuilder::new().build()?;
            let repo = api.repo(Repo::with_revision(repo_id, RepoType::Model, revision));
            let progress = HfDownloadProgress::new(
                self.app_handle.clone(),
                model_info.id.clone(),
                model_info.size_mb.saturating_mul(1024 * 1024),
            );
            match repo
                .download_with_progress_cancellable(&filename, progress, cancel_token)
                .await
            {
                Ok(_) => {}
                Err(ApiError::Cancelled) => {
                    // hf-hub has stopped and joined every chunk task. Its
                    // `.sync.part` cache file stays in place for resume, and
                    // cancel_download already emitted the cancellation event.
                    info!("HF download cancelled for: {}", model_info.id);
                    return Ok(false);
                }
                Err(error) => {
                    return Err(anyhow::anyhow!("Hugging Face download failed: {}", error));
                }
            }

            self.update_download_status()?;
            let _ = self
                .app_handle
                .emit("model-download-complete", &model_info.id);
            info!("Successfully downloaded HF model {}", model_info.id);
            Ok(true)
        }
        .await;

        let completed = matches!(&result, Ok(true));

        self.cancellation_tokens
            .lock()
            .unwrap()
            .remove(&model_info.id);

        if !completed {
            let mut models = self.available_models.lock().unwrap();
            if let Some(model) = models.get_mut(&model_info.id) {
                model.is_downloading = false;
                model.partial_size = 0;
            }
        }

        result.map(|_| ())
    }

    pub async fn download_model(&self, model_id: &str) -> Result<()> {
        let model_info = {
            let models = self.available_models.lock().unwrap();
            models.get(model_id).cloned()
        };

        let model_info =
            model_info.ok_or_else(|| anyhow::anyhow!("Model not found: {}", model_id))?;

        if let Some((repo_id, revision, filename)) = model_hf_source(&model_info) {
            return self
                .download_hf_model(&model_info, repo_id, revision, filename)
                .await;
        }

        let url = model_info
            .url
            .ok_or_else(|| anyhow::anyhow!("No download URL for model"))?;
        let model_path = self.models_dir.join(&model_info.filename);
        let partial_path = self
            .models_dir
            .join(format!("{}.partial", &model_info.filename));

        if model_path.exists() {
            if partial_path.exists() {
                let _ = fs::remove_file(&partial_path);
            }
            self.update_download_status()?;
            return Ok(());
        }

        let cancel_token = CancellationToken::new();
        {
            let mut tokens = self.cancellation_tokens.lock().unwrap();
            tokens.insert(model_id.to_string(), cancel_token.clone());
        }
        {
            let mut models = self.available_models.lock().unwrap();
            if let Some(model) = models.get_mut(model_id) {
                model.is_downloading = true;
            }
        }

        let result: Result<()> = async {
            let mut resume_from = if partial_path.exists() {
                let size = partial_path.metadata()?.len();
                info!("Resuming download of model {} from byte {}", model_id, size);
                size
            } else {
                info!("Starting fresh download of model {} from {}", model_id, url);
                0
            };

            let client = reqwest::Client::new();
            let mut request = client.get(&url);

            if resume_from > 0 {
                request = request.header("Range", format!("bytes={}-", resume_from));
            }

            let mut response = request.send().await?;

            if resume_from > 0 && response.status() == reqwest::StatusCode::OK {
                warn!(
                    "Server doesn't support range requests for model {}, restarting download",
                    model_id
                );
                drop(response);
                let _ = fs::remove_file(&partial_path);
                resume_from = 0;
                response = client.get(&url).send().await?;
            }

            if !response.status().is_success()
                && response.status() != reqwest::StatusCode::PARTIAL_CONTENT
            {
                return Err(anyhow::anyhow!(
                    "Failed to download model: HTTP {}",
                    response.status()
                ));
            }

            let total_size = if resume_from > 0 {
                resume_from + response.content_length().unwrap_or(0)
            } else {
                response.content_length().unwrap_or(0)
            };

            let mut downloaded = resume_from;
            let mut stream = response.bytes_stream();
            let mut file = if resume_from > 0 {
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&partial_path)?
            } else {
                std::fs::File::create(&partial_path)?
            };

            let initial_progress = DownloadProgress {
                model_id: model_id.to_string(),
                downloaded,
                total: total_size,
                percentage: if total_size > 0 {
                    (downloaded as f64 / total_size as f64) * 100.0
                } else {
                    0.0
                },
            };
            let _ = self
                .app_handle
                .emit("model-download-progress", &initial_progress);

            loop {
                tokio::select! {
                    _ = cancel_token.cancelled() => {
                        info!("Download cancelled for model: {}", model_id);
                        drop(file);
                        self.clear_download_state(model_id, &partial_path);
                        let _ = self.app_handle.emit("model-download-cancelled", model_id);
                        return Ok(());
                    }
                    chunk_result = stream.next() => {
                        match chunk_result {
                            Some(Ok(chunk)) => {
                                file.write_all(&chunk)?;
                                downloaded += chunk.len() as u64;

                                let percentage = if total_size > 0 {
                                    (downloaded as f64 / total_size as f64) * 100.0
                                } else {
                                    0.0
                                };

                                let progress = DownloadProgress {
                                    model_id: model_id.to_string(),
                                    downloaded,
                                    total: total_size,
                                    percentage,
                                };
                                let _ = self.app_handle.emit("model-download-progress", &progress);
                            }
                            Some(Err(error)) => return Err(error.into()),
                            None => break,
                        }
                    }
                }
            }

            file.flush()?;
            drop(file);

            if total_size > 0 {
                let actual_size = partial_path.metadata()?.len();
                if actual_size != total_size {
                    let _ = fs::remove_file(&partial_path);
                    return Err(anyhow::anyhow!(
                        "Download incomplete: expected {} bytes, got {} bytes",
                        total_size,
                        actual_size
                    ));
                }
            }

            let _ = self.app_handle.emit("model-verification-started", model_id);
            info!("Verifying SHA256 for model {}...", model_id);
            let verify_path = partial_path.clone();
            let verify_expected = model_info.sha256.clone();
            let verify_model_id = model_id.to_string();
            let verify_result = tokio::task::spawn_blocking(move || {
                Self::verify_sha256(&verify_path, verify_expected.as_deref(), &verify_model_id)
            })
            .await
            .map_err(|error| anyhow::anyhow!("SHA256 task panicked: {}", error))?;
            verify_result?;
            let _ = self
                .app_handle
                .emit("model-verification-completed", model_id);

            if model_info.is_directory {
                let _ = self.app_handle.emit("model-extraction-started", model_id);
                info!("Extracting archive for directory-based model: {}", model_id);

                let temp_extract_dir = self
                    .models_dir
                    .join(format!("{}.extracting", &model_info.filename));
                let final_model_dir = self.models_dir.join(&model_info.filename);

                if temp_extract_dir.exists() {
                    let _ = fs::remove_dir_all(&temp_extract_dir);
                }
                fs::create_dir_all(&temp_extract_dir)?;

                let extraction_result: Result<()> = (|| {
                    let tar_gz = File::open(&partial_path)?;
                    let tar = GzDecoder::new(tar_gz);
                    let mut archive = Archive::new(tar);
                    archive.unpack(&temp_extract_dir)?;

                    let extracted_dirs: Vec<_> = fs::read_dir(&temp_extract_dir)?
                        .filter_map(|entry| entry.ok())
                        .filter(|entry| entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
                        .collect();

                    if extracted_dirs.len() == 1 {
                        let source_dir = extracted_dirs[0].path();
                        if final_model_dir.exists() {
                            fs::remove_dir_all(&final_model_dir)?;
                        }
                        fs::rename(&source_dir, &final_model_dir)?;
                        let _ = fs::remove_dir_all(&temp_extract_dir);
                    } else {
                        if final_model_dir.exists() {
                            fs::remove_dir_all(&final_model_dir)?;
                        }
                        fs::rename(&temp_extract_dir, &final_model_dir)?;
                    }

                    Ok(())
                })();

                if let Err(error) = extraction_result {
                    let error_msg = format!("Failed to extract archive: {}", error);
                    let _ = fs::remove_dir_all(&temp_extract_dir);
                    let _ = fs::remove_file(&partial_path);
                    let _ = self.app_handle.emit(
                        "model-extraction-failed",
                        &serde_json::json!({
                            "model_id": model_id,
                            "error": error_msg
                        }),
                    );
                    return Err(anyhow::anyhow!(error_msg));
                }

                info!("Successfully extracted archive for model: {}", model_id);
                let _ = self.app_handle.emit("model-extraction-completed", model_id);
                let _ = fs::remove_file(&partial_path);
            } else {
                fs::rename(&partial_path, &model_path)?;
            }

            {
                let mut models = self.available_models.lock().unwrap();
                if let Some(model) = models.get_mut(model_id) {
                    model.is_downloading = false;
                    model.is_downloaded = true;
                    model.partial_size = 0;
                }
            }
            self.cancellation_tokens.lock().unwrap().remove(model_id);

            let _ = self.app_handle.emit("model-download-complete", model_id);

            info!(
                "Successfully downloaded model {} to {:?}",
                model_id, model_path
            );

            Ok(())
        }
        .await;

        if result.is_err() {
            self.clear_download_state(model_id, &partial_path);
        }

        result
    }

    pub fn delete_model(&self, model_id: &str) -> Result<()> {
        debug!("ModelManager: delete_model called for: {}", model_id);

        let model_info = {
            let models = self.available_models.lock().unwrap();
            models.get(model_id).cloned()
        };

        let model_info =
            model_info.ok_or_else(|| anyhow::anyhow!("Model not found: {}", model_id))?;

        debug!("ModelManager: Found model info: {:?}", model_info);

        if let Some((repo_id, revision, filename)) = model_hf_source(&model_info) {
            let mut deleted_something = false;
            if let Some(model_path) = hf_cached_path(&repo_id, &revision, &filename) {
                if let Some(repo_dir) = model_path.ancestors().nth(3) {
                    if repo_dir.exists() {
                        info!("Deleting HF cached model repo at: {:?}", repo_dir);
                        fs::remove_dir_all(repo_dir)?;
                        deleted_something = true;
                    }
                }
                if !deleted_something && model_path.exists() {
                    info!("Deleting HF cached model file at: {:?}", model_path);
                    fs::remove_file(model_path)?;
                    deleted_something = true;
                }
            }

            if !deleted_something {
                return Err(anyhow::anyhow!("No model files found to delete"));
            }

            self.update_download_status()?;
            let _ = self.app_handle.emit("model-deleted", model_id);
            return Ok(());
        }

        let model_path = self.models_dir.join(&model_info.filename);
        let partial_path = self
            .models_dir
            .join(format!("{}.partial", &model_info.filename));
        debug!("ModelManager: Model path: {:?}", model_path);
        debug!("ModelManager: Partial path: {:?}", partial_path);

        let mut deleted_something = false;

        if model_info.is_directory {
            // Delete complete model directory if it exists
            if model_path.exists() && model_path.is_dir() {
                info!("Deleting model directory at: {:?}", model_path);
                fs::remove_dir_all(&model_path)?;
                info!("Model directory deleted successfully");
                deleted_something = true;
            }
        } else {
            // Delete complete model file if it exists
            if model_path.exists() {
                info!("Deleting model file at: {:?}", model_path);
                fs::remove_file(&model_path)?;
                info!("Model file deleted successfully");
                deleted_something = true;
            }
        }

        // Delete partial file if it exists (same for both types)
        if partial_path.exists() {
            info!("Deleting partial file at: {:?}", partial_path);
            fs::remove_file(&partial_path)?;
            info!("Partial file deleted successfully");
            deleted_something = true;
        }

        if !deleted_something {
            return Err(anyhow::anyhow!("No model files found to delete"));
        }

        // Custom models have no download URL and should disappear after deletion.
        if model_info.is_custom {
            let mut models = self.available_models.lock().unwrap();
            models.remove(model_id);
            debug!("ModelManager: removed custom model from available models");
        } else {
            self.update_download_status()?;
            debug!("ModelManager: download status updated");
        }

        // Notify UI/state stores that a model was deleted
        let _ = self.app_handle.emit("model-deleted", model_id);

        Ok(())
    }

    pub fn get_model_path(&self, model_id: &str) -> Result<PathBuf> {
        let model_info = self
            .get_model_info(model_id)
            .ok_or_else(|| anyhow::anyhow!("Model not found: {}", model_id))?;

        if !model_info.is_downloaded {
            return Err(anyhow::anyhow!("Model not available: {}", model_id));
        }

        // Ensure we don't return partial files/directories
        if model_info.is_downloading {
            return Err(anyhow::anyhow!(
                "Model is currently downloading: {}",
                model_id
            ));
        }

        if let Some((repo_id, revision, filename)) = model_hf_source(&model_info) {
            return hf_cached_path(&repo_id, &revision, &filename).ok_or_else(|| {
                anyhow::anyhow!("Complete model file not found in HF cache: {}", model_id)
            });
        }

        let model_path = self.models_dir.join(&model_info.filename);
        let partial_path = self
            .models_dir
            .join(format!("{}.partial", &model_info.filename));

        if model_info.is_directory {
            // For directory-based models, ensure the directory exists and is complete
            if model_path.exists() && model_path.is_dir() && !partial_path.exists() {
                Ok(model_path)
            } else {
                Err(anyhow::anyhow!(
                    "Complete model directory not found: {}",
                    model_id
                ))
            }
        } else {
            // For file-based models (existing logic)
            if model_path.exists() && !partial_path.exists() {
                Ok(model_path)
            } else {
                Err(anyhow::anyhow!(
                    "Complete model file not found: {}",
                    model_id
                ))
            }
        }
    }

    pub fn cancel_download(&self, model_id: &str) -> Result<()> {
        debug!("ModelManager: cancel_download called for: {}", model_id);

        let model_info = {
            let models = self.available_models.lock().unwrap();
            models.get(model_id).cloned()
        };

        let model_info =
            model_info.ok_or_else(|| anyhow::anyhow!("Model not found: {}", model_id))?;

        // Cancel the download task via cancellation token.
        let cancellation_sent = {
            let tokens = self.cancellation_tokens.lock().unwrap();
            if let Some(token) = tokens.get(model_id) {
                token.cancel();
                info!("Cancellation signal sent for model: {}", model_id);
                true
            } else {
                debug!("No active download found for model: {}", model_id);
                false
            }
        };

        // Mark as not downloading
        {
            let mut models = self.available_models.lock().unwrap();
            if let Some(model) = models.get_mut(model_id) {
                model.is_downloading = false;
                model.partial_size = 0;
            }
        }

        // Delete the partial file so the model returns to "downloadable" state
        let partial_path = self
            .models_dir
            .join(format!("{}.partial", &model_info.filename));
        if partial_path.exists() {
            if let Err(e) = fs::remove_file(&partial_path) {
                warn!("Failed to delete partial file {:?}: {}", partial_path, e);
            } else {
                info!(
                    "Deleted partial file for cancelled download: {:?}",
                    partial_path
                );
            }
        }

        // Update download status to reflect current state
        self.update_download_status()?;

        // Direct-URL downloads emit this from their existing stream loop.
        // HF downloads return ApiError::Cancelled instead, so emit exactly
        // once here and let the async path treat it as successful cancellation.
        if cancellation_sent && model_hf_source(&model_info).is_some() {
            let _ = self.app_handle.emit("model-download-cancelled", model_id);
        }

        info!("Download cancelled for: {}", model_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{effective_language, ModelManager};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_file_path(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("aivorelay-{name}-{unique}.partial"))
    }

    #[test]
    fn effective_language_falls_back_when_auto_detect_is_missing() {
        let languages = vec!["de".to_string(), "en".to_string()];

        assert_eq!(effective_language("auto", &languages, false), "en");
    }

    #[test]
    fn effective_language_preserves_chinese_script_intent() {
        let languages = vec!["zh".to_string()];

        assert_eq!(effective_language("zh-Hans", &languages, false), "zh-Hans");
        assert_eq!(effective_language("zh-Hant", &languages, false), "zh-Hant");
    }

    #[test]
    fn verify_sha256_skips_when_hash_is_missing() {
        let path = temp_file_path("skip");
        fs::write(&path, b"hello").unwrap();

        assert!(ModelManager::verify_sha256(&path, None, "custom").is_ok());
        assert!(path.exists());

        let _ = fs::remove_file(path);
    }

    #[test]
    fn verify_sha256_accepts_matching_hash() {
        let path = temp_file_path("match");
        fs::write(&path, b"hello").unwrap();
        let hash = ModelManager::compute_sha256(&path).unwrap();

        assert!(ModelManager::verify_sha256(&path, Some(&hash), "test").is_ok());
        assert!(path.exists());

        let _ = fs::remove_file(path);
    }

    #[test]
    fn verify_sha256_deletes_corrupt_partial() {
        let path = temp_file_path("mismatch");
        fs::write(&path, b"hello").unwrap();

        let result = ModelManager::verify_sha256(
            &path,
            Some("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"),
            "bad-model",
        );

        assert!(result.is_err());
        assert!(!path.exists());
    }

    #[test]
    fn verify_sha256_fails_when_file_is_missing() {
        let path = temp_file_path("missing");

        let result = ModelManager::verify_sha256(&path, Some("deadbeef"), "missing-model");

        assert!(result.is_err());
    }

    #[test]
    fn compute_sha256_matches_known_hash_for_empty_file() {
        let path = temp_file_path("empty");
        fs::write(&path, b"").unwrap();

        let hash = ModelManager::compute_sha256(&path).unwrap();

        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn compute_sha256_matches_known_hash_for_hello_file() {
        let path = temp_file_path("hello-known");
        fs::write(&path, b"hello").unwrap();

        let hash = ModelManager::compute_sha256(&path).unwrap();

        assert_eq!(
            hash,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn compute_sha256_fails_for_directory_paths() {
        let path = temp_file_path("dir");
        fs::create_dir_all(&path).unwrap();

        let result = ModelManager::compute_sha256(&path);

        assert!(result.is_err());

        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn verify_sha256_mismatch_error_mentions_model_id_and_retry() {
        let path = temp_file_path("mismatch-message");
        fs::write(&path, b"hello").unwrap();

        let error = ModelManager::verify_sha256(
            &path,
            Some("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"),
            "voice-model",
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("voice-model"));
        assert!(error.contains("Please retry"));
        assert!(!path.exists());
    }

    #[test]
    fn verify_sha256_missing_file_error_mentions_model_id_and_retry() {
        let path = temp_file_path("missing-message");

        let error = ModelManager::verify_sha256(&path, Some("deadbeef"), "missing-voice-model")
            .unwrap_err()
            .to_string();

        assert!(error.contains("missing-voice-model"));
        assert!(error.contains("Please retry"));
    }
}
