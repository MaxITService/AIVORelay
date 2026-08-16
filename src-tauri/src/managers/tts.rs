//! Provider-independent text-to-speech synthesis and file conversion.
//!
//! Provider responses are requested as raw signed 16-bit little-endian mono
//! PCM at 24 kHz. Interactive cloud synthesis publishes bounded WAV cache
//! segments while the response arrives; local/system providers publish each
//! completed semantic chunk. File conversion still assembles PCM first and
//! encodes exactly one final WAV or MP3 stream.

use crate::managers::edge_tts::{self, DEFAULT_EDGE_TTS_VOICE, EDGE_TTS_PROVIDER_LIMIT};
use crate::managers::local_kokoro::{
    KokoroTtsRuntime, KOKORO_LANGUAGES, KOKORO_PROVIDER_LIMIT, KOKORO_VOICES,
};
use crate::managers::local_tts::{
    LocalTtsKind, LocalTtsRuntime, LocalTtsStatus, LOCAL_TTS_LANGUAGES, LOCAL_TTS_PROVIDER_LIMIT,
    LOCAL_TTS_VOICES,
};
use crate::managers::provider_error::{parse_provider_error, safe_text};
use crate::managers::tts_history::{
    metadata_from_settings, TtsHistoryManager, TtsHistoryScope, TtsHistorySourceKind,
};
use crate::managers::tts_llm::{self, TtsLlmProgress};
use crate::managers::tts_resume::{self, ResumeOrigin, ResumeWorkspace, WatcherResumeTask};
use crate::managers::windows_tts::{self, WINDOWS_TTS_PROVIDER_LIMIT};
use crate::settings::{
    apply_text_replacements, ElevenLabsTextNormalization, TtsKeySource, TtsLlmScope,
    TtsOutputFormat, TtsProvider, TtsSettings, DEFAULT_TTS_CARTESIA_VOICE,
    DEFAULT_TTS_ELEVENLABS_MODEL, DEFAULT_TTS_ELEVENLABS_VOICE, DEFAULT_TTS_MURF_GEN2_VOICE,
    DEFAULT_TTS_MURF_VOICE, DEFAULT_TTS_OPENAI_COMPATIBLE_MODEL,
    DEFAULT_TTS_OPENAI_COMPATIBLE_VOICE, DEFAULT_TTS_OPENAI_VOICE, DEFAULT_TTS_SONIOX_VOICE,
};

#[allow(dead_code)]
pub fn normalize_openai_compatible_base_url(raw: &str) -> String {
    normalize_openai_compatible_base_url_with_insecure_option(raw, false)
        .unwrap_or_else(|_| raw.trim().trim_end_matches('/').to_string())
}

pub fn normalize_openai_compatible_base_url_with_insecure_option(
    raw: &str,
    allow_insecure_http: bool,
) -> std::result::Result<String, String> {
    crate::url_security::validate_openai_compatible_tts_base_url(raw, allow_insecure_http)
}
use anyhow::{anyhow, Context, Result};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use futures_util::StreamExt;
use mp3lame_encoder::{Bitrate, Builder as LameBuilder, FlushGap, Mode, MonoPcm, Quality, VbrMode};
use parking_lot::RwLock;
use reqwest::{header::RETRY_AFTER, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use specta::Type;
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, Read, Write};
use std::num::NonZeroU32;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

pub const TTS_EVENT_STATE: &str = "tts://state";
pub const TTS_EVENT_CHUNK_READY: &str = "tts://chunk-ready";
pub const TTS_EVENT_PROGRESS: &str = "tts://progress";
pub const TTS_EVENT_BATCH_PROGRESS: &str = "tts://batch-progress";

pub const SONIOX_CHARACTER_LIMIT: usize = 5_000;
pub const DEEPGRAM_CHARACTER_LIMIT: usize = 2_000;
pub const OPENAI_CHARACTER_LIMIT: usize = 4_096;
pub const MURF_CHARACTER_LIMIT: usize = 3_000;
pub const ELEVENLABS_V3_CHARACTER_LIMIT: usize = 5_000;
pub const ELEVENLABS_MULTILINGUAL_V2_CHARACTER_LIMIT: usize = 10_000;
pub const ELEVENLABS_FLASH_V2_5_CHARACTER_LIMIT: usize = 40_000;
pub const CARTESIA_CONSERVATIVE_CHARACTER_LIMIT: usize = 4_000;
pub const SONIOX_TTS_MODEL_MAX_CHARS: usize = 50;
pub const SONIOX_TTS_LANGUAGE_MAX_CHARS: usize = 50;
pub const SONIOX_TTS_VOICE_MAX_CHARS: usize = 50;
pub const SONIOX_TTS_API_KEY_MAX_CHARS: usize = 250;
pub const OPENAI_TTS_INSTRUCTIONS_MAX_CHARS: usize = 4_096;
pub const PROVIDER_PCM_SAMPLE_RATE: u32 = 24_000;
pub const MP3_OUTPUT_SAMPLE_RATE: u32 = 32_000;
pub const SUPPORTED_MP3_BITRATES: [u16; 6] = [64, 96, 128, 192, 256, 320];

const SONIOX_TTS_URL: &str = "https://tts-rt.soniox.com/tts";
const DEEPGRAM_TTS_URL: &str = "https://api.deepgram.com/v1/speak";
const OPENAI_TTS_URL: &str = "https://api.openai.com/v1/audio/speech";
const MURF_FALCON_TTS_URL: &str = "https://global.api.murf.ai/v1/speech/stream";
const MURF_GEN2_TTS_URL: &str = "https://api.murf.ai/v1/speech/generate";
const MURF_VOICES_URL: &str = "https://api.murf.ai/v1/speech/voices";
const ELEVENLABS_TTS_BASE_URL: &str = "https://api.elevenlabs.io/v1/text-to-speech/";
const ELEVENLABS_VOICES_URL: &str = "https://api.elevenlabs.io/v2/voices";
const CARTESIA_TTS_URL: &str = "https://api.cartesia.ai/tts/bytes";
const CARTESIA_VOICES_URL: &str = "https://api.cartesia.ai/voices";
const CARTESIA_API_VERSION: &str = "2026-03-01";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_RETRY_DELAY: Duration = Duration::from_secs(60);
const INTERACTIVE_CACHE_STALE_AGE: Duration = Duration::from_secs(24 * 60 * 60);
// Match the small-buffer Web Audio queues used by provider demos: emit enough
// PCM for 250 ms of playback instead of waiting for multi-second WAV parts.
// Cloud providers that return raw PCM share this provider-independent streaming path.
const INTERACTIVE_STREAM_SEGMENT_SAMPLES: usize =
    PROVIDER_PCM_SAMPLE_RATE as usize / 4;
const MAX_VOICE_CATALOG_BYTES: usize = 4 * 1024 * 1024;
const MAX_VOICE_CATALOG_PAGES: usize = 20;
const MAX_VOICE_CATALOG_ENTRIES: usize = 2_000;
const MAX_PROVIDER_ERROR_BODY_BYTES: usize = 64 * 1024;
const MAX_CLOUD_PCM_RESPONSE_BYTES: usize = 64 * 1024 * 1024;
const MAX_MURF_GEN2_PCM_BYTES: usize = 24 * 1024 * 1024;
const MAX_MURF_GEN2_JSON_BYTES: usize = 36 * 1024 * 1024;
// Long enough for multi-million-character book sources while bounding the
// additional copies created by Unicode chunking and preprocessing.
pub(crate) const MAX_TTS_TEXT_INPUT_BYTES: usize = 8 * 1024 * 1024;
// Processed narration may grow during cleanup, but remains bounded separately
// from the original source so chunking and resumable manifests stay finite.
pub(crate) const MAX_TTS_PROCESSED_TEXT_BYTES: usize = 16 * 1024 * 1024;
const MAX_TTS_BATCH_FILES: usize = 100_000;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TtsOperationKind {
    Interactive,
    FileConversion,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TtsPhase {
    Idle,
    Preparing,
    Preprocessing,
    Synthesizing,
    Retrying,
    Ready,
    Completed,
    Cancelled,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TtsBoundary {
    Paragraph,
    Sentence,
    Clause,
    Whitespace,
    Hard,
    End,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TtsChunk {
    /// One-based sequence number.
    pub index: usize,
    pub text: String,
    pub character_count: usize,
    pub boundary_after: TtsBoundary,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TtsState {
    pub operation_id: u64,
    pub kind: Option<TtsOperationKind>,
    pub phase: TtsPhase,
    pub provider: Option<TtsProvider>,
    pub completed_chunks: usize,
    pub total_chunks: usize,
    pub current_attempt: u8,
    pub message: Option<String>,
}

impl Default for TtsState {
    fn default() -> Self {
        Self {
            operation_id: 0,
            kind: None,
            phase: TtsPhase::Idle,
            provider: None,
            completed_chunks: 0,
            total_chunks: 0,
            current_attempt: 0,
            message: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TtsChunkReady {
    pub operation_id: u64,
    pub chunk_index: usize,
    pub total_chunks: usize,
    pub wav_path: PathBuf,
    pub boundary_after: TtsBoundary,
    pub pause_after_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TtsVoiceCatalogEntry {
    pub id: String,
    pub label: String,
    pub group: String,
    pub language: String,
    pub gender: String,
    pub description: String,
    #[serde(default)]
    pub locales: Vec<TtsVoiceCatalogLocale>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TtsVoiceCatalogLocale {
    pub locale: String,
    pub label: String,
    pub styles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TtsVoiceCatalog {
    pub provider: TtsProvider,
    pub voices: Vec<TtsVoiceCatalogEntry>,
    pub source: String,
    pub supports_live_refresh: bool,
    pub replace_builtin: bool,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TtsProgress {
    pub operation_id: u64,
    pub completed_chunks: usize,
    pub total_chunks: usize,
    pub current_chunk: usize,
    pub attempt: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct InteractiveSynthesis {
    pub operation_id: u64,
    pub processed_character_count: usize,
    pub chunks: Vec<TtsChunkReady>,
    /// Complete encoded result assembled only when opt-in TTS History is
    /// enabled. Operation directories are retained long enough for the caller
    /// to copy this path into managed history storage.
    pub combined_audio_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TextFileInspection {
    pub path: PathBuf,
    pub source_character_count: usize,
    pub processed_character_count: usize,
    pub chunk_count: usize,
    pub llm_cleanup_enabled: bool,
    pub llm_cleanup_chunk_count: usize,
    pub counts_are_estimates: bool,
    pub encoding: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FileConversionResult {
    pub operation_id: u64,
    pub output_path: PathBuf,
    pub source_character_count: usize,
    pub processed_character_count: usize,
    pub chunk_count: usize,
    pub resumed_chunks: usize,
    pub output_format: TtsOutputFormat,
    pub mp3_bitrate_kbps: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TtsBatchScanRequest {
    pub input_directory: Option<PathBuf>,
    #[serde(default)]
    pub input_paths: Vec<PathBuf>,
    pub output_directory: PathBuf,
    pub recursive: bool,
    pub output_format: TtsOutputFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TtsBatchFilePlan {
    pub input_path: PathBuf,
    pub relative_path: PathBuf,
    pub output_path: PathBuf,
    pub scan_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TtsBatchScanResult {
    pub input_directory: Option<PathBuf>,
    pub output_directory: PathBuf,
    pub recursive: bool,
    pub output_format: TtsOutputFormat,
    pub files: Vec<TtsBatchFilePlan>,
    pub eligible_count: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
struct TtsOutputCollisionError {
    message: String,
}

impl std::fmt::Display for TtsOutputCollisionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for TtsOutputCollisionError {}

fn output_already_exists_error(path: &Path) -> anyhow::Error {
    anyhow::Error::new(TtsOutputCollisionError {
        message: format!("Output file already exists: {}", path.display()),
    })
}

fn output_appeared_while_waiting_error(path: &Path) -> anyhow::Error {
    anyhow::Error::new(TtsOutputCollisionError {
        message: format!(
            "Output file appeared while waiting for the TTS resume workspace: {}",
            path.display()
        ),
    })
}

pub(crate) fn is_tts_output_collision(error: &anyhow::Error) -> bool {
    // Provider error text is untrusted and must never decide Skip vs Failed.
    error.downcast_ref::<TtsOutputCollisionError>().is_some()
}

pub(crate) struct TtsBatchCancellation {
    cancelled: AtomicBool,
    current_operation_id: AtomicU64,
}

impl TtsBatchCancellation {
    fn new() -> Self {
        Self {
            cancelled: AtomicBool::new(false),
            current_operation_id: AtomicU64::new(0),
        }
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    fn attach_operation<'a>(
        &'a self,
        manager: &TtsManager,
        operation_id: u64,
    ) -> TtsBatchOperationRegistration<'a> {
        self.current_operation_id
            .store(operation_id, Ordering::SeqCst);
        if self.is_cancelled() {
            manager.cancel_operation(operation_id);
        }
        TtsBatchOperationRegistration {
            cancellation: self,
            operation_id,
        }
    }

    fn cancel(&self, manager: &TtsManager) {
        self.cancelled.store(true, Ordering::SeqCst);
        let operation_id = self.current_operation_id.load(Ordering::SeqCst);
        if operation_id != 0 {
            manager.cancel_operation(operation_id);
        }
    }
}

struct TtsBatchOperationRegistration<'a> {
    cancellation: &'a TtsBatchCancellation,
    operation_id: u64,
}

impl Drop for TtsBatchOperationRegistration<'_> {
    fn drop(&mut self) {
        let _ = self.cancellation.current_operation_id.compare_exchange(
            self.operation_id,
            0,
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
    }
}

struct ActiveTtsBatch {
    id: u64,
    cancellation: Arc<TtsBatchCancellation>,
}

pub(crate) struct TtsBatchLease {
    manager: Arc<TtsManager>,
    id: u64,
    cancellation: Arc<TtsBatchCancellation>,
}

impl TtsBatchLease {
    pub(crate) fn id(&self) -> u64 {
        self.id
    }

    pub(crate) fn cancellation(&self) -> &Arc<TtsBatchCancellation> {
        &self.cancellation
    }
}

impl Drop for TtsBatchLease {
    fn drop(&mut self) {
        let mut active = self.manager.active_batch.lock();
        if active.as_ref().is_some_and(|batch| batch.id == self.id) {
            *active = None;
        }
    }
}

pub(crate) struct ResolvedTtsResult<T> {
    pub value: T,
    pub settings: TtsSettings,
}

struct ActiveUiJobRegistration<'a> {
    slot: &'a parking_lot::Mutex<Option<String>>,
    job_id: String,
}

impl<'a> ActiveUiJobRegistration<'a> {
    fn new(slot: &'a parking_lot::Mutex<Option<String>>, job_id: String) -> Self {
        *slot.lock() = Some(job_id.clone());
        Self { slot, job_id }
    }
}

impl Drop for ActiveUiJobRegistration<'_> {
    fn drop(&mut self) {
        let mut active = self.slot.lock();
        if active.as_deref() == Some(self.job_id.as_str()) {
            *active = None;
        }
    }
}

#[derive(Debug)]
struct ProviderAttemptError {
    status: Option<StatusCode>,
    safe_message: String,
    transient: bool,
    retry_after: Option<Duration>,
}

type PcmSegmentSink<'a> =
    dyn FnMut(&[i16]) -> std::result::Result<(), ProviderAttemptError> + Send + 'a;

impl std::fmt::Display for ProviderAttemptError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.safe_message)
    }
}

pub struct TtsManager {
    app_handle: AppHandle,
    client: reqwest::Client,
    active_operation_id: AtomicU64,
    state: RwLock<TtsState>,
    cache_root: PathBuf,
    folder_watcher: parking_lot::Mutex<Option<notify::RecommendedWatcher>>,
    watched_paths: Arc<parking_lot::Mutex<HashSet<PathBuf>>>,
    watcher_generation: AtomicU64,
    watched_conversion_lock: tokio::sync::Mutex<()>,
    foreground_operation_lock: Arc<tokio::sync::Mutex<()>>,
    active_batch: parking_lot::Mutex<Option<ActiveTtsBatch>>,
    next_batch_id: AtomicU64,
    ui_jobs_lock: parking_lot::Mutex<()>,
    active_ui_job: parking_lot::Mutex<Option<String>>,
    finalization_lock: parking_lot::Mutex<()>,
    local_tts: LocalTtsRuntime,
    local_kokoro: KokoroTtsRuntime,
}

impl TtsManager {
    /// Creates the manager as an `Arc`, matching the intended Tauri managed
    /// state (`app.manage(TtsManager::new(&app.handle())?)`).
    pub fn new(app_handle: &AppHandle) -> Result<Arc<Self>> {
        let client = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .context("Failed to build TTS HTTP client")?;
        let cache_root = app_handle
            .path()
            .app_cache_dir()
            .context("Could not resolve the application cache directory")?
            .join("tts");
        let local_tts = LocalTtsRuntime::new(app_handle)?;
        let local_kokoro = KokoroTtsRuntime::new(app_handle)?;

        Ok(Arc::new(Self {
            app_handle: app_handle.clone(),
            client,
            active_operation_id: AtomicU64::new(0),
            state: RwLock::new(TtsState::default()),
            cache_root,
            folder_watcher: parking_lot::Mutex::new(None),
            watched_paths: Arc::new(parking_lot::Mutex::new(HashSet::new())),
            watcher_generation: AtomicU64::new(0),
            watched_conversion_lock: tokio::sync::Mutex::new(()),
            foreground_operation_lock: Arc::new(tokio::sync::Mutex::new(())),
            active_batch: parking_lot::Mutex::new(None),
            next_batch_id: AtomicU64::new(0),
            ui_jobs_lock: parking_lot::Mutex::new(()),
            active_ui_job: parking_lot::Mutex::new(None),
            finalization_lock: parking_lot::Mutex::new(()),
            local_tts,
            local_kokoro,
        }))
    }

    pub fn provider_character_limit(provider: TtsProvider, model: &str) -> usize {
        match provider {
            TtsProvider::Soniox => SONIOX_CHARACTER_LIMIT,
            TtsProvider::Deepgram => DEEPGRAM_CHARACTER_LIMIT,
            TtsProvider::OpenAi | TtsProvider::OpenAiCompatible => OPENAI_CHARACTER_LIMIT,
            TtsProvider::Murf => MURF_CHARACTER_LIMIT,
            TtsProvider::ElevenLabs => match model.trim() {
                "eleven_flash_v2_5" => ELEVENLABS_FLASH_V2_5_CHARACTER_LIMIT,
                "eleven_multilingual_v2" => ELEVENLABS_MULTILINGUAL_V2_CHARACTER_LIMIT,
                "eleven_v3" => ELEVENLABS_V3_CHARACTER_LIMIT,
                _ => ELEVENLABS_V3_CHARACTER_LIMIT,
            },
            TtsProvider::Cartesia => CARTESIA_CONSERVATIVE_CHARACTER_LIMIT,
            TtsProvider::Edge => EDGE_TTS_PROVIDER_LIMIT,
            TtsProvider::LocalQwen => LOCAL_TTS_PROVIDER_LIMIT,
            TtsProvider::LocalKokoro => KOKORO_PROVIDER_LIMIT,
            TtsProvider::Windows => WINDOWS_TTS_PROVIDER_LIMIT,
        }
    }

    pub fn settings_character_limit(settings: &TtsSettings) -> usize {
        let model = match settings.provider {
            TtsProvider::Soniox => &settings.soniox_model,
            TtsProvider::Deepgram => &settings.deepgram_model,
            TtsProvider::OpenAi => &settings.openai_model,
            TtsProvider::OpenAiCompatible => &settings.openai_compatible_model,
            TtsProvider::Murf => &settings.murf_model,
            TtsProvider::ElevenLabs => &settings.elevenlabs_model,
            TtsProvider::Cartesia => &settings.cartesia_model,
            TtsProvider::Edge
            | TtsProvider::LocalQwen
            | TtsProvider::LocalKokoro
            | TtsProvider::Windows => "",
        };
        Self::provider_character_limit(settings.provider, model)
    }

    pub fn openai_model_supports_instructions(model: &str) -> bool {
        TtsProvider::OpenAi.supports_instructions(model)
    }

    pub fn validate_openai_instructions(instructions: &str) -> Result<()> {
        validate_max_chars(
            "OpenAI voice instructions",
            instructions,
            OPENAI_TTS_INSTRUCTIONS_MAX_CHARS,
        )
    }

    pub fn validate_synthesis_settings(settings: &TtsSettings) -> Result<()> {
        if settings.provider == TtsProvider::Soniox {
            validate_max_chars(
                "Soniox TTS model",
                &settings.soniox_model,
                SONIOX_TTS_MODEL_MAX_CHARS,
            )?;
            validate_max_chars(
                "Soniox TTS language",
                &settings.soniox_language,
                SONIOX_TTS_LANGUAGE_MAX_CHARS,
            )?;
            validate_max_chars(
                "Soniox TTS voice",
                &settings.soniox_voice,
                SONIOX_TTS_VOICE_MAX_CHARS,
            )?;
        }
        if settings.provider == TtsProvider::OpenAiCompatible {
            validate_required_max_chars(
                "OpenAI-compatible Base URL",
                &settings.openai_compatible_base_url,
                2048,
            )?;
            validate_required_max_chars(
                "OpenAI-compatible model",
                &settings.openai_compatible_model,
                256,
            )?;
            validate_required_max_chars(
                "OpenAI-compatible voice",
                &settings.openai_compatible_voice,
                256,
            )?;
        }
        if settings.provider == TtsProvider::Murf {
            if !matches!(settings.murf_model.as_str(), "falcon-2" | "gen2") {
                return Err(anyhow!("Unsupported Murf TTS model: {}", settings.murf_model));
            }
            validate_required_max_chars("Murf voice ID", &settings.murf_voice, 256)?;
            validate_required_max_chars("Murf locale", &settings.murf_language, 32)?;
            if !(-50..=50).contains(&settings.murf_rate) {
                return Err(anyhow!("Murf rate must be between -50 and 50"));
            }
            if !(-50..=50).contains(&settings.murf_pitch) {
                return Err(anyhow!("Murf pitch must be between -50 and 50"));
            }
            if settings.murf_variation > 5 {
                return Err(anyhow!("Murf variation must be between 0 and 5"));
            }
            if let Some(style) = settings.murf_style.as_deref() {
                validate_max_chars("Murf style", style, 128)?;
            }
            if settings.murf_key_source != TtsKeySource::Separate {
                return Err(anyhow!("Murf always uses its separately stored TTS key"));
            }
        }
        if settings.provider == TtsProvider::ElevenLabs {
            if !matches!(
                settings.elevenlabs_model.as_str(),
                "eleven_v3" | "eleven_multilingual_v2" | "eleven_flash_v2_5"
            ) {
                return Err(anyhow!(
                    "Unsupported ElevenLabs TTS model: {}",
                    settings.elevenlabs_model
                ));
            }
            validate_required_max_chars(
                "ElevenLabs voice ID",
                &settings.elevenlabs_voice,
                256,
            )?;
            validate_required_max_chars(
                "ElevenLabs language",
                &settings.elevenlabs_language,
                32,
            )?;
            validate_iso_639_1_code("ElevenLabs language", &settings.elevenlabs_language)?;
            if settings.elevenlabs_model == "eleven_v3" {
                if settings.speed != 1.0 {
                    return Err(anyhow!(
                        "ElevenLabs speed is unavailable for the Eleven v3 model"
                    ));
                }
            } else {
                validate_finite_range("ElevenLabs speed", settings.speed, 0.7, 1.2)?;
            }
            validate_finite_range(
                "ElevenLabs stability",
                settings.elevenlabs_stability,
                0.0,
                1.0,
            )?;
            validate_finite_range(
                "ElevenLabs similarity boost",
                settings.elevenlabs_similarity_boost,
                0.0,
                1.0,
            )?;
            validate_finite_range(
                "ElevenLabs style",
                settings.elevenlabs_style,
                0.0,
                1.0,
            )?;
            if settings.elevenlabs_key_source != TtsKeySource::Separate {
                return Err(anyhow!(
                    "ElevenLabs always uses its separately stored TTS key"
                ));
            }
        }
        if settings.provider == TtsProvider::Cartesia {
            if settings.cartesia_model != "sonic-3.5" {
                return Err(anyhow!(
                    "Unsupported Cartesia TTS model: {}",
                    settings.cartesia_model
                ));
            }
            validate_required_max_chars("Cartesia voice ID", &settings.cartesia_voice, 256)?;
            validate_required_max_chars("Cartesia language", &settings.cartesia_language, 32)?;
            validate_iso_639_1_code("Cartesia language", &settings.cartesia_language)?;
            validate_finite_range("Cartesia speed", settings.speed, 0.6, 1.5)?;
            validate_finite_range("Cartesia volume", settings.cartesia_volume, 0.5, 2.0)?;
            if let Some(emotion) = settings.cartesia_emotion.as_deref() {
                validate_max_chars("Cartesia emotion", emotion, 64)?;
            }
            if settings.cartesia_key_source != TtsKeySource::Separate {
                return Err(anyhow!(
                    "Cartesia always uses its separately stored TTS key"
                ));
            }
        }
        if settings.provider == TtsProvider::LocalQwen {
            if !LOCAL_TTS_VOICES.contains(&settings.local_qwen_voice.as_str()) {
                return Err(anyhow!(
                    "Unsupported local Qwen3-TTS voice: {}",
                    settings.local_qwen_voice
                ));
            }
            if !LOCAL_TTS_LANGUAGES.contains(&settings.local_qwen_language.as_str()) {
                return Err(anyhow!(
                    "Unsupported local Qwen3-TTS language: {}",
                    settings.local_qwen_language
                ));
            }
        }
        if settings.provider == TtsProvider::LocalKokoro {
            if !KOKORO_VOICES
                .iter()
                .any(|(voice, _)| *voice == settings.local_kokoro_voice)
            {
                return Err(anyhow!(
                    "Unsupported local Kokoro voice: {}",
                    settings.local_kokoro_voice
                ));
            }
            if !KOKORO_LANGUAGES.contains(&settings.local_kokoro_language.as_str()) {
                return Err(anyhow!(
                    "Unsupported local Kokoro language: {}",
                    settings.local_kokoro_language
                ));
            }
            let is_english_voice = KOKORO_VOICES
                .iter()
                .find_map(|(voice, sid)| {
                    (*voice == settings.local_kokoro_voice).then_some(*sid <= 2)
                })
                .unwrap_or(false);
            if (settings.local_kokoro_language == "English") != is_english_voice {
                return Err(anyhow!(
                    "Kokoro voice {} is not compatible with language {}",
                    settings.local_kokoro_voice,
                    settings.local_kokoro_language
                ));
            }
        }
        if settings.provider == TtsProvider::Windows {
            validate_max_chars("Windows voice ID", &settings.windows_voice_id, 1_024)?;
            validate_max_chars(
                "Windows voice language",
                &settings.windows_voice_language,
                128,
            )?;
        }
        if settings.provider == TtsProvider::Edge {
            validate_max_chars("Edge-TTS voice", &settings.edge_voice, 256)?;
            validate_max_chars(
                "Edge-TTS voice language",
                &settings.edge_voice_language,
                128,
            )?;
        }
        Self::validate_openai_instructions(&settings.openai_instructions)?;
        Ok(())
    }

    pub fn validate_settings(settings: &TtsSettings) -> Result<()> {
        Self::validate_synthesis_settings(settings)?;
        for preset in &settings.prompt_presets {
            validate_max_chars(
                &format!(
                    "OpenAI voice instructions in preset '{}'",
                    preset.name.trim()
                ),
                &preset.instructions,
                OPENAI_TTS_INSTRUCTIONS_MAX_CHARS,
            )?;
        }
        let llm = &settings.llm_preprocessing;
        validate_max_chars("TTS AI cleanup provider ID", &llm.provider_id, 128)?;
        validate_max_chars("TTS AI cleanup model", &llm.model, 512)?;
        validate_max_chars(
            "TTS AI cleanup custom base URL",
            &llm.custom_base_url,
            2_048,
        )?;
        for (scope, prompts, selected_id) in [
            (
                "interactive",
                llm.interactive_prompts.as_slice(),
                llm.interactive_selected_prompt_id.as_str(),
            ),
            (
                "file",
                llm.file_prompts.as_slice(),
                llm.file_selected_prompt_id.as_str(),
            ),
        ] {
            if prompts.len() > 100 {
                return Err(anyhow!(
                    "TTS AI cleanup {scope} prompts must not exceed 100 presets"
                ));
            }
            let mut prompt_ids = std::collections::HashSet::new();
            let mut prompt_names = std::collections::HashSet::new();
            for prompt in prompts {
                if prompt.id.trim().is_empty() {
                    return Err(anyhow!("TTS AI cleanup {scope} prompt ID cannot be empty"));
                }
                if prompt.name.trim().is_empty() {
                    return Err(anyhow!(
                        "TTS AI cleanup {scope} prompt name cannot be empty"
                    ));
                }
                if prompt.prompt.trim().is_empty() {
                    return Err(anyhow!(
                        "TTS AI cleanup prompt '{}' cannot be empty",
                        prompt.name.trim()
                    ));
                }
                if !prompt_ids.insert(prompt.id.trim().to_string()) {
                    return Err(anyhow!("TTS AI cleanup {scope} prompt IDs must be unique"));
                }
                if !prompt_names.insert(prompt.name.trim().to_lowercase()) {
                    return Err(anyhow!(
                        "TTS AI cleanup {scope} prompt names must be unique"
                    ));
                }
                validate_max_chars("TTS AI cleanup prompt ID", &prompt.id, 256)?;
                validate_max_chars("TTS AI cleanup prompt name", &prompt.name, 256)?;
                validate_max_chars(
                    &format!("TTS AI cleanup prompt '{}'", prompt.name.trim()),
                    &prompt.prompt,
                    32_768,
                )?;
            }
            if !prompts.iter().any(|prompt| prompt.id == selected_id) {
                return Err(anyhow!(
                    "The selected TTS AI cleanup {scope} prompt no longer exists"
                ));
            }
        }
        validate_max_chars(
            "TTS AI cleanup interactive benchmark text",
            &llm.interactive_benchmark_text,
            50_000,
        )?;
        validate_max_chars(
            "TTS AI cleanup file benchmark text",
            &llm.file_benchmark_text,
            50_000,
        )?;
        Ok(())
    }

    pub async fn voice_catalog(&self, settings: &TtsSettings) -> Result<TtsVoiceCatalog> {
        match settings.provider {
            TtsProvider::Edge => {
                let voices = edge_tts::list_voices(&self.client)
                    .await
                    .map_err(|error| anyhow!(error.safe_message))?
                    .into_iter()
                    .map(|voice| TtsVoiceCatalogEntry {
                        label: voice.id.clone(),
                        group: voice.language.clone(),
                        language: voice.language,
                        gender: voice.gender,
                        description: voice.description,
                        id: voice.id,
                        locales: Vec::new(),
                    })
                    .collect();
                Ok(TtsVoiceCatalog {
                    provider: settings.provider,
                    voices,
                    source: "live".to_string(),
                    supports_live_refresh: true,
                    replace_builtin: true,
                    warning: Some(
                        "Experimental community adapter for Microsoft Edge's online Read Aloud service; availability and protocol may change without notice."
                            .to_string(),
                    ),
                })
            }
            TtsProvider::Deepgram => {
                let api_key = resolve_api_key(settings)?;
                let response = self
                    .client
                    .get("https://api.deepgram.com/v1/models")
                    .header("Authorization", format!("Token {api_key}"))
                    .send()
                    .await
                    .map_err(|error| anyhow!(safe_text(&error.to_string())))?;
                let json = bounded_catalog_json(response).await?;
                let voices = json
                    .get("tts")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(deepgram_catalog_entry)
                    .collect::<Vec<_>>();
                if voices.is_empty() {
                    return Err(anyhow!("Deepgram returned no public TTS voices"));
                }
                Ok(TtsVoiceCatalog {
                    provider: settings.provider,
                    voices,
                    source: "live".to_string(),
                    supports_live_refresh: true,
                    replace_builtin: true,
                    warning: None,
                })
            }
            TtsProvider::Soniox => {
                let api_key = resolve_api_key(settings)?;
                let response = self
                    .client
                    .get("https://api.soniox.com/v1/voices")
                    .query(&[("limit", "1000")])
                    .bearer_auth(api_key)
                    .send()
                    .await
                    .map_err(|error| anyhow!(safe_text(&error.to_string())))?;
                let json = bounded_catalog_json(response).await?;
                let voices = json
                    .get("voices")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(soniox_catalog_entry)
                    .collect();
                Ok(TtsVoiceCatalog {
                    provider: settings.provider,
                    voices,
                    source: "live".to_string(),
                    supports_live_refresh: true,
                    replace_builtin: false,
                    warning: Some(
                        "Project voice refresh adds custom Soniox voices to the built-in catalog."
                            .to_string(),
                    ),
                })
            }
            TtsProvider::Murf => {
                let api_key = resolve_api_key(settings)?;
                let voices = murf_voice_catalog(
                    &self.client,
                    &api_key,
                    nonempty_or(&settings.murf_model, "falcon-2"),
                )
                .await?;
                Ok(TtsVoiceCatalog {
                    provider: settings.provider,
                    voices,
                    source: "live".to_string(),
                    supports_live_refresh: true,
                    replace_builtin: true,
                    warning: None,
                })
            }
            TtsProvider::ElevenLabs => {
                let api_key = resolve_api_key(settings)?;
                let voices = elevenlabs_voice_catalog(&self.client, &api_key).await?;
                Ok(TtsVoiceCatalog {
                    provider: settings.provider,
                    voices,
                    source: "live".to_string(),
                    supports_live_refresh: true,
                    replace_builtin: true,
                    warning: None,
                })
            }
            TtsProvider::Cartesia => {
                let api_key = resolve_api_key(settings)?;
                let voices = cartesia_voice_catalog(&self.client, &api_key).await?;
                Ok(TtsVoiceCatalog {
                    provider: settings.provider,
                    voices,
                    source: "live".to_string(),
                    supports_live_refresh: true,
                    replace_builtin: true,
                    warning: None,
                })
            }
            TtsProvider::OpenAi => Ok(openai_voice_catalog()),
            TtsProvider::OpenAiCompatible => Err(anyhow!(
                "Live voice catalog refresh is unavailable for custom OpenAI-compatible servers"
            )),
            TtsProvider::LocalQwen | TtsProvider::LocalKokoro | TtsProvider::Windows => Err(
                anyhow!("Live voice refresh is unavailable for this provider"),
            ),
        }
    }

    pub fn local_tts_status(&self, kind: LocalTtsKind) -> LocalTtsStatus {
        match kind {
            LocalTtsKind::Qwen => self.local_tts.status(),
            LocalTtsKind::Kokoro => self.local_kokoro.status(),
        }
    }

    pub async fn install_local_tts(
        &self,
        kind: LocalTtsKind,
        disk_reserve_mb: u32,
    ) -> Result<LocalTtsStatus> {
        match kind {
            LocalTtsKind::Qwen => self.local_tts.install(disk_reserve_mb).await,
            LocalTtsKind::Kokoro => self.local_kokoro.install(disk_reserve_mb).await,
        }
    }

    pub fn cancel_local_tts_install(&self, kind: LocalTtsKind) -> bool {
        match kind {
            LocalTtsKind::Qwen => self.local_tts.cancel_install(),
            LocalTtsKind::Kokoro => self.local_kokoro.cancel_install(),
        }
    }

    pub async fn delete_local_tts(&self, kind: LocalTtsKind) -> Result<()> {
        match kind {
            LocalTtsKind::Qwen => self.local_tts.delete().await,
            LocalTtsKind::Kokoro => self.local_kokoro.delete().await,
        }
    }

    pub fn current_state(&self) -> TtsState {
        self.state.read().clone()
    }

    pub(crate) fn begin_batch(self: &Arc<Self>) -> Result<TtsBatchLease> {
        let mut active = self.active_batch.lock();
        if active.is_some() {
            return Err(anyhow!(
                "Another batch text-to-speech conversion is already running"
            ));
        }
        let id = self.next_batch_id.fetch_add(1, Ordering::SeqCst) + 1;
        let cancellation = Arc::new(TtsBatchCancellation::new());
        *active = Some(ActiveTtsBatch {
            id,
            cancellation: Arc::clone(&cancellation),
        });
        Ok(TtsBatchLease {
            manager: Arc::clone(self),
            id,
            cancellation,
        })
    }

    pub fn cancel_batch(&self, batch_id: u64) -> bool {
        let cancellation = self
            .active_batch
            .lock()
            .as_ref()
            .filter(|batch| batch.id == batch_id)
            .map(|batch| Arc::clone(&batch.cancellation));
        if let Some(cancellation) = cancellation {
            cancellation.cancel(self);
            true
        } else {
            false
        }
    }

    pub(crate) async fn wait_for_batch_foreground_slot(
        &self,
        cancellation: &TtsBatchCancellation,
    ) -> Result<tokio::sync::OwnedMutexGuard<()>> {
        loop {
            if cancellation.is_cancelled() {
                return Err(anyhow!("Text-to-speech batch cancelled"));
            }
            match self.try_reserve_foreground_operation() {
                Ok(guard) => {
                    if cancellation.is_cancelled() {
                        drop(guard);
                        return Err(anyhow!("Text-to-speech batch cancelled"));
                    }
                    return Ok(guard);
                }
                Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
            }
        }
    }

    async fn resolve_operation_settings(
        &self,
        operation_id: u64,
        settings: &TtsSettings,
    ) -> Result<TtsSettings> {
        let mut resolved = settings.clone();
        if resolved.provider == TtsProvider::Windows {
            let max_attempts = resolved.retry_count.min(10).saturating_add(1);
            let mut attempt = 1;
            let voice = loop {
                self.ensure_active(operation_id)?;
                let resolution = windows_tts::resolve_voice_selection(&resolved.windows_voice_id);
                let result = tokio::select! {
                    result = resolution => result,
                    _ = self.wait_for_cancellation(operation_id) => {
                        return Err(anyhow!("Text-to-speech operation cancelled"));
                    }
                };
                match result {
                    Ok(voice) => break voice,
                    Err(error) if error.transient && attempt < max_attempts => {
                        let delay = exponential_delay(
                            resolved.retry_base_delay_ms.clamp(100, 30_000),
                            attempt,
                        );
                        log::warn!(
                            "Windows voice catalog failed on attempt {}/{}: {}; retrying in {:.1}s",
                            attempt,
                            max_attempts,
                            error.safe_message,
                            delay.as_secs_f32()
                        );
                        self.update_attempt(
                            operation_id,
                            TtsPhase::Retrying,
                            attempt,
                            Some(safe_text(&error.safe_message)),
                        );
                        tokio::select! {
                            _ = tokio::time::sleep(delay) => {}
                            _ = self.wait_for_cancellation(operation_id) => {
                                return Err(anyhow!("Text-to-speech operation cancelled"));
                            }
                        }
                        attempt = attempt.saturating_add(1);
                    }
                    Err(error) => return Err(anyhow!(error.safe_message)),
                }
            };
            self.ensure_active(operation_id)?;
            resolved.windows_voice_id = voice.id;
            resolved.windows_voice_language = voice.language;
        }
        if resolved.provider == TtsProvider::Murf && resolved.murf_style.is_some() {
            let api_key = resolve_api_key(&resolved)?;
            let catalog = tokio::select! {
                result = murf_voice_catalog(
                    &self.client,
                    &api_key,
                    nonempty_or(&resolved.murf_model, "falcon-2"),
                ) => result?,
                _ = self.wait_for_cancellation(operation_id) => {
                    return Err(anyhow!("Text-to-speech operation cancelled"));
                }
            };
            let voice = catalog
                .iter()
                .find(|voice| voice.id == resolved.murf_voice)
                .ok_or_else(|| {
                    anyhow!(
                        "Murf voice '{}' is not available for model {}",
                        resolved.murf_voice,
                        resolved.murf_model
                    )
                })?;
            let locale = voice
                .locales
                .iter()
                .find(|locale| locale.locale.eq_ignore_ascii_case(&resolved.murf_language))
                .ok_or_else(|| {
                    anyhow!(
                        "Murf voice '{}' does not advertise locale '{}'",
                        resolved.murf_voice,
                        resolved.murf_language
                    )
                })?;
            let requested_style = resolved.murf_style.as_deref().unwrap_or_default();
            let style = locale
                .styles
                .iter()
                .find(|style| style.eq_ignore_ascii_case(requested_style))
                .ok_or_else(|| {
                    anyhow!(
                        "Murf voice '{}' does not advertise style '{}' for locale '{}'",
                        resolved.murf_voice,
                        requested_style,
                        resolved.murf_language
                    )
                })?;
            resolved.murf_voice = voice.id.clone();
            resolved.murf_language = locale.locale.clone();
            resolved.murf_style = Some(style.clone());
        }
        Ok(resolved)
    }

    pub fn try_reserve_foreground_operation(&self) -> Result<tokio::sync::OwnedMutexGuard<()>> {
        try_reserve_foreground_operation_lock(Arc::clone(&self.foreground_operation_lock))
    }

    /// Waits for the shared foreground synthesis lane. Interactive queue
    /// workers use this path so a queued selection remains pending instead of
    /// failing merely because the preceding item is still releasing its lease.
    pub async fn reserve_foreground_operation(&self) -> tokio::sync::OwnedMutexGuard<()> {
        Arc::clone(&self.foreground_operation_lock)
            .lock_owned()
            .await
    }

    /// Cancels exactly one currently running operation.
    pub fn cancel_operation(&self, operation_id: u64) -> bool {
        let _finalization_guard = self.finalization_lock.lock();
        let current = self.current_state();
        if !try_cancel_operation(&self.active_operation_id, &current, operation_id) {
            return false;
        }
        let mut state = self.state.write();
        if state.operation_id == operation_id {
            state.phase = TtsPhase::Cancelled;
            state.message = Some("Text-to-speech cancelled".to_string());
            let snapshot = state.clone();
            drop(state);
            self.emit_state(&snapshot);
        }
        true
    }

    pub fn preprocess_text(text: &str, settings: &TtsSettings) -> String {
        if settings.preprocessing_enabled {
            apply_text_replacements(text, &settings.preprocessing_rules)
        } else {
            text.to_string()
        }
    }

    async fn preprocess_for_scope(
        &self,
        operation_id: u64,
        text: &str,
        settings: &TtsSettings,
        scope: TtsLlmScope,
        mut ui_job: Option<&mut tts_resume::UiFileJobManifest>,
    ) -> Result<String> {
        let mut app_settings = crate::settings::get_settings(&self.app_handle);
        // CLI and History regeneration may supply temporary TTS overrides.
        // Provider definitions remain app-owned, while effective TTS settings
        // must come from the operation rather than the saved JSON snapshot.
        app_settings.tts = settings.clone();
        let config = tts_llm::resolve_config(&app_settings, scope)?;
        let llm_output = if let Some(config) = config {
            let input_chunks = tts_llm::input_chunks(text, config.chunk_target_chars);
            if input_chunks.is_empty() {
                return Err(anyhow!("There is no text for TTS AI cleanup"));
            }
            let cleanup_fingerprint = tts_llm::cleanup_fingerprint(&config, &input_chunks)?;
            let resumed_cleanup = if let Some(job) = ui_job.as_deref_mut() {
                let resumed = tts_resume::load_ui_llm_cleanup_prefix(
                    &self.cache_root,
                    &job.job_id,
                    &cleanup_fingerprint,
                    &input_chunks,
                )?;
                job.cleanup_completed_chunks = resumed.len();
                job.cleanup_total_chunks = input_chunks.len();
                job.status = tts_resume::UiFileJobStatus::Preparing;
                job.last_error = None;
                tts_resume::touch_ui_job(job);
                tts_resume::persist_ui_file_job(&self.cache_root, job)?;
                resumed
            } else {
                Vec::new()
            };
            let cleanup = tts_llm::preprocess_chunks(
                &input_chunks,
                &config,
                resumed_cleanup,
                |progress| {
                    self.update_preprocessing_progress(operation_id, progress);
                },
                |chunk, output| {
                    if let Some(job) = ui_job.as_deref_mut() {
                        tts_resume::persist_ui_llm_cleanup_chunk(
                            &self.cache_root,
                            &job.job_id,
                            &cleanup_fingerprint,
                            chunk,
                            input_chunks.len(),
                            output,
                        )?;
                        job.cleanup_completed_chunks = chunk.index;
                        job.cleanup_total_chunks = input_chunks.len();
                        job.status = tts_resume::UiFileJobStatus::Preparing;
                        job.last_error = None;
                        tts_resume::touch_ui_job(job);
                        tts_resume::persist_ui_file_job(&self.cache_root, job)?;
                    }
                    Ok(())
                },
            );
            tokio::select! {
                result = cleanup => result?,
                _ = self.wait_for_cancellation(operation_id) => {
                    return Err(anyhow!("Text-to-speech operation cancelled"));
                }
            }
        } else {
            text.to_string()
        };
        self.ensure_active(operation_id)?;

        let processed = Self::preprocess_text(&llm_output, settings);
        if processed.len() > MAX_TTS_PROCESSED_TEXT_BYTES {
            return Err(anyhow!(
                "Processed TTS text exceeds the 16 MiB safety limit"
            ));
        }
        if processed.trim().is_empty() {
            return Err(anyhow!("TTS preprocessing returned no speakable text"));
        }
        Ok(processed)
    }

    pub fn chunk_interactive(text: &str, settings: &TtsSettings) -> Vec<TtsChunk> {
        let hard_limit = Self::settings_character_limit(settings);
        semantic_chunks(
            text,
            (settings.interactive_target_chars as usize).clamp(50, hard_limit),
            hard_limit,
        )
    }

    pub fn chunk_file(text: &str, settings: &TtsSettings) -> Vec<TtsChunk> {
        let hard_limit = Self::settings_character_limit(settings);
        semantic_chunks(
            text,
            (settings.file_target_chars as usize).clamp(50, hard_limit),
            hard_limit,
        )
    }

    pub async fn inspect_text_file(
        &self,
        path: impl AsRef<Path>,
        settings: &TtsSettings,
    ) -> Result<TextFileInspection> {
        validate_enabled_independent_settings(settings)?;
        let path = path.as_ref();
        validate_input_extension(path)?;
        let (source, encoding) = read_supported_text_file(path)?;
        let source_for_speech = normalize_source_text(path, &source);
        let llm_cleanup_enabled = settings.llm_preprocessing.file_enabled;
        let locally_processed = if llm_cleanup_enabled {
            // AI output is unknown until the explicit conversion starts. Use
            // the locally normalized source for a non-billable estimate.
            source_for_speech.clone()
        } else {
            Self::preprocess_text(&source_for_speech, settings)
        };
        if !llm_cleanup_enabled && locally_processed.len() > MAX_TTS_PROCESSED_TEXT_BYTES {
            return Err(anyhow!(
                "Processed TTS text exceeds the 16 MiB safety limit"
            ));
        }
        let chunks = Self::chunk_file(&locally_processed, settings);
        let llm_cleanup_chunk_count = if llm_cleanup_enabled {
            tts_llm::input_chunks(
                &source_for_speech,
                settings.llm_preprocessing.chunk_target_chars as usize,
            )
            .len()
        } else {
            0
        };
        Ok(TextFileInspection {
            path: path.to_path_buf(),
            source_character_count: source.chars().count(),
            processed_character_count: locally_processed.chars().count(),
            chunk_count: chunks.len(),
            llm_cleanup_enabled,
            llm_cleanup_chunk_count,
            counts_are_estimates: llm_cleanup_enabled,
            encoding,
        })
    }

    /// Reads the original unprocessed source for opt-in TTS History metadata.
    ///
    /// This deliberately returns the decoded Markdown source, not the rendered
    /// or preprocessed provider text, so future re-synthesis can run the full
    /// current pipeline again.
    pub fn read_original_text_file(&self, path: impl AsRef<Path>) -> Result<String> {
        let path = path.as_ref();
        validate_input_extension(path)?;
        read_supported_text_file(path).map(|(source, _encoding)| source)
    }

    pub fn scan_batch_files(&self, request: TtsBatchScanRequest) -> Result<TtsBatchScanResult> {
        let output_directory =
            canonicalize_batch_directory(&request.output_directory, "batch output directory")?;
        let mut warnings = Vec::new();
        let mut candidates = Vec::new();

        let input_directory = match request.input_directory.as_deref() {
            Some(directory) => {
                if !request.input_paths.is_empty() {
                    return Err(anyhow!(
                        "Choose either one input directory or individual input files, not both"
                    ));
                }
                let directory = canonicalize_batch_directory(directory, "batch input directory")?;
                collect_batch_text_paths(
                    &directory,
                    &output_directory,
                    request.recursive,
                    &mut candidates,
                    &mut warnings,
                )?;
                Some(directory)
            }
            None => {
                if request.input_paths.is_empty() {
                    return Err(anyhow!(
                        "Choose an input directory or at least one text file"
                    ));
                }
                if request.input_paths.len() > MAX_TTS_BATCH_FILES {
                    return Err(anyhow!(
                        "The batch contains more than {MAX_TTS_BATCH_FILES} files"
                    ));
                }
                for path in request.input_paths {
                    if !is_supported_text_path(&path) {
                        continue;
                    }
                    if !path.is_absolute() {
                        warnings.push(format!(
                            "Ignored non-absolute batch input path: {}",
                            path.display()
                        ));
                        continue;
                    }
                    candidates.push(path);
                }
                None
            }
        };

        if candidates.len() > MAX_TTS_BATCH_FILES {
            return Err(anyhow!(
                "The batch contains more than {MAX_TTS_BATCH_FILES} supported files"
            ));
        }
        if input_directory.is_some() {
            candidates.sort_by_key(|path| path.to_string_lossy().to_ascii_lowercase());
        }

        let mut seen_inputs = HashSet::new();
        let mut pending = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            let checked = read_batch_input_no_follow(
                &candidate,
                input_directory.as_deref(),
                request.recursive,
            );
            let (input_path, scan_error) = match checked {
                Ok((canonical_path, _source)) => (canonical_path, None),
                Err(error) => (candidate.clone(), Some(safe_text(&error.to_string()))),
            };
            let input_key = normalized_batch_path_key(&input_path);
            if !seen_inputs.insert(input_key) {
                continue;
            }
            let relative_path = if let Some(root) = input_directory.as_deref() {
                match input_path.strip_prefix(root) {
                    Ok(relative) if relative_path_is_safe(relative) => relative.to_path_buf(),
                    _ => {
                        warnings.push(format!(
                            "Ignored batch input outside the selected folder: {}",
                            input_path.display()
                        ));
                        continue;
                    }
                }
            } else {
                match input_path.file_name() {
                    Some(file_name) => PathBuf::from(file_name),
                    None => {
                        warnings.push(format!(
                            "Ignored batch input without a file name: {}",
                            input_path.display()
                        ));
                        continue;
                    }
                }
            };
            pending.push((input_path, relative_path, scan_error));
        }

        let output_extension = match request.output_format {
            TtsOutputFormat::Mp3 => "mp3",
            TtsOutputFormat::Wav => "wav",
        };
        let mut reserved_outputs = HashSet::new();
        let mut files = Vec::with_capacity(pending.len());
        for (input_path, relative_path, scan_error) in pending {
            let output_path = allocate_batch_output_path(
                &output_directory,
                &relative_path,
                output_extension,
                &mut reserved_outputs,
            )?;
            files.push(TtsBatchFilePlan {
                input_path,
                relative_path,
                output_path,
                scan_error,
            });
        }
        let eligible_count = files
            .iter()
            .filter(|file| file.scan_error.is_none())
            .count();

        Ok(TtsBatchScanResult {
            input_directory,
            output_directory,
            recursive: request.recursive,
            output_format: request.output_format,
            files,
            eligible_count,
            warnings,
        })
    }

    /// Creates, replaces, or removes the folder watcher from the current saved
    /// settings. No OS watcher exists while the feature is disabled.
    pub fn sync_folder_watcher(self: &Arc<Self>) -> Result<()> {
        use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};

        let settings = crate::settings::get_settings(&self.app_handle)
            .tts
            .effective_for_scope(crate::settings::TtsOperationScope::File);
        let generation = self.watcher_generation.fetch_add(1, Ordering::SeqCst) + 1;
        *self.folder_watcher.lock() = None;
        self.watched_paths.lock().clear();
        if !settings.watch_folder_enabled {
            return Ok(());
        }

        let input_dir = PathBuf::from(settings.watch_input_directory.trim());
        let output_dir = PathBuf::from(settings.watch_output_directory.trim());
        if input_dir.as_os_str().is_empty() || !input_dir.is_dir() {
            return Err(anyhow!(
                "TTS watch input directory does not exist: {}",
                input_dir.display()
            ));
        }
        if output_dir.as_os_str().is_empty() {
            return Err(anyhow!("TTS watch output directory is not configured"));
        }
        fs::create_dir_all(&output_dir).with_context(|| {
            format!(
                "Failed to create TTS watch output directory {}",
                output_dir.display()
            )
        })?;
        let output_dir = fs::canonicalize(&output_dir).with_context(|| {
            format!(
                "Failed to resolve TTS watch output directory {}",
                output_dir.display()
            )
        })?;
        let input_dir = fs::canonicalize(&input_dir).with_context(|| {
            format!(
                "Failed to resolve TTS watch input directory {}",
                input_dir.display()
            )
        })?;
        let recursive = settings.watch_recursive;

        // Snapshot before subscribing: these files must never be auto-processed.
        let mut initial = self.watched_paths.lock();
        for path in collect_supported_text_paths(&input_dir, recursive)? {
            initial.insert(path);
        }
        drop(initial);

        let manager = Arc::clone(self);
        let seen = Arc::clone(&self.watched_paths);
        let mut watcher = RecommendedWatcher::new(
            move |event: notify::Result<notify::Event>| {
                let event = match event {
                    Ok(event) => event,
                    Err(error) => {
                        let message = format!(
                            "TTS folder watcher error: {}",
                            safe_text(&error.to_string())
                        );
                        log::error!("{message}");
                        manager.emit_state(&TtsState {
                            operation_id: 0,
                            kind: Some(TtsOperationKind::FileConversion),
                            phase: TtsPhase::Error,
                            provider: None,
                            completed_chunks: 0,
                            total_chunks: 0,
                            current_attempt: 0,
                            message: Some(message),
                        });
                        return;
                    }
                };
                if matches!(event.kind, notify::EventKind::Remove(_)) {
                    for path in event.paths {
                        seen.lock().retain(|candidate| {
                            candidate != &path && !candidate.starts_with(&path)
                        });
                    }
                    return;
                }
                if !matches!(
                    event.kind,
                    notify::EventKind::Create(_) | notify::EventKind::Modify(_)
                ) {
                    return;
                }
                for path in event
                    .paths
                    .into_iter()
                    .filter(|path| is_supported_text_path(path))
                {
                    let queued = {
                        let mut watched = seen.lock();
                        queue_watched_path(&mut watched, path.clone())
                    };
                    if !queued {
                        continue;
                    }
                    let manager = Arc::clone(&manager);
                    tauri::async_runtime::spawn(async move {
                        manager.process_watched_file(path, generation).await;
                    });
                }
            },
            Config::default(),
        )
        .map_err(|error| anyhow!("Failed to create TTS folder watcher: {error}"))?;
        watcher
            .watch(
                &input_dir,
                if recursive {
                    RecursiveMode::Recursive
                } else {
                    RecursiveMode::NonRecursive
                },
            )
            .map_err(|error| {
                anyhow!(
                    "Failed to watch TTS input directory {}: {error}",
                    input_dir.display()
                )
            })?;
        // Reconcile once after subscribing to close the snapshot/watch race.
        for path in collect_supported_text_paths(&input_dir, recursive)? {
            let queued = if is_supported_text_path(&path) {
                let mut watched = self.watched_paths.lock();
                queue_watched_path(&mut watched, path.clone())
            } else {
                false
            };
            if queued {
                let manager = Arc::clone(self);
                tauri::async_runtime::spawn(async move {
                    manager.process_watched_file(path, generation).await;
                });
            }
        }
        let mut resumed_sources = HashSet::new();
        for task in tts_resume::discover_watcher_tasks(&output_dir)? {
            if !watcher_resume_task_allowed(
                &task,
                &input_dir,
                &output_dir,
                recursive,
                settings.output_format,
            ) {
                log::warn!(
                    "Ignoring stale or out-of-scope TTS watcher checkpoint: source={} output={}",
                    task.source_path.display(),
                    task.output_path.display()
                );
                continue;
            }
            if !resumed_sources.insert(task.source_path.clone()) {
                continue;
            }
            let manager = Arc::clone(self);
            tauri::async_runtime::spawn(async move {
                manager
                    .process_watched_file_to(task.source_path, Some(task.output_path), generation)
                    .await;
            });
        }
        *self.folder_watcher.lock() = Some(watcher);
        log::info!(
            "TTS folder watcher enabled: input={} output={} recursive={}",
            input_dir.display(),
            output_dir.display(),
            recursive
        );
        Ok(())
    }

    async fn process_watched_file(self: Arc<Self>, input_path: PathBuf, generation: u64) {
        self.process_watched_file_to(input_path, None, generation)
            .await;
    }

    fn watcher_is_current(&self, generation: u64) -> bool {
        crate::settings::get_settings(&self.app_handle)
            .tts
            .watch_folder_enabled
            && self.watcher_generation.load(Ordering::SeqCst) == generation
    }

    async fn process_watched_file_to(
        self: Arc<Self>,
        input_path: PathBuf,
        resume_output_path: Option<PathBuf>,
        generation: u64,
    ) {
        let result = async {
            let initial_settings = crate::settings::get_settings(&self.app_handle)
                .tts
                .effective_for_scope(crate::settings::TtsOperationScope::File);
            if !initial_settings.watch_folder_enabled
                || self.watcher_generation.load(Ordering::SeqCst) != generation
            {
                return Ok(());
            }
            wait_until_file_stable(
                &input_path,
                Duration::from_millis(u64::from(
                    initial_settings.watch_settle_delay_ms.clamp(100, 60_000),
                )),
                || self.watcher_generation.load(Ordering::SeqCst) == generation,
            )
            .await?;

            // Folder events may arrive in bursts. Queue automatic conversions
            // so one new file cannot supersede another manager operation.
            let _conversion_guard = self.watched_conversion_lock.lock().await;
            let settings = crate::settings::get_settings(&self.app_handle)
                .tts
                .effective_for_scope(crate::settings::TtsOperationScope::File);
            if !settings.watch_folder_enabled
                || self.watcher_generation.load(Ordering::SeqCst) != generation
            {
                return Ok(());
            }
            if !settings.watch_folder_enabled
                || self.watcher_generation.load(Ordering::SeqCst) != generation
            {
                return Ok(());
            }
            let (canonical_input, source) = read_watched_input_no_follow(
                &input_path,
                Path::new(settings.watch_input_directory.trim()),
                settings.watch_recursive,
            )?;
            let output_dir = PathBuf::from(settings.watch_output_directory.trim());
            let extension = match settings.output_format {
                TtsOutputFormat::Mp3 => "mp3",
                TtsOutputFormat::Wav => "wav",
            };
            let require_resume_checkpoint = resume_output_path.is_some();
            let output_path = if let Some(path) = resume_output_path {
                path
            } else {
                unique_watched_output_path(&output_dir, &canonical_input, extension)?
            };
            let conversion = loop {
                if !self.watcher_is_current(generation) {
                    return Ok(());
                }
                while matches!(
                    self.current_state().phase,
                    TtsPhase::Preparing
                        | TtsPhase::Preprocessing
                        | TtsPhase::Synthesizing
                        | TtsPhase::Retrying
                        | TtsPhase::Ready
                ) {
                    if !self.watcher_is_current(generation) {
                        return Ok(());
                    }
                    tokio::time::sleep(Duration::from_millis(250)).await;
                }
                if !self.watcher_is_current(generation) {
                    return Ok(());
                }

                match self
                    .convert_decoded_text_file_resolved(
                        &canonical_input,
                        &source,
                        &output_path,
                        &settings,
                        TtsLlmScope::File,
                        None,
                        ResumeOrigin::Watcher {
                            source_path: canonical_input.clone(),
                            output_path: output_path.clone(),
                        },
                        require_resume_checkpoint,
                        None,
                    )
                    .await
                {
                    Ok(result) => break result,
                    Err(error)
                        if error
                            .to_string()
                            .to_ascii_lowercase()
                            .contains("already running")
                            && self.watcher_is_current(generation) =>
                    {
                        log::info!(
                            "Retrying queued TTS folder conversion after foreground operation: {}",
                            input_path.display()
                        );
                        tokio::time::sleep(Duration::from_millis(500)).await;
                    }
                    Err(error) => return Err(error),
                }
            };
            let ResolvedTtsResult {
                value: conversion,
                settings,
            } = conversion;
            if settings.file_history_enabled {
                let source_kind = canonical_input
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .filter(|extension| extension.eq_ignore_ascii_case("md"))
                    .map(|_| TtsHistorySourceKind::Markdown)
                    .unwrap_or(TtsHistorySourceKind::Text);
                let history_result = self
                    .app_handle
                    .try_state::<Arc<TtsHistoryManager>>()
                    .ok_or_else(|| anyhow!("TTS History manager is unavailable"))
                    .and_then(|history| {
                        history
                            .save_success(
                                metadata_from_settings(
                                    &settings,
                                    TtsHistoryScope::File,
                                    source.clone(),
                                    source_kind,
                                    format!(
                                        "watch-{}-{}",
                                        chrono::Utc::now().timestamp_millis(),
                                        conversion.operation_id
                                    ),
                                    Some(output_path.clone()),
                                ),
                                &output_path,
                            )
                            .map(|_| ())
                    });
                if let Err(error) = history_result {
                    let message =
                        format!("Automatic TTS completed, but History capture failed: {error}");
                    log::error!("{message}");
                    let _ = self.app_handle.emit("tts-history-error", message);
                }
            }
            log::info!(
                "TTS watched file converted: input={} output={}",
                input_path.display(),
                output_path.display()
            );
            Ok::<(), anyhow::Error>(())
        }
        .await;

        self.watched_paths.lock().remove(&input_path);
        if let Err(error) = result {
            let message = format!(
                "TTS folder conversion failed for {}: {}",
                input_path.display(),
                safe_text(&error.to_string())
            );
            log::error!("{message}");
            let event = TtsState {
                operation_id: 0,
                kind: Some(TtsOperationKind::FileConversion),
                phase: TtsPhase::Error,
                provider: None,
                completed_chunks: 0,
                total_chunks: 0,
                current_attempt: 0,
                message: Some(message),
            };
            self.emit_state(&event);
        }
    }

    pub(crate) async fn synthesize_interactive_reserved<F, S>(
        self: &Arc<Self>,
        text: &str,
        settings: &TtsSettings,
        _operation_guard: tokio::sync::OwnedMutexGuard<()>,
        on_started: S,
        on_resolved: F,
    ) -> Result<ResolvedTtsResult<InteractiveSynthesis>>
    where
        F: FnOnce(&TtsSettings) + Send,
        S: FnOnce(u64) -> bool + Send,
    {
        ensure_enabled(settings)?;
        let (operation_id, preparing_state) =
            self.reserve_operation(TtsOperationKind::Interactive, settings.provider, 0);
        if !on_started(operation_id) {
            self.cancel_operation(operation_id);
            return Err(anyhow!("Text-to-speech operation cancelled"));
        }
        self.ensure_active(operation_id)?;
        self.emit_state(&preparing_state);
        self.ensure_active(operation_id)?;
        let resolved_settings = match self
            .resolve_operation_settings(operation_id, settings)
            .await
        {
            Ok(settings) => settings,
            Err(error) => {
                self.finish_result::<()>(operation_id, &Err(anyhow!(error.to_string())));
                return Err(error);
            }
        };
        let settings = &resolved_settings;
        if settings.interactive_history_enabled {
            if let Err(error) = validate_output_settings(settings) {
                self.finish_result::<()>(operation_id, &Err(anyhow!(error.to_string())));
                return Err(error);
            }
        }
        on_resolved(settings);
        let processed = match self
            .preprocess_for_scope(
                operation_id,
                text,
                settings,
                TtsLlmScope::Interactive,
                None,
            )
            .await
        {
            Ok(processed) => processed,
            Err(error) => {
                self.finish_result::<()>(operation_id, &Err(anyhow!(error.to_string())));
                return Err(error);
            }
        };
        let chunks = Self::chunk_interactive(&processed, settings);
        if chunks.is_empty() {
            let error = anyhow!("There is no speakable text");
            self.finish_result::<()>(operation_id, &Err(anyhow!(error.to_string())));
            return Err(error);
        }
        self.set_synthesis_plan(operation_id, chunks.len());
        let operation_cache =
            match create_interactive_operation_cache(&self.cache_root, operation_id) {
                Ok(path) => path,
                Err(error) => {
                    self.fail_operation(operation_id, error.to_string());
                    return Err(error).context("Failed to create the TTS cache directory");
                }
            };
        let speed_milli = (settings.speed.clamp(0.25, 4.0) * 1_000.0).round() as u64;
        let estimated_pcm_bytes = (processed.chars().count() as u64)
            .saturating_mul(12_000_000_u64.saturating_div(speed_milli.max(250)))
            .saturating_add(16 * 1024 * 1024);
        if let Err(error) = ensure_disk_reserve(
            &operation_cache.join("audio.wav"),
            settings.disk_reserve_mb,
            estimated_pcm_bytes,
        ) {
            self.fail_operation(operation_id, error.to_string());
            return Err(error);
        }

        let result = async {
            let api_key = resolve_api_key(settings)?;
            let mut ready = Vec::with_capacity(chunks.len());
            let streams_cloud_pcm = matches!(
                settings.provider,
                TtsProvider::Soniox
                    | TtsProvider::Deepgram
                    | TtsProvider::OpenAi
                    | TtsProvider::OpenAiCompatible
                    | TtsProvider::ElevenLabs
                    | TtsProvider::Cartesia
            ) || (settings.provider == TtsProvider::Murf
                && settings.murf_model == "falcon-2");
            let mut next_playback_chunk_index = 1_usize;
            let history_raw_path = operation_cache.join("history-result.pcm.partial");
            let history_audio_path = operation_cache.join(format!(
                "history-result.{}",
                match settings.output_format {
                    TtsOutputFormat::Mp3 => "mp3",
                    TtsOutputFormat::Wav => "wav",
                }
            ));
            let mut history_raw_file = if settings.interactive_history_enabled {
                Some(
                    OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&history_raw_path)
                        .with_context(|| {
                            format!(
                                "Failed to create interactive history PCM {}",
                                history_raw_path.display()
                            )
                        })?,
                )
            } else {
                None
            };
            for chunk in &chunks {
                self.ensure_active(operation_id)?;
                let pcm = if streams_cloud_pcm {
                    let mut publish_segment =
                        |segment: &[i16]| -> std::result::Result<(), ProviderAttemptError> {
                            self.ensure_active(operation_id)
                                .map_err(local_attempt_error)?;
                            let wav_path = operation_cache
                                .join(format!("{:05}.wav", next_playback_chunk_index));
                            ensure_disk_reserve(
                                &wav_path,
                                settings.disk_reserve_mb,
                                segment.len().saturating_mul(2).saturating_add(44) as u64,
                            )
                            .map_err(local_attempt_error)?;
                            write_wav_file(&wav_path, segment, PROVIDER_PCM_SAMPLE_RATE)
                                .map_err(local_attempt_error)?;

                            let event = TtsChunkReady {
                                operation_id,
                                chunk_index: next_playback_chunk_index,
                                // Zero means that more playback segments can
                                // still arrive. The final event supplies the
                                // authoritative total.
                                total_chunks: 0,
                                wav_path,
                                boundary_after: TtsBoundary::Hard,
                                pause_after_ms: 0,
                            };
                            next_playback_chunk_index = next_playback_chunk_index.saturating_add(1);
                            ready.push(event.clone());
                            let _ = self.app_handle.emit(TTS_EVENT_CHUNK_READY, &event);
                            Ok(())
                        };
                    let result = self
                        .synthesize_chunk_with_retry_streaming(
                            operation_id,
                            chunk,
                            chunks.len(),
                            settings,
                            &api_key,
                            &mut publish_segment,
                        )
                        .await;
                    drop(publish_segment);
                    result?
                } else {
                    self.synthesize_chunk_with_retry(
                        operation_id,
                        chunk,
                        chunks.len(),
                        settings,
                        &api_key,
                    )
                    .await?
                };
                self.ensure_active(operation_id)?;

                let pause_after_ms = interactive_pause_after(chunk, chunks.len(), settings);
                if streams_cloud_pcm {
                    let last = ready.last_mut().ok_or_else(|| {
                        anyhow!(
                            "{} returned empty audio for chunk {}",
                            provider_name(settings.provider),
                            chunk.index
                        )
                    })?;
                    last.boundary_after = chunk.boundary_after;
                    last.pause_after_ms = pause_after_ms;
                    let _ = self.app_handle.emit(TTS_EVENT_CHUNK_READY, last.clone());
                } else {
                    let wav_path = operation_cache.join(format!("{:05}.wav", chunk.index));
                    ensure_disk_reserve(
                        &wav_path,
                        settings.disk_reserve_mb,
                        pcm.len().saturating_mul(2).saturating_add(44) as u64,
                    )?;
                    write_wav_file(&wav_path, &pcm, PROVIDER_PCM_SAMPLE_RATE)?;
                    let event = TtsChunkReady {
                        operation_id,
                        chunk_index: chunk.index,
                        total_chunks: chunks.len(),
                        wav_path,
                        boundary_after: chunk.boundary_after,
                        pause_after_ms,
                    };
                    ready.push(event.clone());
                    let _ = self.app_handle.emit(TTS_EVENT_CHUNK_READY, &event);
                }

                if let Some(raw_file) = history_raw_file.as_mut() {
                    write_i16_le(raw_file, &pcm)?;
                    let pause_bytes = u64::from(pause_after_ms)
                        .saturating_mul(u64::from(PROVIDER_PCM_SAMPLE_RATE))
                        .saturating_div(1_000)
                        .saturating_mul(2);
                    ensure_disk_reserve(
                        &history_raw_path,
                        settings.disk_reserve_mb,
                        (pcm.len() as u64)
                            .saturating_mul(2)
                            .saturating_add(pause_bytes),
                    )?;
                    if pause_after_ms > 0 {
                        write_silence(raw_file, pause_after_ms, PROVIDER_PCM_SAMPLE_RATE)?;
                    }
                    raw_file
                        .flush()
                        .context("Failed to flush interactive history PCM")?;
                }
                self.mark_chunk_completed(operation_id, chunk.index, chunks.len());
            }
            if streams_cloud_pcm {
                let total_playback_chunks = ready.len();
                let last = ready
                    .last_mut()
                    .ok_or_else(|| anyhow!("The provider returned no playable audio"))?;
                last.total_chunks = total_playback_chunks;
                let _ = self.app_handle.emit(TTS_EVENT_CHUNK_READY, last.clone());
            }
            let combined_audio_path = if let Some(mut raw_file) = history_raw_file {
                raw_file
                    .flush()
                    .context("Failed to flush interactive history PCM")?;
                raw_file
                    .sync_all()
                    .context("Failed to finalize interactive history PCM")?;
                drop(raw_file);
                self.ensure_active(operation_id)?;

                let raw_bytes = fs::metadata(&history_raw_path)
                    .with_context(|| {
                        format!(
                            "Failed to inspect interactive history PCM {}",
                            history_raw_path.display()
                        )
                    })?
                    .len();
                ensure_disk_reserve(
                    &history_audio_path,
                    settings.disk_reserve_mb,
                    raw_bytes.saturating_add(64 * 1024),
                )?;
                let mut output = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&history_audio_path)
                    .with_context(|| {
                        format!(
                            "Failed to create interactive history audio {}",
                            history_audio_path.display()
                        )
                    })?;
                match settings.output_format {
                    TtsOutputFormat::Wav => write_wav_from_pcm_file(
                        &history_raw_path,
                        &mut output,
                        PROVIDER_PCM_SAMPLE_RATE,
                    )?,
                    TtsOutputFormat::Mp3 => encode_mp3_cbr_file(
                        &history_raw_path,
                        &mut output,
                        PROVIDER_PCM_SAMPLE_RATE,
                        settings.mp3_bitrate_kbps,
                    )?,
                }
                output
                    .sync_all()
                    .context("Failed to finalize interactive history audio")?;
                drop(output);
                fs::remove_file(&history_raw_path).with_context(|| {
                    format!(
                        "Failed to remove interactive history PCM {}",
                        history_raw_path.display()
                    )
                })?;
                Some(history_audio_path)
            } else {
                None
            };
            self.complete_operation_if_active(operation_id)?;
            Ok(InteractiveSynthesis {
                operation_id,
                processed_character_count: processed.chars().count(),
                chunks: ready,
                combined_audio_path,
            })
        }
        .await;

        if result.is_err() {
            let _ = fs::remove_file(operation_cache.join("history-result.pcm.partial"));
            let _ = fs::remove_file(operation_cache.join("history-result.mp3"));
            let _ = fs::remove_file(operation_cache.join("history-result.wav"));
        }
        self.finish_result(operation_id, &result);
        result.map(|value| ResolvedTtsResult {
            value,
            settings: resolved_settings,
        })
    }

    /// Converts a supported text file to one final WAV or CBR MP3. Provider
    /// chunks are deterministic and sequential; a transient failure retries
    /// only its current chunk.
    pub async fn convert_text_file(
        self: &Arc<Self>,
        input_path: impl AsRef<Path>,
        output_path: impl AsRef<Path>,
        settings: &TtsSettings,
    ) -> Result<FileConversionResult> {
        Ok(self
            .convert_text_file_resolved(input_path, output_path, settings)
            .await?
            .value)
    }

    pub(crate) async fn convert_text_file_resolved(
        self: &Arc<Self>,
        input_path: impl AsRef<Path>,
        output_path: impl AsRef<Path>,
        settings: &TtsSettings,
    ) -> Result<ResolvedTtsResult<FileConversionResult>> {
        validate_enabled_independent_settings(settings)?;
        validate_input_extension(input_path.as_ref())?;
        validate_output_extension(output_path.as_ref(), settings.output_format)?;
        validate_output_settings(settings)?;
        if output_path.as_ref().exists() {
            return Err(anyhow!(
                "Output file already exists: {}",
                output_path.as_ref().display()
            ));
        }
        let (source, _encoding) = read_supported_text_file(input_path.as_ref())?;
        self.convert_decoded_text_file_resolved(
            input_path.as_ref(),
            &source,
            output_path.as_ref(),
            settings,
            TtsLlmScope::File,
            None,
            ResumeOrigin::Manual,
            false,
            None,
        )
        .await
    }

    pub async fn create_ui_file_job(
        self: &Arc<Self>,
        input_path: impl AsRef<Path>,
        output_path: impl AsRef<Path>,
        settings: &TtsSettings,
    ) -> Result<(String, ResolvedTtsResult<FileConversionResult>)> {
        validate_enabled_independent_settings(settings)?;
        validate_input_extension(input_path.as_ref())?;
        validate_output_extension(output_path.as_ref(), settings.output_format)?;
        validate_output_settings(settings)?;
        if output_path.as_ref().exists() {
            return Err(anyhow!(
                "Output file already exists: {}",
                output_path.as_ref().display()
            ));
        }
        let (source, _encoding) = read_supported_text_file(input_path.as_ref())?;
        let manifest = {
            let _registry_guard = self.ui_jobs_lock.lock();
            tts_resume::create_ui_file_job(
                &self.cache_root,
                input_path.as_ref().to_path_buf(),
                output_path.as_ref().to_path_buf(),
                source,
                settings.clone(),
            )?
        };
        let job_id = manifest.job_id.clone();
        let result = self.resume_ui_file_job(&job_id).await?;
        Ok((job_id, result))
    }

    pub async fn resume_ui_file_job(
        self: &Arc<Self>,
        job_id: &str,
    ) -> Result<ResolvedTtsResult<FileConversionResult>> {
        let manifest = tts_resume::load_ui_file_job(&self.cache_root, job_id)?;
        if manifest.output_path.exists() {
            return Err(anyhow!(
                "Output file already exists: {}",
                manifest.output_path.display()
            ));
        }
        let operation_guard = self.try_reserve_foreground_operation()?;
        self.convert_decoded_text_file_reserved(
            &manifest.source_path,
            &manifest.source_text,
            &manifest.output_path,
            &manifest.settings,
            TtsLlmScope::File,
            None,
            ResumeOrigin::UiJob {
                job_id: manifest.job_id.clone(),
                source_path: manifest.source_path.clone(),
                output_path: manifest.output_path.clone(),
            },
            false,
            operation_guard,
            None,
            Some(job_id),
        )
        .await
    }

    /// Re-synthesizes retained History source through the prompt collection
    /// matching its original scope, while still using the non-overlay file
    /// conversion sink.
    pub(crate) async fn convert_text_file_for_history_resolved(
        self: &Arc<Self>,
        input_path: impl AsRef<Path>,
        output_path: impl AsRef<Path>,
        settings: &TtsSettings,
        scope: TtsLlmScope,
        resume_namespace: Option<&str>,
    ) -> Result<ResolvedTtsResult<FileConversionResult>> {
        validate_enabled_independent_settings(settings)?;
        validate_input_extension(input_path.as_ref())?;
        let (source, _encoding) = read_supported_text_file(input_path.as_ref())?;
        self.convert_decoded_text_file_resolved(
            input_path.as_ref(),
            &source,
            output_path.as_ref(),
            settings,
            scope,
            resume_namespace,
            ResumeOrigin::Manual,
            false,
            None,
        )
        .await
    }

    pub(crate) async fn convert_batch_file_resolved(
        self: &Arc<Self>,
        plan: &TtsBatchFilePlan,
        input_root: Option<&Path>,
        recursive: bool,
        output_root: &Path,
        settings: &TtsSettings,
        operation_guard: tokio::sync::OwnedMutexGuard<()>,
        cancellation: &TtsBatchCancellation,
    ) -> Result<(ResolvedTtsResult<FileConversionResult>, String, String)> {
        if cancellation.is_cancelled() {
            return Err(anyhow!("Text-to-speech batch cancelled"));
        }
        let (canonical_input, source) =
            read_batch_input_no_follow(&plan.input_path, input_root, recursive)?;
        let current_relative = if let Some(root) = input_root {
            canonical_input
                .strip_prefix(root)
                .map(Path::to_path_buf)
                .map_err(|_| {
                    anyhow!(
                        "Batch input moved outside the selected folder: {}",
                        plan.input_path.display()
                    )
                })?
        } else {
            PathBuf::from(
                canonical_input
                    .file_name()
                    .ok_or_else(|| anyhow!("Batch input has no file name"))?,
            )
        };
        if current_relative != plan.relative_path || !relative_path_is_safe(&current_relative) {
            return Err(anyhow!(
                "Batch input no longer matches the scanned relative path: {}",
                plan.input_path.display()
            ));
        }
        prepare_batch_output_path(&plan.output_path, output_root, &current_relative, settings)?;
        if cancellation.is_cancelled() {
            return Err(anyhow!("Text-to-speech batch cancelled"));
        }
        let manifest = {
            let _registry_guard = self.ui_jobs_lock.lock();
            match tts_resume::find_ui_file_job_by_output(&self.cache_root, &plan.output_path)? {
                Some(existing)
                    if tts_resume::paths_refer_to_same_output(
                        &existing.source_path,
                        &canonical_input,
                    ) && existing.source_text == source =>
                {
                    existing
                }
                Some(_) => {
                    return Err(anyhow!(
                        "An unfinished conversion already owns this output path but its source has changed. Resume or discard that saved conversion first."
                    ));
                }
                None => tts_resume::create_ui_file_job(
                    &self.cache_root,
                    canonical_input.clone(),
                    plan.output_path.clone(),
                    source.clone(),
                    settings.clone(),
                )?,
            }
        };
        let job_id = manifest.job_id.clone();
        let result = self
            .convert_decoded_text_file_reserved(
                &canonical_input,
                &source,
                &plan.output_path,
                settings,
                TtsLlmScope::File,
                None,
                ResumeOrigin::UiJob {
                    job_id: job_id.clone(),
                    source_path: canonical_input.clone(),
                    output_path: plan.output_path.clone(),
                },
                false,
                operation_guard,
                Some(cancellation),
                Some(&job_id),
            )
            .await?;
        Ok((result, source, job_id))
    }

    pub fn discard_managed_resume_namespace(&self, resume_namespace: &str) -> Result<()> {
        tts_resume::discard_managed(&self.cache_root, resume_namespace)
    }

    pub fn list_ui_file_jobs(&self) -> Result<Vec<tts_resume::UiFileJobSummary>> {
        let _registry_guard = self.ui_jobs_lock.lock();
        let active_job_id = self.active_ui_job.lock().clone();
        tts_resume::list_ui_file_jobs(&self.cache_root, active_job_id.as_deref())
    }

    pub fn discard_ui_file_job(&self, job_id: &str) -> Result<()> {
        let _registry_guard = self.ui_jobs_lock.lock();
        if self.active_ui_job.lock().as_deref() == Some(job_id) {
            return Err(anyhow!(
                "Pause this TTS conversion before discarding its saved progress"
            ));
        }
        tts_resume::discard_ui_file_job(&self.cache_root, job_id)
    }

    pub fn ui_file_job_history_source(&self, job_id: &str) -> Result<(String, PathBuf)> {
        let job = tts_resume::load_ui_file_job(&self.cache_root, job_id)?;
        Ok((job.source_text, job.source_path))
    }

    pub fn complete_ui_file_job(&self, job_id: &str) -> Result<()> {
        let _registry_guard = self.ui_jobs_lock.lock();
        tts_resume::remove_ui_file_job_record(&self.cache_root, job_id)
    }

    pub fn export_ui_file_job_partial(&self, job_id: &str, destination: &Path) -> Result<usize> {
        let job = tts_resume::load_ui_file_job(&self.cache_root, job_id)?;
        if matches!(
            job.status,
            tts_resume::UiFileJobStatus::Preparing
                | tts_resume::UiFileJobStatus::Running
                | tts_resume::UiFileJobStatus::Retrying
        ) {
            return Err(anyhow!(
                "Pause the TTS conversion before exporting its completed part"
            ));
        }
        if job.chunks.is_empty() || job.processed_character_count.is_none() {
            return Err(anyhow!("This TTS job has no completed audio to export"));
        }
        validate_output_extension(destination, job.settings.output_format)?;
        if destination.exists() {
            return Err(anyhow!(
                "Output file already exists: {}",
                destination.display()
            ));
        }
        let signature = tts_resume::synthesis_signature(&job.chunks, &job.settings)?;
        let workspace = ResumeWorkspace::open_for_output(
            &job.output_path,
            signature,
            job.chunks.len(),
            ResumeOrigin::UiJob {
                job_id: job.job_id.clone(),
                source_path: job.source_path.clone(),
                output_path: job.output_path.clone(),
            },
        )?;
        let completed_chunks = workspace.completed_chunks();
        if completed_chunks == 0 || workspace.committed_bytes() == 0 {
            return Err(anyhow!("This TTS job has no completed audio to export"));
        }
        ensure_disk_reserve(
            destination,
            job.settings.disk_reserve_mb,
            workspace.committed_bytes().saturating_add(64 * 1024),
        )?;
        let (mut output, temporary) = create_partial_export_file(destination)?;
        let encode_result = match job.settings.output_format {
            TtsOutputFormat::Wav => {
                write_wav_from_pcm_file(workspace.raw_path(), &mut output, PROVIDER_PCM_SAMPLE_RATE)
            }
            TtsOutputFormat::Mp3 => encode_mp3_cbr_file(
                workspace.raw_path(),
                &mut output,
                PROVIDER_PCM_SAMPLE_RATE,
                job.settings.mp3_bitrate_kbps,
            ),
        }
        .and_then(|_| output.sync_all().map_err(anyhow::Error::from));
        drop(output);
        if let Err(error) = encode_result {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
        if let Err(error) = crate::no_clobber::publish_new_file(&temporary, destination) {
            let _ = fs::remove_file(&temporary);
            return Err(error).with_context(|| {
                format!(
                    "Failed to publish partial TTS audio {}",
                    destination.display()
                )
            });
        }
        Ok(completed_chunks)
    }

    async fn convert_decoded_text_file_resolved(
        self: &Arc<Self>,
        input_path: &Path,
        source: &str,
        output_path: &Path,
        settings: &TtsSettings,
        preprocessing_scope: TtsLlmScope,
        resume_namespace: Option<&str>,
        resume_origin: ResumeOrigin,
        require_resume_checkpoint: bool,
        ui_job_id: Option<&str>,
    ) -> Result<ResolvedTtsResult<FileConversionResult>> {
        validate_enabled_independent_settings(settings)?;
        let operation_guard = self.try_reserve_foreground_operation()?;
        self.convert_decoded_text_file_reserved(
            input_path,
            source,
            output_path,
            settings,
            preprocessing_scope,
            resume_namespace,
            resume_origin,
            require_resume_checkpoint,
            operation_guard,
            None,
            ui_job_id,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn convert_decoded_text_file_reserved(
        self: &Arc<Self>,
        input_path: &Path,
        source: &str,
        output_path: &Path,
        settings: &TtsSettings,
        preprocessing_scope: TtsLlmScope,
        resume_namespace: Option<&str>,
        resume_origin: ResumeOrigin,
        require_resume_checkpoint: bool,
        _operation_guard: tokio::sync::OwnedMutexGuard<()>,
        batch_cancellation: Option<&TtsBatchCancellation>,
        ui_job_id: Option<&str>,
    ) -> Result<ResolvedTtsResult<FileConversionResult>> {
        validate_enabled_independent_settings(settings)?;
        validate_input_extension(input_path)?;
        validate_output_extension(output_path, settings.output_format)?;
        validate_output_settings(settings)?;
        if output_path.exists() {
            return Err(output_already_exists_error(output_path));
        }

        let _active_ui_job = ui_job_id
            .map(|job_id| ActiveUiJobRegistration::new(&self.active_ui_job, job_id.to_string()));

        let mut ui_job = if let Some(job_id) = ui_job_id {
            let mut job = tts_resume::load_ui_file_job(&self.cache_root, job_id)?;
            if !tts_resume::paths_refer_to_same_output(&job.source_path, input_path)
                || !tts_resume::paths_refer_to_same_output(&job.output_path, output_path)
            {
                return Err(anyhow!(
                    "The saved TTS job paths do not match the requested conversion"
                ));
            }
            job.status = if job.processed_character_count.is_some() && !job.chunks.is_empty() {
                tts_resume::UiFileJobStatus::Running
            } else {
                tts_resume::UiFileJobStatus::Preparing
            };
            job.last_error = None;
            tts_resume::touch_ui_job(&mut job);
            tts_resume::persist_ui_file_job(&self.cache_root, &job)?;
            Some(job)
        } else {
            None
        };

        let operation_id =
            self.begin_operation(TtsOperationKind::FileConversion, settings.provider, 0);
        let _batch_registration = batch_cancellation
            .map(|cancellation| cancellation.attach_operation(self, operation_id));
        let saved_plan_available = ui_job
            .as_ref()
            .is_some_and(|job| job.processed_character_count.is_some() && !job.chunks.is_empty());
        let resolved_settings = if saved_plan_available {
            ui_job
                .as_ref()
                .map(|job| job.settings.clone())
                .expect("saved UI job checked above")
        } else {
            match self
                .resolve_operation_settings(operation_id, settings)
                .await
            {
                Ok(settings) => settings,
                Err(error) => {
                    if let Some(job) = ui_job.as_mut() {
                        job.status = self.ui_job_error_status(operation_id);
                        job.last_error = Some(error.to_string());
                        tts_resume::touch_ui_job(job);
                        let _ = tts_resume::persist_ui_file_job(&self.cache_root, job);
                    }
                    self.finish_result::<()>(operation_id, &Err(anyhow!(error.to_string())));
                    return Err(error);
                }
            }
        };
        let settings = &resolved_settings;
        let (processed_character_count, chunks) = if saved_plan_available {
            let job = ui_job.as_ref().expect("saved UI job checked above");
            (
                job.processed_character_count
                    .expect("saved UI job plan checked above"),
                job.chunks.clone(),
            )
        } else {
            let source_for_speech = normalize_source_text(input_path, source);
            let processed = match self
                .preprocess_for_scope(
                    operation_id,
                    &source_for_speech,
                    settings,
                    preprocessing_scope,
                    ui_job.as_mut(),
                )
                .await
            {
                Ok(processed) => processed,
                Err(error) => {
                    if let Some(job) = ui_job.as_mut() {
                        job.status = self.ui_job_error_status(operation_id);
                        job.last_error = Some(error.to_string());
                        tts_resume::touch_ui_job(job);
                        let _ = tts_resume::persist_ui_file_job(&self.cache_root, job);
                    }
                    self.finish_result::<()>(operation_id, &Err(anyhow!(error.to_string())));
                    return Err(error);
                }
            };
            let chunks = Self::chunk_file(&processed, settings);
            let processed_character_count = processed.chars().count();
            if chunks.is_empty() {
                let error = anyhow!("There is no speakable text to convert");
                if let Some(job) = ui_job.as_mut() {
                    job.status = self.ui_job_error_status(operation_id);
                    job.last_error = Some(error.to_string());
                    tts_resume::touch_ui_job(job);
                    let _ = tts_resume::persist_ui_file_job(&self.cache_root, job);
                }
                self.finish_result::<()>(operation_id, &Err(anyhow!(error.to_string())));
                return Err(error);
            }
            if let Some(job) = ui_job.as_mut() {
                job.processed_text = None;
                job.processed_character_count = Some(processed_character_count);
                job.chunks = chunks.clone();
                job.settings = resolved_settings.clone();
                job.status = tts_resume::UiFileJobStatus::Running;
                job.last_error = None;
                tts_resume::touch_ui_job(job);
                if let Err(error) = tts_resume::persist_ui_file_job(&self.cache_root, job) {
                    self.finish_result::<()>(operation_id, &Err(anyhow!(error.to_string())));
                    return Err(error);
                }
            }
            (processed_character_count, chunks)
        };
        let preparation = (|| -> Result<(Vec<TtsChunk>, ResumeWorkspace, usize)> {
            let synthesis_signature = tts_resume::synthesis_signature(&chunks, settings)?;
            let resume_workspace = if let Some(namespace) = resume_namespace {
                ResumeWorkspace::open_managed(
                    &self.cache_root,
                    namespace,
                    synthesis_signature,
                    chunks.len(),
                    resume_origin,
                )?
            } else {
                ResumeWorkspace::open_for_output(
                    output_path,
                    synthesis_signature,
                    chunks.len(),
                    resume_origin,
                )?
            };
            if output_path.exists() {
                return Err(output_appeared_while_waiting_error(output_path));
            }
            let resumed_chunks = resume_workspace.completed_chunks();
            if require_resume_checkpoint && resumed_chunks == 0 {
                resume_workspace.discard();
                return Err(anyhow!(
                    "The saved watcher checkpoint no longer matches the current source/settings; the pre-existing file was skipped without an API request"
                ));
            }
            ensure_disk_capacity(
                output_path,
                processed_character_count,
                chunks.len(),
                resume_workspace.committed_bytes(),
                settings,
            )?;
            Ok((chunks.clone(), resume_workspace, resumed_chunks))
        })();
        let (chunks, mut resume_workspace, resumed_chunks) = match preparation {
            Ok(prepared) => prepared,
            Err(error) => {
                if let Some(job) = ui_job.as_mut() {
                    job.status = self.ui_job_error_status(operation_id);
                    job.last_error = Some(error.to_string());
                    tts_resume::touch_ui_job(job);
                    let _ = tts_resume::persist_ui_file_job(&self.cache_root, job);
                }
                self.finish_result::<()>(operation_id, &Err(anyhow!(error.to_string())));
                return Err(error);
            }
        };
        self.set_synthesis_plan(operation_id, chunks.len());
        if resumed_chunks > 0 {
            self.mark_resume_loaded(operation_id, resumed_chunks, chunks.len());
        }
        let raw_partial = resume_workspace.raw_path().to_path_buf();
        let encoded_partial = resume_workspace.encoded_partial_path();
        let result = async {
            let api_key = if resumed_chunks < chunks.len() {
                Some(resolve_api_key(settings)?)
            } else {
                None
            };

            for chunk in chunks.iter().skip(resumed_chunks) {
                self.ensure_active(operation_id)?;
                let pcm = self
                    .synthesize_chunk_with_retry(
                        operation_id,
                        chunk,
                        chunks.len(),
                        settings,
                        api_key
                            .as_deref()
                            .ok_or_else(|| anyhow!("TTS API key was not resolved"))?,
                    )
                    .await?;
                self.ensure_active(operation_id)?;
                let pause_ms = if chunk.index < chunks.len() {
                    if chunk.boundary_after == TtsBoundary::Paragraph {
                        settings.paragraph_pause_ms.min(10_000)
                    } else {
                        settings.inter_chunk_pause_ms.min(5_000)
                    }
                } else {
                    0
                };
                let pause_bytes = u64::from(pause_ms)
                    .saturating_mul(u64::from(PROVIDER_PCM_SAMPLE_RATE))
                    .saturating_div(1_000)
                    .saturating_mul(2);
                ensure_disk_reserve(
                    output_path,
                    settings.disk_reserve_mb,
                    (pcm.len() as u64)
                        .saturating_mul(2)
                        .saturating_add(pause_bytes),
                )?;
                let segment = pcm_segment_bytes(&pcm, pause_ms, PROVIDER_PCM_SAMPLE_RATE)?;
                resume_workspace.append_segment(chunk.index, &segment)?;
                self.mark_chunk_completed(operation_id, chunk.index, chunks.len());
            }
            self.ensure_active(operation_id)?;

            let mut final_partial = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&encoded_partial)
                .with_context(|| {
                    format!(
                        "Failed to create encoded partial file {}",
                        encoded_partial.display()
                    )
                })?;
            let raw_bytes = fs::metadata(&raw_partial)
                .with_context(|| format!("Failed to inspect {}", raw_partial.display()))?
                .len();
            ensure_disk_reserve(
                output_path,
                settings.disk_reserve_mb,
                raw_bytes.saturating_add(64 * 1024),
            )?;
            match settings.output_format {
                TtsOutputFormat::Wav => write_wav_from_pcm_file(
                    &raw_partial,
                    &mut final_partial,
                    PROVIDER_PCM_SAMPLE_RATE,
                )?,
                TtsOutputFormat::Mp3 => encode_mp3_cbr_file(
                    &raw_partial,
                    &mut final_partial,
                    PROVIDER_PCM_SAMPLE_RATE,
                    settings.mp3_bitrate_kbps,
                )?,
            }
            self.ensure_active(operation_id)?;
            final_partial
                .sync_all()
                .context("Failed to finalize encoded audio")?;
            drop(final_partial);
            self.ensure_active(operation_id)?;

            {
                let _finalization_guard = self.finalization_lock.lock();
                self.ensure_active(operation_id)?;
                if let Err(error) =
                    crate::no_clobber::publish_new_file(&encoded_partial, output_path)
                {
                    if output_path.exists() {
                        return Err(output_appeared_while_waiting_error(output_path));
                    }
                    return Err(error).with_context(|| {
                        format!(
                            "Failed to publish completed audio file {}",
                            output_path.display()
                        )
                    });
                }
                self.mark_completed_locked(operation_id)?;
            }

            Ok(FileConversionResult {
                operation_id,
                output_path: output_path.to_path_buf(),
                source_character_count: source.chars().count(),
                processed_character_count,
                chunk_count: chunks.len(),
                resumed_chunks,
                output_format: settings.output_format,
                mp3_bitrate_kbps: (settings.output_format == TtsOutputFormat::Mp3)
                    .then_some(settings.mp3_bitrate_kbps),
            })
        }
        .await;

        if result.is_err() {
            let _ = fs::remove_file(&encoded_partial);
        }
        let cancelled = self.active_operation_id.load(Ordering::SeqCst) != operation_id;
        let completed_chunks = resume_workspace.completed_chunks();
        if let Some(job) = ui_job.as_mut() {
            job.completed_chunks = completed_chunks;
            job.status = if result.is_ok() {
                tts_resume::UiFileJobStatus::Completed
            } else if cancelled {
                tts_resume::UiFileJobStatus::Paused
            } else {
                tts_resume::UiFileJobStatus::Failed
            };
            job.last_error = result.as_ref().err().map(ToString::to_string);
            tts_resume::touch_ui_job(job);
            let _ = tts_resume::persist_ui_file_job(&self.cache_root, job);
        }
        self.finish_result(operation_id, &result);
        let resolved = result.map(|value| ResolvedTtsResult {
            value,
            settings: resolved_settings,
        });
        if ui_job_id.is_some() {
            if resolved.is_ok() {
                resume_workspace.discard();
            }
        } else if cancelled || (resolved.is_ok() && resume_namespace.is_none()) {
            resume_workspace.discard();
        }
        resolved
    }

    fn begin_operation(
        &self,
        kind: TtsOperationKind,
        provider: TtsProvider,
        total_chunks: usize,
    ) -> u64 {
        let (operation_id, state) = self.reserve_operation(kind, provider, total_chunks);
        self.emit_state(&state);
        operation_id
    }

    /// Establishes an operation ID and manager state without publishing the
    /// first event. Interactive queue callers claim that ID before Preparing
    /// can reach the overlay, closing the final Clear/Stop startup boundary.
    fn reserve_operation(
        &self,
        kind: TtsOperationKind,
        provider: TtsProvider,
        total_chunks: usize,
    ) -> (u64, TtsState) {
        let operation_id = self.active_operation_id.fetch_add(1, Ordering::SeqCst) + 1;
        let state = TtsState {
            operation_id,
            kind: Some(kind),
            phase: TtsPhase::Preparing,
            provider: Some(provider),
            completed_chunks: 0,
            total_chunks,
            current_attempt: 0,
            message: None,
        };
        *self.state.write() = state.clone();
        (operation_id, state)
    }

    fn ensure_active(&self, operation_id: u64) -> Result<()> {
        if !operation_is_active(&self.active_operation_id, operation_id) {
            Err(anyhow!("Text-to-speech operation cancelled"))
        } else {
            Ok(())
        }
    }

    fn ui_job_error_status(&self, operation_id: u64) -> tts_resume::UiFileJobStatus {
        if operation_is_active(&self.active_operation_id, operation_id) {
            tts_resume::UiFileJobStatus::Failed
        } else {
            tts_resume::UiFileJobStatus::Paused
        }
    }

    fn complete_operation_if_active(&self, operation_id: u64) -> Result<()> {
        let _finalization_guard = self.finalization_lock.lock();
        self.ensure_active(operation_id)?;
        self.mark_completed_locked(operation_id)
    }

    fn mark_completed_locked(&self, operation_id: u64) -> Result<()> {
        let mut state = self.state.write();
        if state.operation_id != operation_id {
            return Err(anyhow!("Text-to-speech operation cancelled"));
        }
        state.phase = TtsPhase::Completed;
        state.message = None;
        let snapshot = state.clone();
        drop(state);
        self.emit_state(&snapshot);
        Ok(())
    }

    fn emit_state(&self, state: &TtsState) {
        let _ = self.app_handle.emit(TTS_EVENT_STATE, state);
    }

    fn update_attempt(
        &self,
        operation_id: u64,
        phase: TtsPhase,
        attempt: u8,
        message: Option<String>,
    ) {
        let mut state = self.state.write();
        if state.operation_id != operation_id {
            return;
        }
        state.phase = phase;
        state.current_attempt = attempt;
        state.message = message;
        let snapshot = state.clone();
        drop(state);
        self.emit_state(&snapshot);
    }

    fn update_preprocessing_progress(&self, operation_id: u64, progress: TtsLlmProgress) {
        let mut state = self.state.write();
        if state.operation_id != operation_id {
            return;
        }
        state.phase = TtsPhase::Preprocessing;
        state.completed_chunks = progress.completed_chunks;
        state.total_chunks = progress.total_chunks;
        state.current_attempt = progress.attempt;
        state.message = Some(progress.message);
        let snapshot = state.clone();
        drop(state);
        self.emit_state(&snapshot);
        let _ = self.app_handle.emit(
            TTS_EVENT_PROGRESS,
            TtsProgress {
                operation_id,
                completed_chunks: progress.completed_chunks,
                total_chunks: progress.total_chunks,
                current_chunk: progress.current_chunk,
                attempt: progress.attempt,
            },
        );
    }

    fn set_synthesis_plan(&self, operation_id: u64, total_chunks: usize) {
        let mut state = self.state.write();
        if state.operation_id != operation_id {
            return;
        }
        state.phase = TtsPhase::Preparing;
        state.completed_chunks = 0;
        state.total_chunks = total_chunks;
        state.current_attempt = 0;
        state.message = None;
        let snapshot = state.clone();
        drop(state);
        self.emit_state(&snapshot);
    }

    fn mark_resume_loaded(&self, operation_id: u64, completed: usize, total: usize) {
        let mut state = self.state.write();
        if state.operation_id != operation_id {
            return;
        }
        state.phase = TtsPhase::Preparing;
        state.completed_chunks = completed;
        state.current_attempt = 0;
        state.message = Some(format!(
            "Recovered {completed}/{total} verified audio chunks from the previous attempt."
        ));
        let snapshot = state.clone();
        drop(state);
        self.emit_state(&snapshot);
        let _ = self.app_handle.emit(
            TTS_EVENT_PROGRESS,
            TtsProgress {
                operation_id,
                completed_chunks: completed,
                total_chunks: total,
                current_chunk: completed,
                attempt: 0,
            },
        );
    }

    fn mark_chunk_completed(&self, operation_id: u64, completed: usize, total: usize) {
        let mut state = self.state.write();
        if state.operation_id != operation_id {
            return;
        }
        state.phase = TtsPhase::Ready;
        state.completed_chunks = completed;
        state.current_attempt = 0;
        state.message = None;
        let snapshot = state.clone();
        drop(state);
        self.emit_state(&snapshot);
        let _ = self.app_handle.emit(
            TTS_EVENT_PROGRESS,
            TtsProgress {
                operation_id,
                completed_chunks: completed,
                total_chunks: total,
                current_chunk: completed,
                attempt: 0,
            },
        );
    }

    fn fail_operation(&self, operation_id: u64, message: String) {
        let mut state = self.state.write();
        if state.operation_id != operation_id {
            return;
        }
        state.phase = TtsPhase::Error;
        state.message = Some(message);
        let snapshot = state.clone();
        drop(state);
        self.emit_state(&snapshot);
    }

    fn finish_result<T>(&self, operation_id: u64, result: &Result<T>) {
        let mut state = self.state.write();
        if state.operation_id != operation_id {
            return;
        }
        match result {
            _ if self.active_operation_id.load(Ordering::SeqCst) != operation_id => {
                state.phase = TtsPhase::Cancelled;
                state.message = Some("Text-to-speech cancelled".to_string());
            }
            Ok(_) => {
                state.phase = TtsPhase::Completed;
                state.message = None;
            }
            Err(error) => {
                state.phase = TtsPhase::Error;
                state.message = Some(safe_text(&error.to_string()));
            }
        }
        let snapshot = state.clone();
        drop(state);
        self.emit_state(&snapshot);
    }

    async fn synthesize_chunk_with_retry(
        &self,
        operation_id: u64,
        chunk: &TtsChunk,
        total_chunks: usize,
        settings: &TtsSettings,
        api_key: &str,
    ) -> Result<Vec<i16>> {
        self.synthesize_chunk_with_retry_inner(
            operation_id,
            chunk,
            total_chunks,
            settings,
            api_key,
            None,
        )
        .await
    }

    async fn synthesize_chunk_with_retry_streaming(
        &self,
        operation_id: u64,
        chunk: &TtsChunk,
        total_chunks: usize,
        settings: &TtsSettings,
        api_key: &str,
        on_pcm_segment: &mut PcmSegmentSink<'_>,
    ) -> Result<Vec<i16>> {
        self.synthesize_chunk_with_retry_inner(
            operation_id,
            chunk,
            total_chunks,
            settings,
            api_key,
            Some(on_pcm_segment),
        )
        .await
    }

    async fn synthesize_chunk_with_retry_inner(
        &self,
        operation_id: u64,
        chunk: &TtsChunk,
        total_chunks: usize,
        settings: &TtsSettings,
        api_key: &str,
        mut on_pcm_segment: Option<&mut PcmSegmentSink<'_>>,
    ) -> Result<Vec<i16>> {
        let max_attempts = settings.retry_count.min(10).saturating_add(1);
        for attempt in 1..=max_attempts {
            self.ensure_active(operation_id)?;
            self.update_attempt(operation_id, TtsPhase::Synthesizing, attempt, None);
            let _ = self.app_handle.emit(
                TTS_EVENT_PROGRESS,
                TtsProgress {
                    operation_id,
                    completed_chunks: chunk.index.saturating_sub(1),
                    total_chunks,
                    current_chunk: chunk.index,
                    attempt,
                },
            );

            let mut emitted_during_attempt = false;
            let synthesis = if let Some(callback) = on_pcm_segment.as_deref_mut() {
                let mut tracked_callback = |pcm: &[i16]| {
                    let result = callback(pcm);
                    if result.is_ok() {
                        emitted_during_attempt = true;
                    }
                    result
                };
                self.synthesize_once(
                    operation_id,
                    &chunk.text,
                    settings,
                    api_key,
                    Some(&mut tracked_callback),
                )
                .await
            } else {
                self.synthesize_once(operation_id, &chunk.text, settings, api_key, None)
                    .await
            };

            match synthesis {
                Ok(pcm) if pcm.is_empty() => {
                    return Err(anyhow!(
                        "{} returned empty audio for chunk {}",
                        provider_name(settings.provider),
                        chunk.index
                    ));
                }
                Ok(pcm) => return Ok(pcm),
                Err(mut error) => {
                    self.ensure_active(operation_id)?;
                    if emitted_during_attempt {
                        error.transient = false;
                        error.safe_message = format!(
                            "{} (the streamed response ended after playback began)",
                            error.safe_message
                        );
                    }
                    let safe_error = if api_key.is_empty() {
                        error.safe_message.clone()
                    } else {
                        error.safe_message.replace(api_key, "[redacted]")
                    };
                    let status = attempt_status_label(settings.provider, error.status);
                    log::error!(
                        "TTS provider={} status={} chunk={}/{} attempt={}/{} error={}",
                        provider_name(settings.provider),
                        status,
                        chunk.index,
                        total_chunks,
                        attempt,
                        max_attempts,
                        safe_error
                    );
                    if !error.transient || attempt >= max_attempts {
                        return Err(anyhow!(
                            "{} TTS failed (status {}, chunk {}, attempt {}): {}",
                            provider_name(settings.provider),
                            status,
                            chunk.index,
                            attempt,
                            safe_error
                        ));
                    }

                    let delay = error.retry_after.unwrap_or_else(|| {
                        exponential_delay(settings.retry_base_delay_ms.clamp(100, 30_000), attempt)
                    });
                    let message = format!(
                        "{}; retrying chunk {} in {:.1}s",
                        safe_error,
                        chunk.index,
                        delay.as_secs_f32()
                    );
                    self.update_attempt(operation_id, TtsPhase::Retrying, attempt, Some(message));
                    self.cancellable_delay(operation_id, delay).await?;
                }
            }
        }
        Err(anyhow!("TTS retry loop ended unexpectedly"))
    }

    async fn cancellable_delay(&self, operation_id: u64, delay: Duration) -> Result<()> {
        let slice = Duration::from_millis(100);
        let mut remaining = delay.min(MAX_RETRY_DELAY);
        while !remaining.is_zero() {
            self.ensure_active(operation_id)?;
            let sleep_for = remaining.min(slice);
            tokio::time::sleep(sleep_for).await;
            remaining = remaining.saturating_sub(sleep_for);
        }
        self.ensure_active(operation_id)
    }

    async fn synthesize_once(
        &self,
        operation_id: u64,
        text: &str,
        settings: &TtsSettings,
        api_key: &str,
        mut on_pcm_segment: Option<&mut PcmSegmentSink<'_>>,
    ) -> std::result::Result<Vec<i16>, ProviderAttemptError> {
        if text.trim().is_empty() {
            return Err(ProviderAttemptError {
                status: None,
                safe_message: "Refusing to send an empty or whitespace-only TTS request"
                    .to_string(),
                transient: false,
                retry_after: None,
            });
        }
        if text.chars().count() > Self::settings_character_limit(settings) {
            return Err(ProviderAttemptError {
                status: None,
                safe_message: "TTS chunk exceeds the provider character limit".to_string(),
                transient: false,
                retry_after: None,
            });
        }

        if settings.provider == TtsProvider::LocalQwen {
            let synthesis = self.local_tts.synthesize(
                text,
                nonempty_or(&settings.local_qwen_voice, "Ryan"),
                nonempty_or(&settings.local_qwen_language, "Auto"),
                "",
                settings.speed,
            );
            return tokio::select! {
                result = synthesis => result.map_err(|error| ProviderAttemptError {
                    status: None,
                    safe_message: error.safe_message,
                    transient: error.transient,
                    retry_after: None,
                }),
                _ = self.wait_for_cancellation(operation_id) => {
                    self.local_tts.stop_worker().await;
                    Err(cancelled_attempt_error())
                }
            };
        }
        if settings.provider == TtsProvider::LocalKokoro {
            let synthesis = self.local_kokoro.synthesize(
                text,
                nonempty_or(&settings.local_kokoro_voice, "af_maple"),
                nonempty_or(&settings.local_kokoro_language, "English"),
                settings.speed,
            );
            return tokio::select! {
                result = synthesis => result.map_err(|error| ProviderAttemptError {
                    status: None,
                    safe_message: error.safe_message,
                    transient: error.transient,
                    retry_after: None,
                }),
                _ = self.wait_for_cancellation(operation_id) => {
                    self.local_kokoro.stop_worker().await;
                    Err(cancelled_attempt_error())
                }
            };
        }
        if settings.provider == TtsProvider::Windows {
            // The WinRT call runs on a blocking thread. Cancellation returns to
            // AivoRelay promptly via select and discards the late result, though
            // the detached OS operation may still finish internally.
            let synthesis = windows_tts::synthesize(
                text.to_string(),
                settings.windows_voice_id.clone(),
                settings.speed,
            );
            return tokio::select! {
                result = synthesis => result.map_err(|error| ProviderAttemptError {
                    status: None,
                    safe_message: error.safe_message,
                    transient: error.transient,
                    retry_after: None,
                }),
                _ = self.wait_for_cancellation(operation_id) => Err(cancelled_attempt_error()),
            };
        }
        if settings.provider == TtsProvider::Edge {
            let synthesis = edge_tts::synthesize(
                text,
                nonempty_or(&settings.edge_voice, DEFAULT_EDGE_TTS_VOICE),
                settings.speed,
            );
            return tokio::select! {
                result = synthesis => result.map_err(|error| ProviderAttemptError {
                    status: None,
                    safe_message: error.safe_message,
                    transient: error.transient,
                    retry_after: None,
                }),
                _ = self.wait_for_cancellation(operation_id) => Err(cancelled_attempt_error()),
            };
        }

        let request = match settings.provider {
            TtsProvider::Soniox => {
                self.client
                    .post(SONIOX_TTS_URL)
                    .bearer_auth(api_key)
                    .json(&json!({
                        "model": nonempty_or(&settings.soniox_model, "tts-rt-v1"),
                        "language": nonempty_or(&settings.soniox_language, "en"),
                        "voice": nonempty_or(&settings.soniox_voice, DEFAULT_TTS_SONIOX_VOICE),
                        "audio_format": "pcm_s16le",
                        "sample_rate": PROVIDER_PCM_SAMPLE_RATE,
                        "speed": settings.speed.clamp(0.7, 1.3),
                        "text": text,
                    }))
            }
            TtsProvider::Deepgram => {
                let query = vec![
                    (
                        "model".to_string(),
                        nonempty_or(&settings.deepgram_model, "aura-2-thalia-en").to_string(),
                    ),
                    ("encoding".to_string(), "linear16".to_string()),
                    ("container".to_string(), "none".to_string()),
                    ("sample_rate".to_string(), "24000".to_string()),
                    (
                        "speed".to_string(),
                        settings.speed.clamp(0.7, 1.5).to_string(),
                    ),
                ];
                self.client
                    .post(DEEPGRAM_TTS_URL)
                    .query(&query)
                    .header("Authorization", format!("Token {api_key}"))
                    .json(&json!({ "text": text }))
            }
            TtsProvider::OpenAi => {
                let mut body = json!({
                    "model": nonempty_or(&settings.openai_model, "gpt-4o-mini-tts"),
                    "voice": nonempty_or(&settings.openai_voice, DEFAULT_TTS_OPENAI_VOICE),
                    "input": text,
                    "response_format": "pcm",
                    "speed": settings.speed.clamp(0.25, 4.0),
                });
                if !settings.openai_instructions.trim().is_empty()
                    && Self::openai_model_supports_instructions(&settings.openai_model)
                {
                    body["instructions"] = Value::String(settings.openai_instructions.clone());
                }
                self.client
                    .post(OPENAI_TTS_URL)
                    .bearer_auth(api_key)
                    .json(&body)
            }
            TtsProvider::OpenAiCompatible => {
                let base_url = match normalize_openai_compatible_base_url_with_insecure_option(
                    &settings.openai_compatible_base_url,
                    settings.openai_compatible_allow_insecure_http,
                ) {
                    Ok(url) => url,
                    Err(err) => {
                        return Err(ProviderAttemptError {
                            status: None,
                            safe_message: err,
                            transient: false,
                            retry_after: None,
                        });
                    }
                };
                let speech_url = if base_url.ends_with("/audio/speech") || base_url.ends_with("/speech") {
                    base_url.to_string()
                } else if base_url.ends_with("/audio") {
                    format!("{base_url}/speech")
                } else {
                    format!("{base_url}/audio/speech")
                };
                let body = json!({
                    "model": nonempty_or(&settings.openai_compatible_model, DEFAULT_TTS_OPENAI_COMPATIBLE_MODEL),
                    "voice": nonempty_or(&settings.openai_compatible_voice, DEFAULT_TTS_OPENAI_COMPATIBLE_VOICE),
                    "input": text,
                    "response_format": "pcm",
                    "speed": settings.speed.clamp(0.25, 4.0),
                });
                let mut req = self.client.post(&speech_url);
                if !api_key.trim().is_empty() {
                    req = req.bearer_auth(api_key);
                }
                req.json(&body)
            }
            TtsProvider::Murf => {
                if settings.murf_model == "gen2" {
                    let mut body = json!({
                        "text": text,
                        "voiceId": nonempty_or(&settings.murf_voice, DEFAULT_TTS_MURF_GEN2_VOICE),
                        "modelVersion": "GEN2",
                        "format": "PCM",
                        "sampleRate": PROVIDER_PCM_SAMPLE_RATE,
                        "channelType": "MONO",
                        "encodeAsBase64": true,
                        "locale": nonempty_or(&settings.murf_language, "en-US"),
                        "rate": settings.murf_rate.clamp(-50, 50),
                        "pitch": settings.murf_pitch.clamp(-50, 50),
                        "variation": settings.murf_variation.min(5),
                    });
                    if let Some(style) = settings.murf_style.as_deref() {
                        body["style"] = Value::String(style.to_string());
                    }
                    self.client
                        .post(MURF_GEN2_TTS_URL)
                        .header("api-key", api_key)
                        .json(&body)
                } else {
                    let mut body = json!({
                        "text": text,
                        "voiceId": nonempty_or(&settings.murf_voice, DEFAULT_TTS_MURF_VOICE),
                        "model": "falcon-2",
                        "format": "PCM",
                        "sampleRate": PROVIDER_PCM_SAMPLE_RATE,
                        "channelType": "MONO",
                        "locale": nonempty_or(&settings.murf_language, "en-US"),
                        "rate": settings.murf_rate.clamp(-50, 50),
                        "pitch": settings.murf_pitch.clamp(-50, 50),
                    });
                    if let Some(style) = settings.murf_style.as_deref() {
                        body["style"] = Value::String(style.to_string());
                    }
                    self.client
                        .post(MURF_FALCON_TTS_URL)
                        .header("api-key", api_key)
                        .json(&body)
                }
            }
            TtsProvider::ElevenLabs => {
                let mut url = reqwest::Url::parse(ELEVENLABS_TTS_BASE_URL).map_err(|error| {
                    ProviderAttemptError {
                        status: None,
                        safe_message: format!("Invalid ElevenLabs endpoint: {error}"),
                        transient: false,
                        retry_after: None,
                    }
                })?;
                url.path_segments_mut()
                    .map_err(|_| ProviderAttemptError {
                        status: None,
                        safe_message: "Invalid ElevenLabs endpoint".to_string(),
                        transient: false,
                        retry_after: None,
                    })?
                    .push(nonempty_or(
                        &settings.elevenlabs_voice,
                        DEFAULT_TTS_ELEVENLABS_VOICE,
                    ));
                url.query_pairs_mut()
                    .append_pair("output_format", "pcm_24000");
                let is_v3 = settings.elevenlabs_model == "eleven_v3";
                let mut voice_settings = json!({
                    "stability": settings.elevenlabs_stability.clamp(0.0, 1.0),
                    "style": settings.elevenlabs_style.clamp(0.0, 1.0),
                });
                if !is_v3 {
                    voice_settings["similarity_boost"] = Value::from(
                        settings.elevenlabs_similarity_boost.clamp(0.0, 1.0),
                    );
                    voice_settings["use_speaker_boost"] =
                        Value::Bool(settings.elevenlabs_use_speaker_boost);
                    voice_settings["speed"] = Value::from(settings.speed.clamp(0.7, 1.2));
                }
                let mut body = json!({
                    "text": text,
                    "model_id": nonempty_or(
                        &settings.elevenlabs_model,
                        DEFAULT_TTS_ELEVENLABS_MODEL
                    ),
                    "voice_settings": voice_settings,
                    "apply_text_normalization": elevenlabs_normalization_value(
                        settings.elevenlabs_apply_text_normalization
                    ),
                });
                // ElevenLabs explicitly does not support language_code for
                // Multilingual v2. Eleven v3 accepts it as a pronunciation and
                // text-normalization hint for short or ambiguous input.
                if is_v3 && !settings.elevenlabs_language.trim().is_empty() {
                    body["language_code"] = Value::String(
                        settings.elevenlabs_language.trim().to_ascii_lowercase(),
                    );
                }
                self.client
                    .post(url)
                    .header("xi-api-key", api_key)
                    .json(&body)
            }
            TtsProvider::Cartesia => {
                let mut body = json!({
                    "model_id": "sonic-3.5",
                    "transcript": text,
                    "voice": {
                        "mode": "id",
                        "id": nonempty_or(&settings.cartesia_voice, DEFAULT_TTS_CARTESIA_VOICE),
                    },
                    "language": nonempty_or(&settings.cartesia_language, "en").to_ascii_lowercase(),
                    "output_format": {
                        "container": "raw",
                        "encoding": "pcm_s16le",
                        "sample_rate": PROVIDER_PCM_SAMPLE_RATE,
                    },
                });
                let mut generation_config = json!({
                    "speed": settings.speed.clamp(0.6, 1.5),
                    "volume": settings.cartesia_volume.clamp(0.5, 2.0),
                });
                if let Some(emotion) = settings.cartesia_emotion.as_deref() {
                    generation_config["emotion"] = Value::String(emotion.to_string());
                }
                body["generation_config"] = generation_config;
                self.client
                    .post(CARTESIA_TTS_URL)
                    .bearer_auth(api_key)
                    .header("Cartesia-Version", CARTESIA_API_VERSION)
                    .json(&body)
            }
            TtsProvider::Edge => unreachable!("Edge provider returned before HTTP dispatch"),
            TtsProvider::LocalQwen => unreachable!("local provider returned before HTTP dispatch"),
            TtsProvider::LocalKokoro => {
                unreachable!("local provider returned before HTTP dispatch")
            }
            TtsProvider::Windows => unreachable!("Windows provider returned before HTTP dispatch"),
        };

        let mut response = tokio::select! {
            response = request.send() => response.map_err(network_error)?,
            _ = self.wait_for_cancellation(operation_id) => {
                return Err(cancelled_attempt_error());
            }
        };
        let status = response.status();
        let retry_after = parse_retry_after(response.headers().get(RETRY_AFTER));
        if settings.provider == TtsProvider::Murf && settings.murf_model == "gen2" {
            return self
                .decode_murf_gen2_response(operation_id, response, status, retry_after)
                .await;
        }
        if !status.is_success() {
            let bytes = self
                .read_error_body_bounded(operation_id, response)
                .await?;
            return decode_cloud_pcm_response(status, retry_after, &bytes);
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_CLOUD_PCM_RESPONSE_BYTES as u64)
        {
            return Err(nontransient_attempt_error(
                "The TTS provider audio response is unexpectedly large",
            ));
        }
        if on_pcm_segment.is_none() {
            let bytes = self
                .read_success_body_bounded(
                    operation_id,
                    response,
                    MAX_CLOUD_PCM_RESPONSE_BYTES,
                    "The TTS provider audio response is unexpectedly large",
                )
                .await?;
            return decode_cloud_pcm_response(status, retry_after, &bytes);
        }

        let initial_capacity = response
            .content_length()
            .and_then(|length| usize::try_from(length).ok())
            .unwrap_or_default()
            .min(16 * 1024 * 1024);
        let mut bytes = Vec::with_capacity(initial_capacity);
        let mut emitted_bytes = 0_usize;
        let segment_bytes = INTERACTIVE_STREAM_SEGMENT_SAMPLES.saturating_mul(2);

        loop {
            let next = tokio::select! {
                next = response.chunk() => next.map_err(network_error)?,
                _ = self.wait_for_cancellation(operation_id) => {
                    return Err(cancelled_attempt_error());
                }
            };
            let Some(next) = next else {
                break;
            };
            if bytes.len().saturating_add(next.len()) > MAX_CLOUD_PCM_RESPONSE_BYTES {
                return Err(nontransient_attempt_error(
                    "The TTS provider audio response is unexpectedly large",
                ));
            }
            bytes.extend_from_slice(&next);

            if let Some(callback) = on_pcm_segment.as_deref_mut() {
                publish_complete_pcm_segments(&bytes, &mut emitted_bytes, segment_bytes, callback)?;
            }
        }

        let pcm = decode_cloud_pcm_response(status, retry_after, &bytes)?;
        if emitted_bytes < bytes.len() {
            let tail = decode_cloud_pcm_response(StatusCode::OK, None, &bytes[emitted_bytes..])?;
            if let Some(callback) = on_pcm_segment.as_deref_mut() {
                callback(&tail)?;
            }
        }
        Ok(pcm)
    }

    async fn read_error_body_bounded(
        &self,
        operation_id: u64,
        mut response: reqwest::Response,
    ) -> std::result::Result<Vec<u8>, ProviderAttemptError> {
        let mut bytes = Vec::new();
        while bytes.len() < MAX_PROVIDER_ERROR_BODY_BYTES {
            let next = tokio::select! {
                next = response.chunk() => next.map_err(network_error)?,
                _ = self.wait_for_cancellation(operation_id) => {
                    return Err(cancelled_attempt_error());
                }
            };
            let Some(next) = next else {
                break;
            };
            let remaining = MAX_PROVIDER_ERROR_BODY_BYTES.saturating_sub(bytes.len());
            bytes.extend_from_slice(&next[..next.len().min(remaining)]);
        }
        Ok(bytes)
    }

    async fn read_success_body_bounded(
        &self,
        operation_id: u64,
        mut response: reqwest::Response,
        maximum: usize,
        oversized_message: &str,
    ) -> std::result::Result<Vec<u8>, ProviderAttemptError> {
        if response
            .content_length()
            .is_some_and(|length| length > maximum as u64)
        {
            return Err(nontransient_attempt_error(oversized_message));
        }
        let mut bytes = Vec::with_capacity(
            response
                .content_length()
                .and_then(|length| usize::try_from(length).ok())
                .unwrap_or_default()
                .min(maximum),
        );
        loop {
            let next = tokio::select! {
                next = response.chunk() => next.map_err(network_error)?,
                _ = self.wait_for_cancellation(operation_id) => {
                    return Err(cancelled_attempt_error());
                }
            };
            let Some(next) = next else {
                break;
            };
            if bytes.len().saturating_add(next.len()) > maximum {
                return Err(nontransient_attempt_error(oversized_message));
            }
            bytes.extend_from_slice(&next);
        }
        Ok(bytes)
    }

    async fn decode_murf_gen2_response(
        &self,
        operation_id: u64,
        response: reqwest::Response,
        status: StatusCode,
        retry_after: Option<Duration>,
    ) -> std::result::Result<Vec<i16>, ProviderAttemptError> {
        if !status.is_success() {
            let bytes = self
                .read_error_body_bounded(operation_id, response)
                .await?;
            return decode_cloud_pcm_response(status, retry_after, &bytes);
        }
        let bytes = self
            .read_success_body_bounded(
                operation_id,
                response,
                MAX_MURF_GEN2_JSON_BYTES,
                "The Murf Gen2 JSON response is unexpectedly large",
            )
            .await?;
        self.ensure_active(operation_id)
            .map_err(local_attempt_error)?;
        let json: Value = serde_json::from_slice(&bytes).map_err(|_| {
            nontransient_attempt_error("Murf Gen2 returned an invalid JSON response")
        })?;
        let encoded = json
            .get("encodedAudio")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                nontransient_attempt_error(
                    "Murf Gen2 response did not contain nonempty encodedAudio",
                )
            })?;
        let maximum_encoded_length = MAX_MURF_GEN2_PCM_BYTES.div_ceil(3).saturating_mul(4);
        if encoded.len() > maximum_encoded_length {
            return Err(nontransient_attempt_error(
                "The Murf Gen2 audio payload is unexpectedly large",
            ));
        }
        let decoded = BASE64_STANDARD.decode(encoded).map_err(|_| {
            nontransient_attempt_error("Murf Gen2 returned invalid base64 audio")
        })?;
        if decoded.len() > MAX_MURF_GEN2_PCM_BYTES {
            return Err(nontransient_attempt_error(
                "The Murf Gen2 audio payload is unexpectedly large",
            ));
        }
        self.ensure_active(operation_id)
            .map_err(local_attempt_error)?;
        decode_cloud_pcm_response(StatusCode::OK, None, &decoded)
    }

    async fn wait_for_cancellation(&self, operation_id: u64) {
        while self.active_operation_id.load(Ordering::SeqCst) == operation_id {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}

async fn bounded_catalog_json(response: reqwest::Response) -> Result<Value> {
    let mut remaining_bytes = MAX_VOICE_CATALOG_BYTES;
    bounded_catalog_json_with_budget(response, &mut remaining_bytes).await
}

async fn bounded_catalog_json_with_budget(
    response: reqwest::Response,
    remaining_bytes: &mut usize,
) -> Result<Value> {
    let status = response.status();
    if response
        .content_length()
        .is_some_and(|length| length > *remaining_bytes as u64)
    {
        return Err(anyhow!("The provider voice catalog is unexpectedly large"));
    }
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| anyhow!(safe_text(&error.to_string())))?;
        if bytes.len().saturating_add(chunk.len()) > *remaining_bytes {
            return Err(anyhow!("The provider voice catalog is unexpectedly large"));
        }
        bytes.extend_from_slice(&chunk);
    }
    *remaining_bytes = (*remaining_bytes).saturating_sub(bytes.len());
    if !status.is_success() {
        return Err(anyhow!(
            "Voice catalog refresh failed: {}",
            parse_provider_error(&bytes, status)
        ));
    }
    serde_json::from_slice(&bytes).context("The provider returned an invalid voice catalog")
}

async fn murf_voice_catalog(
    client: &reqwest::Client,
    api_key: &str,
    model: &str,
) -> Result<Vec<TtsVoiceCatalogEntry>> {
    let model = if model == "gen2" { "gen2" } else { "falcon-2" };
    let response = client
        .get(MURF_VOICES_URL)
        .query(&[("model", model)])
        .header("api-key", api_key)
        .send()
        .await
        .map_err(|error| anyhow!(safe_text(&error.to_string())))?;
    let json = bounded_catalog_json(response).await?;
    let values = json
        .as_array()
        .or_else(|| json.get("voices").and_then(Value::as_array))
        .ok_or_else(|| anyhow!("Murf returned an invalid voice catalog"))?;
    let mut seen = HashSet::new();
    let voices = values
        .iter()
        .filter_map(murf_catalog_entry)
        .filter(|voice| seen.insert(voice.id.clone()))
        .take(MAX_VOICE_CATALOG_ENTRIES)
        .collect::<Vec<_>>();
    if voices.is_empty() {
        Err(anyhow!("Murf returned no TTS voices for {model}"))
    } else {
        Ok(voices)
    }
}

fn murf_catalog_entry(value: &Value) -> Option<TtsVoiceCatalogEntry> {
    let id = value.get("voiceId")?.as_str()?.trim();
    if id.is_empty() {
        return None;
    }
    let display_name = value
        .get("displayName")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(id);
    let primary_locale = value
        .get("locale")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    let display_language = value
        .get("displayLanguage")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    let accent = value
        .get("accent")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    let mut locales = Vec::new();
    if let Some(supported) = value.get("supportedLocales").and_then(Value::as_object) {
        for (locale, details) in supported {
            let label = details
                .get("detail")
                .and_then(Value::as_str)
                .unwrap_or(locale)
                .trim()
                .to_string();
            let styles = json_string_array(details.get("availableStyles"));
            locales.push(TtsVoiceCatalogLocale {
                locale: locale.clone(),
                label,
                styles,
            });
        }
    }
    if locales.is_empty() && !primary_locale.is_empty() {
        locales.push(TtsVoiceCatalogLocale {
            locale: primary_locale.to_string(),
            label: if display_language.is_empty() {
                primary_locale.to_string()
            } else {
                display_language.to_string()
            },
            styles: json_string_array(value.get("availableStyles")),
        });
    }
    let group = [display_language, primary_locale, accent]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");
    Some(TtsVoiceCatalogEntry {
        id: id.to_string(),
        label: format!("{display_name} — {id}"),
        group: if group.is_empty() {
            "Other".to_string()
        } else {
            group
        },
        language: primary_locale.to_string(),
        gender: value
            .get("gender")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        description: value
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        locales,
    })
}

async fn elevenlabs_voice_catalog(
    client: &reqwest::Client,
    api_key: &str,
) -> Result<Vec<TtsVoiceCatalogEntry>> {
    let mut remaining_bytes = MAX_VOICE_CATALOG_BYTES;
    let mut next_page_token: Option<String> = None;
    let mut seen_tokens = HashSet::new();
    let mut seen_voices = HashSet::new();
    let mut voices = Vec::new();

    for page in 0..MAX_VOICE_CATALOG_PAGES {
        let mut request = client
            .get(ELEVENLABS_VOICES_URL)
            .query(&[("page_size", "100")])
            .header("xi-api-key", api_key);
        if let Some(token) = next_page_token.as_deref() {
            request = request.query(&[("next_page_token", token)]);
        }
        let response = request
            .send()
            .await
            .map_err(|error| anyhow!(safe_text(&error.to_string())))?;
        let json = bounded_catalog_json_with_budget(response, &mut remaining_bytes).await?;
        for value in json
            .get("voices")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(voice) = elevenlabs_catalog_entry(value) {
                if seen_voices.insert(voice.id.clone()) {
                    voices.push(voice);
                    if voices.len() >= MAX_VOICE_CATALOG_ENTRIES {
                        break;
                    }
                }
            }
        }
        if voices.len() >= MAX_VOICE_CATALOG_ENTRIES
            || !json.get("has_more").and_then(Value::as_bool).unwrap_or(false)
        {
            break;
        }
        if page + 1 >= MAX_VOICE_CATALOG_PAGES {
            return Err(anyhow!("ElevenLabs voice catalog exceeded the page limit"));
        }
        let token = json
            .get("next_page_token")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|token| !token.is_empty())
            .ok_or_else(|| anyhow!("ElevenLabs voice catalog omitted its next-page token"))?
            .to_string();
        if !seen_tokens.insert(token.clone()) {
            return Err(anyhow!("ElevenLabs voice catalog repeated a page token"));
        }
        next_page_token = Some(token);
    }
    if voices.is_empty() {
        Err(anyhow!("ElevenLabs returned no accessible TTS voices"))
    } else {
        Ok(voices)
    }
}

fn elevenlabs_catalog_entry(value: &Value) -> Option<TtsVoiceCatalogEntry> {
    let id = value.get("voice_id")?.as_str()?.trim();
    if id.is_empty() {
        return None;
    }
    let name = value
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or(id);
    let labels = value.get("labels").and_then(Value::as_object);
    let advertised_language = value
        .get("language")
        .and_then(Value::as_str)
        .or_else(|| labels.and_then(|labels| labels.get("language")?.as_str()))
        .unwrap_or_default()
        .trim();
    let language = advertised_language
        .split(['-', '_'])
        .next()
        .filter(|value| value.len() == 2 && value.bytes().all(|byte| byte.is_ascii_alphabetic()))
        .unwrap_or_default();
    let gender = labels
        .and_then(|labels| labels.get("gender"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    let mut locale_ids = HashSet::new();
    let mut locales = Vec::new();
    if let Some(verified_languages) = value.get("verified_languages").and_then(Value::as_array) {
        for verified in verified_languages {
            let verified_language = verified
                .get("language")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim();
            let advertised_locale = verified
                .get("locale")
                .and_then(Value::as_str)
                .unwrap_or(verified_language)
                .trim();
            let locale = if verified_language.is_empty() {
                advertised_locale
                    .split(['-', '_'])
                    .next()
                    .unwrap_or_default()
            } else {
                verified_language
            };
            if locale.is_empty() || !locale_ids.insert(locale.to_ascii_lowercase()) {
                continue;
            }
            let accent = verified
                .get("accent")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim();
            locales.push(TtsVoiceCatalogLocale {
                locale: locale.to_string(),
                label: [advertised_locale, accent]
                    .into_iter()
                    .filter(|value| !value.is_empty())
                    .collect::<Vec<_>>()
                    .join(" · "),
                styles: Vec::new(),
            });
        }
    }
    if locales.is_empty() && !language.is_empty() {
        locales.push(TtsVoiceCatalogLocale {
            locale: language.to_string(),
            label: language.to_string(),
            styles: Vec::new(),
        });
    }
    let primary_language = locales
        .first()
        .map(|locale| locale.locale.as_str())
        .filter(|locale| !locale.is_empty())
        .unwrap_or(language);
    Some(TtsVoiceCatalogEntry {
        id: id.to_string(),
        label: format!("{name} — {id}"),
        group: value
            .get("category")
            .and_then(Value::as_str)
            .unwrap_or("Other")
            .to_string(),
        language: primary_language.to_string(),
        gender: gender.to_string(),
        description: value
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        locales,
    })
}

async fn cartesia_voice_catalog(
    client: &reqwest::Client,
    api_key: &str,
) -> Result<Vec<TtsVoiceCatalogEntry>> {
    let mut remaining_bytes = MAX_VOICE_CATALOG_BYTES;
    let mut starting_after: Option<String> = None;
    let mut seen_cursors = HashSet::new();
    let mut seen_voices = HashSet::new();
    let mut voices = Vec::new();

    for page in 0..MAX_VOICE_CATALOG_PAGES {
        let mut request = client
            .get(CARTESIA_VOICES_URL)
            .query(&[("limit", "100")])
            .bearer_auth(api_key)
            .header("Cartesia-Version", CARTESIA_API_VERSION);
        if let Some(cursor) = starting_after.as_deref() {
            request = request.query(&[("starting_after", cursor)]);
        }
        let response = request
            .send()
            .await
            .map_err(|error| anyhow!(safe_text(&error.to_string())))?;
        let json = bounded_catalog_json_with_budget(response, &mut remaining_bytes).await?;
        for value in json
            .get("data")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(voice) = cartesia_catalog_entry(value) {
                if seen_voices.insert(voice.id.clone()) {
                    voices.push(voice);
                    if voices.len() >= MAX_VOICE_CATALOG_ENTRIES {
                        break;
                    }
                }
            }
        }
        if voices.len() >= MAX_VOICE_CATALOG_ENTRIES
            || !json.get("has_more").and_then(Value::as_bool).unwrap_or(false)
        {
            break;
        }
        if page + 1 >= MAX_VOICE_CATALOG_PAGES {
            return Err(anyhow!("Cartesia voice catalog exceeded the page limit"));
        }
        let cursor = json
            .get("next_page")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|cursor| !cursor.is_empty())
            .ok_or_else(|| anyhow!("Cartesia voice catalog omitted its next-page cursor"))?
            .to_string();
        if !seen_cursors.insert(cursor.clone()) {
            return Err(anyhow!("Cartesia voice catalog repeated a page cursor"));
        }
        starting_after = Some(cursor);
    }
    if voices.is_empty() {
        Err(anyhow!("Cartesia returned no accessible TTS voices"))
    } else {
        Ok(voices)
    }
}

fn cartesia_catalog_entry(value: &Value) -> Option<TtsVoiceCatalogEntry> {
    let id = value.get("id")?.as_str()?.trim();
    if id.is_empty() {
        return None;
    }
    let name = value
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or(id);
    let language = value
        .get("language")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    let country = value
        .get("country")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    let group = [language, country]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");
    let mut locale_ids = HashSet::new();
    let mut locales = value
        .get("locales")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.get("locale").and_then(Value::as_str))
        .map(str::trim)
        .filter(|locale| !locale.is_empty())
        .filter(|locale| locale_ids.insert(locale.to_ascii_lowercase()))
        .map(|locale| TtsVoiceCatalogLocale {
            locale: locale.to_string(),
            label: locale.to_string(),
            styles: Vec::new(),
        })
        .collect::<Vec<_>>();
    if locales.is_empty() && !language.is_empty() {
        locales.push(TtsVoiceCatalogLocale {
            locale: language.to_string(),
            label: language.to_string(),
            styles: Vec::new(),
        });
    }
    Some(TtsVoiceCatalogEntry {
        id: id.to_string(),
        label: format!("{name} — {id}"),
        group: if group.is_empty() {
            "Other".to_string()
        } else {
            group
        },
        language: language.to_string(),
        gender: value
            .get("gender")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        description: value
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        locales,
    })
}

fn json_string_array(value: Option<&Value>) -> Vec<String> {
    let mut seen = HashSet::new();
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .filter(|value| seen.insert((*value).to_string()))
        .map(str::to_string)
        .collect()
}

fn deepgram_catalog_entry(value: &Value) -> Option<TtsVoiceCatalogEntry> {
    let id = value.get("canonical_name")?.as_str()?.trim();
    if id.is_empty() {
        return None;
    }
    let name = value
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or(id);
    let languages = value
        .get("languages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|language| !language.is_empty())
        .collect::<Vec<_>>();
    let tags = value
        .pointer("/metadata/tags")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    let gender = tags
        .iter()
        .copied()
        .find(|tag| matches!(*tag, "feminine" | "masculine" | "neutral"))
        .unwrap_or_default();
    let accent = value
        .pointer("/metadata/accent")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let group = if languages.is_empty() {
        "Other".to_string()
    } else if accent.is_empty() {
        languages.join(", ")
    } else {
        format!("{} · {accent}", languages.join(", "))
    };
    Some(TtsVoiceCatalogEntry {
        id: id.to_string(),
        label: format!("{name} — {id}"),
        group,
        language: languages.first().copied().unwrap_or_default().to_string(),
        gender: gender.to_string(),
        description: tags.join(", "),
        locales: Vec::new(),
    })
}

fn soniox_catalog_entry(value: &Value) -> Option<TtsVoiceCatalogEntry> {
    let id = value.get("id")?.as_str()?.trim();
    if id.is_empty() {
        return None;
    }
    let name = value
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or(id);
    let statuses = value
        .get("models")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|model| {
            Some(format!(
                "{}: {}",
                model.get("model")?.as_str()?,
                model.get("status")?.as_str()?
            ))
        })
        .collect::<Vec<_>>();
    Some(TtsVoiceCatalogEntry {
        id: id.to_string(),
        label: format!("{name} — {id}"),
        group: "Custom voices".to_string(),
        language: String::new(),
        gender: String::new(),
        description: statuses.join(", "),
        locales: Vec::new(),
    })
}

fn openai_voice_catalog() -> TtsVoiceCatalog {
    let voices = [
        "alloy", "ash", "ballad", "cedar", "coral", "echo", "fable", "marin", "nova", "onyx",
        "sage", "shimmer", "verse",
    ]
    .into_iter()
    .map(|voice| TtsVoiceCatalogEntry {
        id: voice.to_string(),
        label: voice.to_string(),
        group: "Built-in voices".to_string(),
        language: String::new(),
        gender: String::new(),
        description: String::new(),
        locales: Vec::new(),
    })
    .collect();
    TtsVoiceCatalog {
        provider: TtsProvider::OpenAi,
        voices,
        source: "builtin".to_string(),
        supports_live_refresh: false,
        replace_builtin: true,
        warning: Some(
            "OpenAI does not expose a list endpoint for built-in TTS voices; this catalog follows the documented speech API."
                .to_string(),
        ),
    }
}

pub fn semantic_chunks(text: &str, requested_target: usize, hard_limit: usize) -> Vec<TtsChunk> {
    if text.trim().is_empty() || hard_limit == 0 {
        return Vec::new();
    }
    let chars: Vec<char> = text.chars().collect();
    let target = requested_target.max(1).min(hard_limit);
    let mut chunks = Vec::new();
    let mut start = 0;

    while start < chars.len() {
        let upper = (start + target).min(chars.len());
        let (end, boundary) = if upper == chars.len() {
            (upper, TtsBoundary::End)
        } else {
            choose_boundary(&chars, start, upper)
        };
        let end = end
            .max(start + 1)
            .min((start + hard_limit).min(chars.len()));
        let chunk_text: String = chars[start..end].iter().collect();
        if !chunk_text.trim().is_empty() {
            chunks.push(TtsChunk {
                index: chunks.len() + 1,
                character_count: chunk_text.chars().count(),
                text: chunk_text,
                boundary_after: boundary,
            });
        }
        start = end;
    }
    chunks
}

fn choose_boundary(chars: &[char], start: usize, upper: usize) -> (usize, TtsBoundary) {
    if let Some(end) = paragraph_boundary(chars, start, upper) {
        return (end, TtsBoundary::Paragraph);
    }
    if let Some(end) = last_boundary(chars, start, upper, is_sentence_boundary) {
        return (end, TtsBoundary::Sentence);
    }
    if let Some(end) = last_boundary(chars, start, upper, is_clause_boundary) {
        return (end, TtsBoundary::Clause);
    }
    if let Some(end) = last_boundary(chars, start, upper, char::is_whitespace) {
        return (end, TtsBoundary::Whitespace);
    }
    (upper, TtsBoundary::Hard)
}

fn paragraph_boundary(chars: &[char], start: usize, upper: usize) -> Option<usize> {
    let mut last_newline = None;
    for index in start..upper {
        if chars[index] == '\n' {
            if let Some(previous) = last_newline {
                if chars[previous + 1..index]
                    .iter()
                    .all(|ch| ch.is_whitespace())
                {
                    // Paragraphs are kept in separate provider requests so the
                    // assembler can apply the configured paragraph pause.
                    return Some(index + 1);
                }
            }
            last_newline = Some(index);
        } else if !chars[index].is_whitespace() {
            last_newline = None;
        }
    }
    None
}

fn last_boundary(
    chars: &[char],
    start: usize,
    upper: usize,
    predicate: impl Fn(char) -> bool,
) -> Option<usize> {
    (start..upper)
        .rev()
        .find(|&index| {
            predicate(chars[index])
                && chars[start..=index]
                    .iter()
                    .any(|character| !character.is_whitespace())
        })
        .map(|index| index + 1)
}

fn is_sentence_boundary(ch: char) -> bool {
    matches!(ch, '.' | '!' | '?' | '。' | '！' | '？' | '…' | '\n')
}

fn is_clause_boundary(ch: char) -> bool {
    matches!(ch, ',' | ';' | ':' | '，' | '、' | '；' | '：' | '—' | '–')
}

fn resolve_api_key(settings: &TtsSettings) -> Result<String> {
    if !settings.provider.requires_api_key() {
        return Ok(String::new());
    }
    let source = match settings.provider {
        TtsProvider::Soniox => settings.soniox_key_source,
        TtsProvider::Deepgram => settings.deepgram_key_source,
        TtsProvider::OpenAi => settings.openai_key_source,
        TtsProvider::OpenAiCompatible => settings.openai_compatible_key_source,
        TtsProvider::Murf => settings.murf_key_source,
        TtsProvider::ElevenLabs => settings.elevenlabs_key_source,
        TtsProvider::Cartesia => settings.cartesia_key_source,
        TtsProvider::Edge => unreachable!("handled above"),
        TtsProvider::LocalQwen => unreachable!("handled above"),
        TtsProvider::LocalKokoro => unreachable!("handled above"),
        TtsProvider::Windows => unreachable!("handled above"),
    };
    let key = match (settings.provider, source) {
        (provider, TtsKeySource::Separate) => {
            crate::secure_keys::get_tts_api_key(provider.as_str())
        }
        (TtsProvider::Soniox, TtsKeySource::Shared) => crate::secure_keys::get_soniox_api_key(),
        (TtsProvider::Deepgram, TtsKeySource::Shared) => crate::secure_keys::get_deepgram_api_key(),
        (
            TtsProvider::OpenAi | TtsProvider::OpenAiCompatible,
            TtsKeySource::Shared,
        ) => {
            crate::secure_keys::get_post_process_api_key("openai")
        }
        (
            TtsProvider::Murf | TtsProvider::ElevenLabs | TtsProvider::Cartesia,
            TtsKeySource::Shared,
        ) => {
            return Err(anyhow!(
                "{} always uses its separately stored TTS key",
                provider_name(settings.provider)
            ));
        }
        (TtsProvider::Edge, _) => unreachable!("handled above"),
        (TtsProvider::LocalQwen, _) => unreachable!("handled above"),
        (TtsProvider::LocalKokoro, _) => unreachable!("handled above"),
        (TtsProvider::Windows, _) => unreachable!("handled above"),
    };
    let key = key.trim();
    if key.is_empty() {
        if settings.provider == TtsProvider::OpenAiCompatible {
            // Custom OpenAI-compatible servers (e.g. local Kokoro-FastAPI) may
            // not require authentication; synthesize without an Authorization
            // header when no key is configured.
            return Ok(String::new());
        }
        Err(anyhow!(
            "No {} API key is configured for text-to-speech",
            provider_name(settings.provider)
        ))
    } else if settings.provider == TtsProvider::Soniox
        && key.chars().count() > SONIOX_TTS_API_KEY_MAX_CHARS
    {
        Err(anyhow!(
            "Soniox TTS API key must not exceed {} characters; received {}",
            SONIOX_TTS_API_KEY_MAX_CHARS,
            key.chars().count()
        ))
    } else {
        Ok(key.to_string())
    }
}

fn provider_name(provider: TtsProvider) -> &'static str {
    match provider {
        TtsProvider::Soniox => "Soniox",
        TtsProvider::Deepgram => "Deepgram",
        TtsProvider::OpenAi => "OpenAI",
        TtsProvider::OpenAiCompatible => "OpenAI-compatible",
        TtsProvider::Murf => "Murf AI",
        TtsProvider::ElevenLabs => "ElevenLabs",
        TtsProvider::Cartesia => "Cartesia",
        TtsProvider::Edge => "Microsoft Read Aloud (unofficial)",
        TtsProvider::LocalQwen => "Local Qwen3-TTS",
        TtsProvider::LocalKokoro => "Local Kokoro",
        TtsProvider::Windows => "Windows voices",
    }
}

fn interactive_pause_after(chunk: &TtsChunk, total_chunks: usize, settings: &TtsSettings) -> u32 {
    if chunk.index >= total_chunks {
        0
    } else if chunk.boundary_after == TtsBoundary::Paragraph {
        settings.paragraph_pause_ms.min(10_000)
    } else {
        settings.inter_chunk_pause_ms.min(5_000)
    }
}

fn ensure_enabled(settings: &TtsSettings) -> Result<()> {
    if !settings.enabled {
        return Err(anyhow!("Text-to-speech is disabled"));
    }
    TtsManager::validate_settings(settings)?;
    Ok(())
}

fn validate_enabled_independent_settings(settings: &TtsSettings) -> Result<()> {
    TtsManager::validate_settings(settings)
}

fn validate_max_chars(label: &str, value: &str, maximum: usize) -> Result<()> {
    let count = value.chars().count();
    if count > maximum {
        Err(anyhow!(
            "{label} must not exceed {maximum} characters; received {count}"
        ))
    } else {
        Ok(())
    }
}

fn validate_required_max_chars(label: &str, value: &str, maximum: usize) -> Result<()> {
    if value.trim().is_empty() {
        return Err(anyhow!("{label} cannot be empty"));
    }
    validate_max_chars(label, value, maximum)
}

fn validate_iso_639_1_code(label: &str, value: &str) -> Result<()> {
    let value = value.trim();
    if value.len() == 2 && value.bytes().all(|byte| byte.is_ascii_alphabetic()) {
        Ok(())
    } else {
        Err(anyhow!("{label} must be a two-letter ISO 639-1 code"))
    }
}

fn validate_finite_range(label: &str, value: f32, minimum: f32, maximum: f32) -> Result<()> {
    if !value.is_finite() || value < minimum || value > maximum {
        Err(anyhow!(
            "{label} must be a finite number between {minimum} and {maximum}"
        ))
    } else {
        Ok(())
    }
}

fn nonempty_or<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.trim().is_empty() {
        fallback
    } else {
        value.trim()
    }
}

fn elevenlabs_normalization_value(value: ElevenLabsTextNormalization) -> &'static str {
    match value {
        ElevenLabsTextNormalization::Auto => "auto",
        ElevenLabsTextNormalization::On => "on",
        ElevenLabsTextNormalization::Off => "off",
    }
}

fn network_error(error: reqwest::Error) -> ProviderAttemptError {
    let transient =
        error.is_timeout() || error.is_connect() || error.is_request() || error.is_body();
    ProviderAttemptError {
        status: error.status(),
        safe_message: safe_text(&error.to_string()),
        transient,
        retry_after: None,
    }
}

fn local_attempt_error(error: anyhow::Error) -> ProviderAttemptError {
    ProviderAttemptError {
        status: None,
        safe_message: safe_text(&error.to_string()),
        transient: false,
        retry_after: None,
    }
}

fn attempt_status_label(provider: TtsProvider, status: Option<StatusCode>) -> String {
    status.map_or_else(
        || {
            if provider.is_local_or_system() {
                "local".to_string()
            } else {
                "network".to_string()
            }
        },
        |value| value.as_u16().to_string(),
    )
}

fn cancelled_attempt_error() -> ProviderAttemptError {
    ProviderAttemptError {
        status: None,
        safe_message: "Text-to-speech operation cancelled".to_string(),
        transient: false,
        retry_after: None,
    }
}

fn nontransient_attempt_error(message: impl Into<String>) -> ProviderAttemptError {
    ProviderAttemptError {
        status: None,
        safe_message: message.into(),
        transient: false,
        retry_after: None,
    }
}

fn try_reserve_foreground_operation_lock(
    lock: Arc<tokio::sync::Mutex<()>>,
) -> Result<tokio::sync::OwnedMutexGuard<()>> {
    lock.try_lock_owned()
        .map_err(|_| anyhow!("Another text-to-speech operation is already running"))
}

fn operation_is_active(active_operation_id: &AtomicU64, operation_id: u64) -> bool {
    active_operation_id.load(Ordering::SeqCst) == operation_id
}

fn try_cancel_operation(
    active_operation_id: &AtomicU64,
    current: &TtsState,
    operation_id: u64,
) -> bool {
    if current.operation_id != operation_id
        || !matches!(
            current.phase,
            TtsPhase::Preparing
                | TtsPhase::Preprocessing
                | TtsPhase::Synthesizing
                | TtsPhase::Retrying
                | TtsPhase::Ready
        )
    {
        return false;
    }
    active_operation_id
        .compare_exchange(
            operation_id,
            operation_id.saturating_add(1),
            Ordering::SeqCst,
            Ordering::SeqCst,
        )
        .is_ok()
}

fn decode_cloud_pcm_response(
    status: StatusCode,
    retry_after: Option<Duration>,
    bytes: &[u8],
) -> std::result::Result<Vec<i16>, ProviderAttemptError> {
    if !status.is_success() {
        let message = parse_provider_error(bytes, status);
        return Err(ProviderAttemptError {
            status: Some(status),
            transient: is_transient_status(status, &message),
            safe_message: message,
            retry_after,
        });
    }

    decode_pcm_s16_le(bytes).map_err(|error| ProviderAttemptError {
        status: Some(status),
        safe_message: error.to_string(),
        transient: false,
        retry_after: None,
    })
}

fn publish_complete_pcm_segments(
    bytes: &[u8],
    emitted_bytes: &mut usize,
    segment_bytes: usize,
    callback: &mut PcmSegmentSink<'_>,
) -> std::result::Result<(), ProviderAttemptError> {
    debug_assert!(segment_bytes > 0 && segment_bytes % 2 == 0);
    while bytes.len().saturating_sub(*emitted_bytes) >= segment_bytes {
        let end = (*emitted_bytes).saturating_add(segment_bytes);
        let pcm = decode_cloud_pcm_response(StatusCode::OK, None, &bytes[*emitted_bytes..end])?;
        callback(&pcm)?;
        *emitted_bytes = end;
    }
    Ok(())
}

fn decode_pcm_s16_le(bytes: &[u8]) -> Result<Vec<i16>> {
    if bytes.is_empty() {
        return Err(anyhow!("Provider returned empty PCM audio"));
    }
    if bytes.len() % 2 != 0 {
        return Err(anyhow!("Provider returned malformed 16-bit PCM audio"));
    }
    Ok(bytes
        .chunks_exact(2)
        .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
        .collect())
}

fn is_transient_status(status: StatusCode, message: &str) -> bool {
    if matches!(
        status,
        StatusCode::UNAUTHORIZED
            | StatusCode::FORBIDDEN
            | StatusCode::PAYMENT_REQUIRED
            | StatusCode::BAD_REQUEST
            | StatusCode::NOT_FOUND
            | StatusCode::UNPROCESSABLE_ENTITY
    ) {
        return false;
    }
    let lower = message.to_lowercase();
    if lower.contains("quota exhausted")
        || lower.contains("insufficient_quota")
        || (lower.contains("exceeded") && lower.contains("quota"))
        || lower.contains("monthly budget")
        || lower.contains("balance has been reached")
    {
        return false;
    }
    status == StatusCode::REQUEST_TIMEOUT
        || status == StatusCode::TOO_MANY_REQUESTS
        || status.is_server_error()
}

fn parse_retry_after(value: Option<&reqwest::header::HeaderValue>) -> Option<Duration> {
    let text = value?.to_str().ok()?.trim();
    if let Ok(seconds) = text.parse::<u64>() {
        return Some(Duration::from_secs(seconds).min(MAX_RETRY_DELAY));
    }
    let date = chrono::DateTime::parse_from_rfc2822(text).ok()?;
    let now = chrono::Utc::now();
    let wait = date.with_timezone(&chrono::Utc).signed_duration_since(now);
    wait.to_std().ok().map(|value| value.min(MAX_RETRY_DELAY))
}

fn exponential_delay(base_ms: u32, attempt: u8) -> Duration {
    let exponent = u32::from(attempt.saturating_sub(1)).min(10);
    Duration::from_millis(u64::from(base_ms.max(1)).saturating_mul(1_u64 << exponent))
        .min(MAX_RETRY_DELAY)
}

fn read_supported_text_file(path: &Path) -> Result<(String, String)> {
    let file =
        File::open(path).with_context(|| format!("Failed to open text file {}", path.display()))?;
    let bytes = read_tts_text_bytes_bounded(file, path)?;
    decode_supported_text_bytes(bytes)
}

fn read_tts_text_bytes_bounded(mut reader: impl Read, path: &Path) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .by_ref()
        .take(MAX_TTS_TEXT_INPUT_BYTES.saturating_add(1) as u64)
        .read_to_end(&mut bytes)
        .with_context(|| format!("Failed to read text file {}", path.display()))?;
    if bytes.len() > MAX_TTS_TEXT_INPUT_BYTES {
        return Err(anyhow!(
            "TTS text input exceeds the 8 MiB safety limit: {}",
            path.display()
        ));
    }
    Ok(bytes)
}

fn decode_supported_text_bytes(mut bytes: Vec<u8>) -> Result<(String, String)> {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        bytes.drain(..3);
        return Ok((
            String::from_utf8(bytes).context("Invalid UTF-8 text after BOM")?,
            "utf-8-bom".to_string(),
        ));
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return Ok((decode_utf16(&bytes[2..], true)?, "utf-16-le".to_string()));
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return Ok((decode_utf16(&bytes[2..], false)?, "utf-16-be".to_string()));
    }
    Ok((
        String::from_utf8(bytes)
            .context("Unsupported text encoding; use UTF-8 or BOM-marked UTF-16 LE/BE")?,
        "utf-8".to_string(),
    ))
}

fn decode_utf16(bytes: &[u8], little_endian: bool) -> Result<String> {
    if bytes.len() % 2 != 0 {
        return Err(anyhow!("Malformed UTF-16 file: odd byte length"));
    }
    let units = bytes.chunks_exact(2).map(|pair| {
        if little_endian {
            u16::from_le_bytes([pair[0], pair[1]])
        } else {
            u16::from_be_bytes([pair[0], pair[1]])
        }
    });
    std::char::decode_utf16(units)
        .map(|item| item.map_err(|_| anyhow!("Malformed UTF-16 surrogate pair")))
        .collect()
}

fn validate_output_extension(path: &Path, format: TtsOutputFormat) -> Result<()> {
    let expected = match format {
        TtsOutputFormat::Mp3 => "mp3",
        TtsOutputFormat::Wav => "wav",
    };
    let actual = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(anyhow!(
            "Output path must use the .{} extension for the selected format",
            expected
        ))
    }
}

fn validate_output_settings(settings: &TtsSettings) -> Result<()> {
    if settings.output_format == TtsOutputFormat::Mp3
        && !SUPPORTED_MP3_BITRATES.contains(&settings.mp3_bitrate_kbps)
    {
        return Err(anyhow!(
            "Unsupported MP3 bitrate: {} kb/s",
            settings.mp3_bitrate_kbps
        ));
    }
    Ok(())
}

fn validate_input_extension(path: &Path) -> Result<()> {
    if is_supported_text_path(path) {
        Ok(())
    } else {
        Err(anyhow!(
            "Unsupported text input {}; use a .txt or .md file",
            path.display()
        ))
    }
}

fn unique_watched_output_path(
    output_dir: &Path,
    input_path: &Path,
    output_extension: &str,
) -> Result<PathBuf> {
    let source_name = input_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| anyhow!("Watched text file has no valid file name"))?;
    let first = output_dir.join(format!("{source_name}.{output_extension}"));
    if !first.exists() {
        return Ok(first);
    }
    for index in 2..=10_000 {
        let candidate = output_dir.join(format!("{source_name}-{index}.{output_extension}"));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(anyhow!(
        "Could not allocate a unique output name for {}",
        input_path.display()
    ))
}

fn watcher_resume_task_allowed(
    task: &WatcherResumeTask,
    input_root: &Path,
    output_root: &Path,
    recursive: bool,
    output_format: TtsOutputFormat,
) -> bool {
    if task.output_path.exists() || !is_supported_text_path(&task.source_path) {
        return false;
    }
    let expected_extension = match output_format {
        TtsOutputFormat::Mp3 => "mp3",
        TtsOutputFormat::Wav => "wav",
    };
    if !task
        .output_path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case(expected_extension))
    {
        return false;
    }
    let Ok(output_parent) =
        fs::canonicalize(task.output_path.parent().unwrap_or_else(|| Path::new(".")))
    else {
        return false;
    };
    if output_parent != output_root {
        return false;
    }
    let Ok(source) = fs::canonicalize(&task.source_path) else {
        return false;
    };
    let Some(source_parent) = source.parent() else {
        return false;
    };
    if recursive {
        source.starts_with(input_root) && source != input_root
    } else {
        source_parent == input_root
    }
}

fn is_supported_text_path(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case("txt") || value.eq_ignore_ascii_case("md"))
        .unwrap_or(false)
}

fn metadata_is_reparse_point(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        let _ = metadata;
        false
    }
}

fn canonicalize_batch_directory(path: &Path, label: &str) -> Result<PathBuf> {
    if path.as_os_str().is_empty() {
        return Err(anyhow!("The {label} is not configured"));
    }
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("Failed to inspect {label} {}", path.display()))?;
    if metadata.file_type().is_symlink() || metadata_is_reparse_point(&metadata) {
        return Err(anyhow!(
            "Refusing reparse-point or symlink {label}: {}",
            path.display()
        ));
    }
    if !metadata.is_dir() {
        return Err(anyhow!(
            "The {label} is not a directory: {}",
            path.display()
        ));
    }
    fs::canonicalize(path).with_context(|| format!("Failed to resolve {label} {}", path.display()))
}

fn relative_path_is_safe(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn normalized_batch_path_key(path: &Path) -> String {
    let value = path.to_string_lossy();
    #[cfg(windows)]
    {
        value.to_ascii_lowercase()
    }
    #[cfg(not(windows))]
    {
        value.into_owned()
    }
}

fn collect_batch_text_paths(
    input_root: &Path,
    output_root: &Path,
    recursive: bool,
    paths: &mut Vec<PathBuf>,
    warnings: &mut Vec<String>,
) -> Result<()> {
    if output_root == input_root {
        return Ok(());
    }
    let excluded_output = output_root
        .starts_with(input_root)
        .then(|| output_root.to_path_buf());
    let mut pending = vec![input_root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if directory != input_root => {
                warnings.push(format!(
                    "Could not scan {}: {}",
                    directory.display(),
                    safe_text(&error.to_string())
                ));
                continue;
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("Failed to scan {}", directory.display()))
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    warnings.push(format!(
                        "Could not read an entry in {}: {}",
                        directory.display(),
                        safe_text(&error.to_string())
                    ));
                    continue;
                }
            };
            let path = entry.path();
            let metadata = match fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(error) => {
                    warnings.push(format!(
                        "Could not inspect {}: {}",
                        path.display(),
                        safe_text(&error.to_string())
                    ));
                    continue;
                }
            };
            if metadata.file_type().is_symlink() || metadata_is_reparse_point(&metadata) {
                continue;
            }
            if metadata.is_dir() {
                if !recursive {
                    continue;
                }
                let canonical = match fs::canonicalize(&path) {
                    Ok(canonical) => canonical,
                    Err(error) => {
                        warnings.push(format!(
                            "Could not resolve {}: {}",
                            path.display(),
                            safe_text(&error.to_string())
                        ));
                        continue;
                    }
                };
                if !canonical.starts_with(input_root)
                    || excluded_output
                        .as_deref()
                        .is_some_and(|output| canonical.starts_with(output))
                {
                    continue;
                }
                pending.push(canonical);
            } else if metadata.is_file() && is_supported_text_path(&path) {
                paths.push(path);
                if paths.len() > MAX_TTS_BATCH_FILES {
                    return Err(anyhow!(
                        "The selected folder contains more than {MAX_TTS_BATCH_FILES} supported files"
                    ));
                }
            }
        }
    }
    Ok(())
}

fn allocate_batch_output_path(
    output_root: &Path,
    source_relative: &Path,
    output_extension: &str,
    reserved: &mut HashSet<String>,
) -> Result<PathBuf> {
    if !relative_path_is_safe(source_relative) {
        return Err(anyhow!(
            "Unsafe relative batch input path: {}",
            source_relative.display()
        ));
    }
    let mut relative_output = source_relative.to_path_buf();
    relative_output.set_extension(output_extension);
    let parent = relative_output.parent().unwrap_or_else(|| Path::new(""));
    let stem = relative_output
        .file_stem()
        .ok_or_else(|| anyhow!("Batch input has no valid file stem"))?;
    let extension = relative_output
        .extension()
        .ok_or_else(|| anyhow!("Batch output has no extension"))?;

    for index in 1..=10_000 {
        let file_name = if index == 1 {
            relative_output
                .file_name()
                .expect("relative output has a file name")
                .to_os_string()
        } else {
            let mut name = stem.to_os_string();
            name.push(format!("-{index}."));
            name.push(extension);
            name
        };
        let candidate = output_root.join(parent).join(file_name);
        let key = normalized_batch_path_key(&candidate);
        if !candidate.exists() && reserved.insert(key) {
            return Ok(candidate);
        }
    }
    Err(anyhow!(
        "Could not allocate a collision-safe output name for {}",
        source_relative.display()
    ))
}

fn read_batch_input_no_follow(
    path: &Path,
    configured_root: Option<&Path>,
    recursive: bool,
) -> Result<(PathBuf, String)> {
    validate_input_extension(path)?;
    let canonical_root = configured_root.map(Path::to_path_buf);
    let canonical_parent = fs::canonicalize(
        path.parent()
            .ok_or_else(|| anyhow!("Batch TTS input has no parent directory"))?,
    )
    .with_context(|| {
        format!(
            "Failed to resolve batch input parent for {}",
            path.display()
        )
    })?;
    if let Some(root) = canonical_root.as_deref() {
        let parent_is_allowed = if recursive {
            canonical_parent.starts_with(root)
        } else {
            canonical_parent == root
        };
        if !parent_is_allowed {
            return Err(anyhow!(
                "Refusing batch TTS input outside the selected folder: {}",
                path.display()
            ));
        }
    }

    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = options
        .open(path)
        .with_context(|| format!("Failed to safely open batch input {}", path.display()))?;
    let metadata = file
        .metadata()
        .with_context(|| format!("Failed to inspect batch input handle {}", path.display()))?;
    if !metadata.is_file() {
        return Err(anyhow!(
            "Batch TTS input is not a regular file: {}",
            path.display()
        ));
    }
    if metadata_is_reparse_point(&metadata) {
        return Err(anyhow!(
            "Refusing reparse-point batch TTS input: {}",
            path.display()
        ));
    }

    let canonical_path = fs::canonicalize(path)
        .with_context(|| format!("Failed to resolve batch input {}", path.display()))?;
    if let Some(root) = canonical_root.as_deref() {
        if !canonical_path.starts_with(root) || canonical_path == root {
            return Err(anyhow!(
                "Refusing batch TTS input outside the selected folder: {}",
                path.display()
            ));
        }
    }
    let bytes = read_tts_text_bytes_bounded(file, path)
        .with_context(|| format!("Failed to read batch input {}", path.display()))?;
    let (source, _encoding) = decode_supported_text_bytes(bytes)
        .with_context(|| format!("Failed to decode batch input {}", path.display()))?;
    Ok((canonical_path, source))
}

fn prepare_batch_output_path(
    output_path: &Path,
    configured_root: &Path,
    source_relative: &Path,
    settings: &TtsSettings,
) -> Result<()> {
    validate_output_extension(output_path, settings.output_format)?;
    validate_output_settings(settings)?;
    let output_root = canonicalize_batch_directory(configured_root, "batch output directory")?;
    let relative_output = output_path.strip_prefix(&output_root).map_err(|_| {
        anyhow!(
            "Refusing batch output outside the selected output folder: {}",
            output_path.display()
        )
    })?;
    if !relative_path_is_safe(relative_output) {
        return Err(anyhow!(
            "Refusing unsafe batch output path: {}",
            output_path.display()
        ));
    }
    let source_parent = source_relative.parent().unwrap_or_else(|| Path::new(""));
    let output_parent = relative_output.parent().unwrap_or_else(|| Path::new(""));
    if source_parent != output_parent {
        return Err(anyhow!(
            "Batch output no longer matches the scanned relative folder: {}",
            output_path.display()
        ));
    }

    let mut current = output_root.clone();
    for component in output_parent.components() {
        let Component::Normal(component) = component else {
            return Err(anyhow!("Refusing unsafe batch output directory"));
        };
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if !metadata.is_dir()
                    || metadata.file_type().is_symlink()
                    || metadata_is_reparse_point(&metadata)
                {
                    return Err(anyhow!(
                        "Refusing unsafe batch output directory: {}",
                        current.display()
                    ));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                match fs::create_dir(&current) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(error) => {
                        return Err(error).with_context(|| {
                            format!("Failed to create batch output folder {}", current.display())
                        })
                    }
                }
                let metadata = fs::symlink_metadata(&current).with_context(|| {
                    format!("Failed to verify batch output folder {}", current.display())
                })?;
                if !metadata.is_dir()
                    || metadata.file_type().is_symlink()
                    || metadata_is_reparse_point(&metadata)
                {
                    return Err(anyhow!(
                        "Refusing unsafe batch output directory: {}",
                        current.display()
                    ));
                }
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "Failed to inspect batch output folder {}",
                        current.display()
                    )
                })
            }
        }
    }
    let canonical_parent = fs::canonicalize(output_path.parent().unwrap_or(&output_root))
        .with_context(|| {
            format!(
                "Failed to resolve batch output parent {}",
                output_path.display()
            )
        })?;
    if !canonical_parent.starts_with(&output_root) {
        return Err(anyhow!(
            "Refusing batch output outside the selected output folder: {}",
            output_path.display()
        ));
    }
    if output_path.exists() {
        return Err(output_already_exists_error(output_path));
    }
    Ok(())
}

fn collect_supported_text_paths(root: &Path, recursive: bool) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let entries = fs::read_dir(&directory)
            .with_context(|| format!("Failed to scan {}", directory.display()))?;
        for entry in entries {
            let entry = entry
                .with_context(|| format!("Failed to read an entry in {}", directory.display()))?;
            let file_type = entry.file_type().with_context(|| {
                format!("Failed to inspect watched path {}", entry.path().display())
            })?;
            if file_type.is_symlink() {
                continue;
            }
            let path = entry.path();
            if file_type.is_dir() {
                if recursive {
                    pending.push(path);
                }
            } else if file_type.is_file() && is_supported_text_path(&path) {
                paths.push(path);
            }
        }
    }
    Ok(paths)
}

fn read_watched_input_no_follow(
    path: &Path,
    configured_root: &Path,
    recursive: bool,
) -> Result<(PathBuf, String)> {
    let canonical_root = fs::canonicalize(configured_root).with_context(|| {
        format!(
            "Failed to resolve configured TTS watch folder {}",
            configured_root.display()
        )
    })?;
    let canonical_parent = fs::canonicalize(
        path.parent()
            .ok_or_else(|| anyhow!("Watched TTS input has no parent directory"))?,
    )
    .with_context(|| {
        format!(
            "Failed to resolve watched input parent for {}",
            path.display()
        )
    })?;
    let parent_is_allowed = if recursive {
        canonical_parent.starts_with(&canonical_root)
    } else {
        canonical_parent == canonical_root
    };
    if !parent_is_allowed {
        return Err(anyhow!(
            "Refusing TTS watch input outside the configured folder: {}",
            path.display()
        ));
    }

    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = options
        .open(path)
        .with_context(|| format!("Failed to safely open watched input {}", path.display()))?;
    let metadata = file
        .metadata()
        .with_context(|| format!("Failed to inspect watched input handle {}", path.display()))?;
    if !metadata.is_file() {
        return Err(anyhow!(
            "Watched TTS input is not a regular file: {}",
            path.display()
        ));
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(anyhow!(
                "Refusing reparse-point TTS watch input: {}",
                path.display()
            ));
        }
    }

    let canonical_path = fs::canonicalize(path)
        .with_context(|| format!("Failed to resolve watched input {}", path.display()))?;
    if !canonical_path.starts_with(&canonical_root) || canonical_path == canonical_root {
        return Err(anyhow!(
            "Refusing TTS watch input outside the configured folder: {}",
            path.display()
        ));
    }
    let bytes = read_tts_text_bytes_bounded(file, path)
        .with_context(|| format!("Failed to read watched input {}", path.display()))?;
    let (source, _encoding) = decode_supported_text_bytes(bytes)
        .with_context(|| format!("Failed to decode watched input {}", path.display()))?;
    Ok((canonical_path, source))
}

fn normalize_source_text(path: &Path, source: &str) -> String {
    let is_markdown = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case("md"))
        .unwrap_or(false);
    if is_markdown {
        normalize_markdown_for_speech(source)
    } else {
        source.to_string()
    }
}

/// Renders Markdown into the readable text a person would normally speak.
/// Link labels, headings, lists, tables, and code content remain; URL targets,
/// HTML, and formatting syntax do not.
pub fn normalize_markdown_for_speech(markdown: &str) -> String {
    use pulldown_cmark::{Event, Options, Parser, TagEnd};

    let normalized_newlines = markdown.replace("\r\n", "\n").replace('\r', "\n");
    let without_frontmatter = if let Some(rest) = normalized_newlines.strip_prefix("---\n") {
        rest.find("\n---\n")
            .map(|end| &rest[end + 5..])
            .unwrap_or(&normalized_newlines)
    } else {
        &normalized_newlines
    };
    let mut source = without_frontmatter.to_string();
    for (pattern, replacement) in [
        (r"!\[\[[^\]|]+\|([^\]]+)\]\]", "$1"),
        (r"!\[\[[^\]]+\]\]", ""),
        (r"\[\[[^\]|]+\|([^\]]+)\]\]", "$1"),
        (r"\[\[([^\]]+)\]\]", "$1"),
    ] {
        if let Ok(regex) = regex::Regex::new(pattern) {
            source = regex.replace_all(&source, replacement).into_owned();
        }
    }

    let parser = Parser::new_ext(&source, Options::all());
    let mut output = String::with_capacity(source.len());
    for event in parser {
        match event {
            Event::Text(text) | Event::Code(text) => {
                if output
                    .chars()
                    .last()
                    .is_some_and(|last| !last.is_whitespace())
                    && text.chars().next().is_some_and(|first| {
                        !matches!(first, '.' | ',' | ';' | ':' | '!' | '?' | ')' | ']' | '}')
                    })
                {
                    output.push(' ');
                }
                output.push_str(&text);
            }
            Event::SoftBreak | Event::HardBreak | Event::Rule => {
                if !output.ends_with('\n') {
                    output.push('\n');
                }
            }
            Event::End(TagEnd::TableCell) => {
                if !output.ends_with(' ') && !output.ends_with('\n') {
                    output.push_str(", ");
                }
            }
            Event::End(
                TagEnd::Paragraph
                | TagEnd::Heading(_)
                | TagEnd::BlockQuote(_)
                | TagEnd::CodeBlock
                | TagEnd::HtmlBlock
                | TagEnd::List(_)
                | TagEnd::Item
                | TagEnd::FootnoteDefinition
                | TagEnd::DefinitionList
                | TagEnd::DefinitionListTitle
                | TagEnd::DefinitionListDefinition
                | TagEnd::Table
                | TagEnd::TableHead
                | TagEnd::TableRow,
            ) => {
                if !output.ends_with('\n') {
                    output.push('\n');
                }
            }
            _ => {}
        }
    }
    output.trim().to_string()
}

async fn wait_until_file_stable(
    path: &Path,
    settle_delay: Duration,
    still_current: impl Fn() -> bool,
) -> Result<()> {
    let mut previous_signature = None;
    // A bounded wait avoids retaining a task forever if another program keeps
    // a file continuously changing.
    for _ in 0..80 {
        if !still_current() {
            return Err(anyhow!("TTS folder watcher was reconfigured"));
        }
        match fs::metadata(path) {
            Ok(metadata) if metadata.is_file() => {
                let signature = (metadata.len(), metadata.modified().ok());
                if previous_signature == Some(signature) {
                    File::open(path).with_context(|| {
                        format!("Watched file is not readable yet: {}", path.display())
                    })?;
                    return Ok(());
                }
                previous_signature = Some(signature);
            }
            Ok(_) => return Err(anyhow!("Watched path is not a regular file")),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                previous_signature = None;
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("Failed to inspect watched file {}", path.display()))
            }
        }
        tokio::time::sleep(settle_delay).await;
    }
    Err(anyhow!(
        "Watched text file did not settle before the timeout: {}",
        path.display()
    ))
}

fn create_interactive_operation_cache(cache_root: &Path, operation_id: u64) -> Result<PathBuf> {
    reset_interactive_cache(cache_root)?;
    // Operation IDs restart at one with every process, while successful
    // history audio intentionally remains cached for up to 24 hours.
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for _ in 0..4 {
        let mut nonce = [0_u8; 16];
        getrandom::getrandom(&mut nonce)
            .map_err(|error| anyhow!("Failed to generate a TTS cache identifier: {error}"))?;
        let mut suffix = String::with_capacity(nonce.len() * 2);
        for byte in nonce {
            suffix.push(HEX[(byte >> 4) as usize] as char);
            suffix.push(HEX[(byte & 0x0f) as usize] as char);
        }
        let operation_cache = cache_root.join(format!("operation-{operation_id}-{suffix}"));
        match fs::create_dir(&operation_cache) {
            Ok(()) => return Ok(operation_cache),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "Failed to create interactive TTS cache {}",
                        operation_cache.display()
                    )
                })
            }
        }
    }
    Err(anyhow!(
        "Failed to allocate a unique interactive TTS cache directory"
    ))
}

fn reset_interactive_cache(cache_root: &Path) -> Result<()> {
    fs::create_dir_all(cache_root)
        .with_context(|| format!("Failed to create TTS cache {}", cache_root.display()))?;
    for entry in fs::read_dir(cache_root)
        .with_context(|| format!("Failed to inspect TTS cache {}", cache_root.display()))?
    {
        let path = entry?.path();
        let is_operation_directory = path.is_dir()
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("operation-"));
        if !is_operation_directory {
            continue;
        }
        let is_stale = fs::metadata(&path)
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|modified| modified.elapsed().ok())
            .is_some_and(|age| age >= INTERACTIVE_CACHE_STALE_AGE);
        if is_stale {
            if let Err(error) = fs::remove_dir_all(&path) {
                log::warn!(
                    "Failed to clean stale TTS cache {}: {}",
                    path.display(),
                    error
                );
            }
        }
    }
    Ok(())
}

fn ensure_disk_capacity(
    output_path: &Path,
    character_count: usize,
    chunk_count: usize,
    committed_resume_bytes: u64,
    settings: &TtsSettings,
) -> Result<()> {
    let output_dir = output_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let available = fs2::available_space(output_dir).with_context(|| {
        format!(
            "Failed to check free disk space for {}",
            output_dir.display()
        )
    })?;

    // Budget raw PCM plus another raw-sized finalized stream. Four characters
    // per second at 1x is deliberately conservative; the speed term makes
    // OpenAI's 0.25x minimum approximately four times more expensive.
    let speed_milli = (settings.speed.clamp(0.25, 4.0) * 1_000.0).round() as u64;
    let working_bytes_per_character = 24_000_000_u64
        .saturating_div(speed_milli.max(250))
        .max(6_000);
    let pause_ms = u64::from(
        settings
            .paragraph_pause_ms
            .min(10_000)
            .max(settings.inter_chunk_pause_ms.min(5_000)),
    )
    .saturating_mul(chunk_count.saturating_sub(1) as u64);
    let estimated_working_bytes = (character_count as u64)
        .saturating_mul(working_bytes_per_character)
        .saturating_add(pause_ms.saturating_mul(96))
        .saturating_add(32 * 1024 * 1024);
    // The checkpointed PCM already consumes disk and does not need to be
    // budgeted a second time. Keep the full final-encode allowance while
    // subtracting only the verified raw prefix from the remaining estimate.
    let estimated_additional_bytes = estimated_working_bytes.saturating_sub(committed_resume_bytes);
    let reserve_mb = settings.disk_reserve_mb.min(1_048_576);
    let reserve_bytes = u64::from(reserve_mb).saturating_mul(1024 * 1024);
    let required = estimated_additional_bytes.saturating_add(reserve_bytes);
    if !disk_capacity_is_sufficient(available, required) {
        return Err(anyhow!(
            "Insufficient disk space for TTS conversion: {:.1} MiB available, {:.1} MiB required (including {} MiB reserve)",
            available as f64 / (1024.0 * 1024.0),
            required as f64 / (1024.0 * 1024.0),
            reserve_mb
        ));
    }
    Ok(())
}

fn ensure_disk_reserve(output_path: &Path, reserve_mb: u32, additional_bytes: u64) -> Result<()> {
    let output_dir = output_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let available = fs2::available_space(output_dir).with_context(|| {
        format!(
            "Failed to re-check free disk space for {}",
            output_dir.display()
        )
    })?;
    let required = u64::from(reserve_mb.min(1_048_576))
        .saturating_mul(1024 * 1024)
        .saturating_add(additional_bytes);
    if !disk_capacity_is_sufficient(available, required) {
        return Err(anyhow!(
            "TTS conversion stopped to protect disk space: {:.1} MiB available, {:.1} MiB required",
            available as f64 / (1024.0 * 1024.0),
            required as f64 / (1024.0 * 1024.0)
        ));
    }
    Ok(())
}

fn queue_watched_path(seen: &mut HashSet<PathBuf>, path: PathBuf) -> bool {
    seen.insert(path)
}

fn disk_capacity_is_sufficient(available: u64, required: u64) -> bool {
    available >= required
}

fn write_i16_le(writer: &mut impl Write, samples: &[i16]) -> Result<()> {
    for sample in samples {
        writer.write_all(&sample.to_le_bytes())?;
    }
    Ok(())
}

fn write_silence(writer: &mut impl Write, duration_ms: u32, sample_rate: u32) -> Result<()> {
    let samples = u64::from(duration_ms)
        .saturating_mul(u64::from(sample_rate))
        .saturating_div(1_000);
    const ZEROES: [u8; 8_192] = [0; 8_192];
    let mut bytes = samples.saturating_mul(2);
    while bytes != 0 {
        let count = bytes.min(ZEROES.len() as u64) as usize;
        writer.write_all(&ZEROES[..count])?;
        bytes -= count as u64;
    }
    Ok(())
}

fn pcm_segment_bytes(samples: &[i16], pause_ms: u32, sample_rate: u32) -> Result<Vec<u8>> {
    let pause_samples = u64::from(pause_ms)
        .saturating_mul(u64::from(sample_rate))
        .saturating_div(1_000);
    let pause_bytes = usize::try_from(pause_samples.saturating_mul(2))
        .map_err(|_| anyhow!("TTS pause is too large to assemble safely"))?;
    let sample_bytes = samples
        .len()
        .checked_mul(2)
        .ok_or_else(|| anyhow!("TTS PCM chunk is too large to assemble safely"))?;
    let total_bytes = sample_bytes
        .checked_add(pause_bytes)
        .ok_or_else(|| anyhow!("TTS PCM segment is too large to assemble safely"))?;
    let mut bytes = Vec::with_capacity(total_bytes);
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    bytes.resize(total_bytes, 0);
    Ok(bytes)
}

fn write_wav_file(path: &Path, samples: &[i16], sample_rate: u32) -> Result<()> {
    fs::write(path, encode_wav(samples, sample_rate))
        .with_context(|| format!("Failed to write WAV cache file {}", path.display()))
}

fn encode_wav(samples: &[i16], sample_rate: u32) -> Vec<u8> {
    let data_size = samples.len().saturating_mul(2).min(u32::MAX as usize) as u32;
    let riff_size = 36_u32.saturating_add(data_size);
    let mut output = Vec::with_capacity(44 + data_size as usize);
    output.extend_from_slice(b"RIFF");
    output.extend_from_slice(&riff_size.to_le_bytes());
    output.extend_from_slice(b"WAVEfmt ");
    output.extend_from_slice(&16_u32.to_le_bytes());
    output.extend_from_slice(&1_u16.to_le_bytes());
    output.extend_from_slice(&1_u16.to_le_bytes());
    output.extend_from_slice(&sample_rate.to_le_bytes());
    output.extend_from_slice(&sample_rate.saturating_mul(2).to_le_bytes());
    output.extend_from_slice(&2_u16.to_le_bytes());
    output.extend_from_slice(&16_u16.to_le_bytes());
    output.extend_from_slice(b"data");
    output.extend_from_slice(&data_size.to_le_bytes());
    for sample in samples.iter().take(data_size as usize / 2) {
        output.extend_from_slice(&sample.to_le_bytes());
    }
    output
}

fn write_wav_from_pcm_file(
    raw_path: &Path,
    output: &mut impl Write,
    sample_rate: u32,
) -> Result<()> {
    let data_size = fs::metadata(raw_path)
        .with_context(|| format!("Failed to inspect partial PCM {}", raw_path.display()))?
        .len();
    if data_size % 2 != 0 || data_size > u64::from(u32::MAX) {
        return Err(anyhow!(
            "Partial PCM is too large or malformed for a standard WAV file"
        ));
    }
    let data_size = data_size as u32;
    output.write_all(b"RIFF")?;
    output.write_all(&36_u32.saturating_add(data_size).to_le_bytes())?;
    output.write_all(b"WAVEfmt ")?;
    output.write_all(&16_u32.to_le_bytes())?;
    output.write_all(&1_u16.to_le_bytes())?;
    output.write_all(&1_u16.to_le_bytes())?;
    output.write_all(&sample_rate.to_le_bytes())?;
    output.write_all(&sample_rate.saturating_mul(2).to_le_bytes())?;
    output.write_all(&2_u16.to_le_bytes())?;
    output.write_all(&16_u16.to_le_bytes())?;
    output.write_all(b"data")?;
    output.write_all(&data_size.to_le_bytes())?;
    let mut reader = BufReader::new(
        File::open(raw_path)
            .with_context(|| format!("Failed to reopen partial PCM {}", raw_path.display()))?,
    );
    std::io::copy(&mut reader, output).context("Failed to stream PCM into WAV")?;
    Ok(())
}

fn create_partial_export_file(destination: &Path) -> Result<(File, PathBuf)> {
    let parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).with_context(|| {
        format!(
            "Failed to create partial TTS export directory {}",
            parent.display()
        )
    })?;
    let name = destination
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new("tts-partial"))
        .to_string_lossy();
    for sequence in 0..100_u32 {
        let path = parent.join(format!(".{name}.{}-{sequence}.partial", std::process::id()));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((file, path)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Err(anyhow!(
        "Unable to allocate a temporary partial TTS export file"
    ))
}

/// The only MP3-encoder-specific adapter in the TTS pipeline.
///
/// `mp3lame-encoder` statically bundles libmp3lame. LAME resamples the 24 kHz
/// provider PCM to 32 kHz so all UI-supported CBR rates, including 256 and
/// 320 kb/s, are valid MPEG-1 combinations. No FFmpeg executable or runtime DLL
/// is needed.
fn encode_mp3_cbr_file(
    raw_path: &Path,
    output: &mut impl Write,
    input_sample_rate: u32,
    bitrate_kbps: u16,
) -> Result<()> {
    let bitrate = match bitrate_kbps {
        64 => Bitrate::Kbps64,
        96 => Bitrate::Kbps96,
        128 => Bitrate::Kbps128,
        192 => Bitrate::Kbps192,
        256 => Bitrate::Kbps256,
        320 => Bitrate::Kbps320,
        other => return Err(anyhow!("Unsupported MP3 bitrate: {other} kb/s")),
    };
    let output_rate = NonZeroU32::new(MP3_OUTPUT_SAMPLE_RATE)
        .expect("MP3_OUTPUT_SAMPLE_RATE is a non-zero constant");
    let builder = LameBuilder::new().ok_or_else(|| anyhow!("Failed to allocate LAME encoder"))?;
    let mut encoder = builder
        .with_num_channels(1)
        .and_then(|builder| builder.with_sample_rate(input_sample_rate))
        .and_then(|builder| builder.with_output_sample_rate(Some(output_rate)))
        .and_then(|builder| builder.with_brate(bitrate))
        .and_then(|builder| builder.with_mode(Mode::Mono))
        .and_then(|builder| builder.with_vbr_mode(VbrMode::Off))
        .and_then(|builder| builder.with_quality(Quality::Best))
        .and_then(LameBuilder::build)
        .map_err(|error| anyhow!("Failed to configure LAME encoder: {error}"))?;

    let raw_size = fs::metadata(raw_path)
        .with_context(|| format!("Failed to inspect partial PCM {}", raw_path.display()))?
        .len();
    if raw_size % 2 != 0 {
        return Err(anyhow!("Partial PCM has an invalid byte length"));
    }
    let mut reader = BufReader::new(
        File::open(raw_path)
            .with_context(|| format!("Failed to reopen partial PCM {}", raw_path.display()))?,
    );
    let mut remaining = raw_size;
    let mut bytes = vec![0_u8; 64 * 1024];
    while remaining != 0 {
        let count = remaining.min(bytes.len() as u64) as usize;
        reader.read_exact(&mut bytes[..count])?;
        let samples: Vec<i16> = bytes[..count]
            .chunks_exact(2)
            .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        let mut encoded =
            Vec::with_capacity(mp3_encode_buffer_capacity(samples.len(), input_sample_rate));
        encoder
            .encode_to_vec(MonoPcm(&samples), &mut encoded)
            .map_err(|error| anyhow!("LAME encoding failed: {error}"))?;
        output.write_all(&encoded)?;
        remaining -= count as u64;
    }
    let mut tail = Vec::with_capacity(7_200);
    encoder
        .flush_to_vec::<FlushGap>(&mut tail)
        .map_err(|error| anyhow!("LAME finalization failed: {error}"))?;
    output.write_all(&tail)?;
    Ok(())
}

fn mp3_encode_buffer_capacity(input_samples: usize, input_sample_rate: u32) -> usize {
    let resampled_samples = input_samples
        .saturating_mul(MP3_OUTPUT_SAMPLE_RATE as usize)
        .div_ceil(input_sample_rate.max(1) as usize);
    mp3lame_encoder::max_required_buffer_size(input_samples.max(resampled_samples))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloud_voice_catalog_parsers_keep_provider_ids_and_groups() {
        let deepgram = deepgram_catalog_entry(&json!({
            "name": "thalia",
            "canonical_name": "aura-2-thalia-en",
            "languages": ["en", "en-US"],
            "metadata": { "accent": "American", "tags": ["feminine", "clear"] }
        }))
        .unwrap();
        assert_eq!(deepgram.id, "aura-2-thalia-en");
        assert!(deepgram.group.contains("American"));
        assert_eq!(deepgram.gender, "feminine");

        let soniox = soniox_catalog_entry(&json!({
            "id": "voice-id",
            "name": "Narrator",
            "models": [{ "model": "tts-rt-v1", "status": "ready" }]
        }))
        .unwrap();
        assert_eq!(soniox.id, "voice-id");
        assert_eq!(soniox.group, "Custom voices");
        assert!(soniox.description.contains("ready"));
    }

    #[test]
    fn edge_provider_has_a_bounded_chunk_limit() {
        assert_eq!(
            TtsManager::provider_character_limit(TtsProvider::Edge, ""),
            EDGE_TTS_PROVIDER_LIMIT
        );
    }

    #[test]
    fn reused_operation_ids_get_distinct_interactive_cache_directories() {
        let directory = std::env::temp_dir().join(format!(
            "aivorelay-tts-interactive-cache-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let first = create_interactive_operation_cache(&directory, 1).unwrap();
        let retained_audio = first.join("history-result.wav");
        fs::write(&retained_audio, b"retained history audio").unwrap();

        let second = create_interactive_operation_cache(&directory, 1).unwrap();

        assert_ne!(first, second);
        assert!(retained_audio.exists());
        assert!(!second.join("history-result.wav").exists());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn batch_skip_classification_requires_a_typed_output_collision() {
        let collision = output_already_exists_error(Path::new("voice.wav"));
        assert!(is_tts_output_collision(&collision));

        let provider_error = anyhow::anyhow!(
            "Provider response included: Output file already exists: not-a-local-path"
        );
        assert!(!is_tts_output_collision(&provider_error));
    }

    #[test]
    fn new_cloud_provider_limits_are_model_aware() {
        assert_eq!(
            TtsManager::provider_character_limit(TtsProvider::Murf, "falcon-2"),
            MURF_CHARACTER_LIMIT
        );
        assert_eq!(
            TtsManager::provider_character_limit(TtsProvider::ElevenLabs, "eleven_v3"),
            ELEVENLABS_V3_CHARACTER_LIMIT
        );
        assert_eq!(
            TtsManager::provider_character_limit(
                TtsProvider::ElevenLabs,
                "eleven_multilingual_v2"
            ),
            ELEVENLABS_MULTILINGUAL_V2_CHARACTER_LIMIT
        );
        assert_eq!(
            TtsManager::provider_character_limit(
                TtsProvider::ElevenLabs,
                "eleven_flash_v2_5"
            ),
            ELEVENLABS_FLASH_V2_5_CHARACTER_LIMIT
        );
        assert_eq!(
            TtsManager::provider_character_limit(TtsProvider::Cartesia, "sonic-3.5"),
            CARTESIA_CONSERVATIVE_CHARACTER_LIMIT
        );
    }

    #[test]
    fn elevenlabs_and_cartesia_reject_locale_ids_as_language_codes() {
        let mut elevenlabs = TtsSettings {
            provider: TtsProvider::ElevenLabs,
            ..TtsSettings::default()
        };
        elevenlabs.elevenlabs_language = "en-US".to_string();
        assert!(TtsManager::validate_settings(&elevenlabs)
            .unwrap_err()
            .to_string()
            .contains("two-letter ISO 639-1"));

        let mut cartesia = TtsSettings {
            provider: TtsProvider::Cartesia,
            ..TtsSettings::default()
        };
        cartesia.cartesia_language = "en-US".to_string();
        assert!(TtsManager::validate_settings(&cartesia)
            .unwrap_err()
            .to_string()
            .contains("two-letter ISO 639-1"));
    }

    #[test]
    fn mp3_buffer_accounts_for_upsampling() {
        let input_samples = 32_768;
        let unscaled = mp3lame_encoder::max_required_buffer_size(input_samples);

        assert!(mp3_encode_buffer_capacity(input_samples, 24_000) > unscaled);
        assert_eq!(mp3_encode_buffer_capacity(input_samples, 48_000), unscaled);
    }

    #[test]
    fn semantic_chunking_is_unicode_safe_and_lossless() {
        let source = "Привет 🌍. 你好世界。AReallyLongUnbrokenToken";
        let chunks = semantic_chunks(source, 10, 12);

        assert_eq!(
            chunks
                .iter()
                .map(|chunk| chunk.text.as_str())
                .collect::<String>(),
            source
        );
        assert!(chunks
            .iter()
            .all(|chunk| chunk.character_count <= 12 && !chunk.text.is_empty()));
    }

    #[test]
    fn semantic_chunking_rejects_empty_and_whitespace_only_input() {
        assert!(semantic_chunks("", 100, 200).is_empty());
        assert!(semantic_chunks(" \r\n\t ", 100, 200).is_empty());
    }

    #[test]
    fn semantic_chunking_drops_leading_blank_paragraph_chunks() {
        let chunks = semantic_chunks("\n\nSpoken text", 2, 100);

        assert_eq!(
            chunks
                .iter()
                .map(|chunk| chunk.text.as_str())
                .collect::<String>(),
            "Spoken text"
        );
        assert!(chunks.iter().all(|chunk| !chunk.text.trim().is_empty()));
    }

    #[test]
    fn semantic_chunking_never_turns_a_leading_newline_into_its_own_request() {
        let source = "AivoRelay Windows TTS smoke test\n\
                      Hello from the Windows text-to-speech provider.\n\
                      This file checks Markdown preprocessing, Unicode text.";
        let chunks = semantic_chunks(source, 80, WINDOWS_TTS_PROVIDER_LIMIT);

        assert_eq!(
            chunks
                .iter()
                .map(|chunk| chunk.text.as_str())
                .collect::<String>(),
            source
        );
        assert!(chunks.iter().all(|chunk| !chunk.text.trim().is_empty()));
    }

    #[test]
    fn markdown_normalization_keeps_readable_content_only() {
        let source = "---\ntitle: Hidden metadata\n---\n# Heading\n\
                      A [readable link](https://example.com) and [[Target|Obsidian label]].\n\
                      | Left | Right |\n| --- | --- |\n| One | Two |";
        let normalized = normalize_markdown_for_speech(source);

        assert!(normalized.contains("Heading"));
        assert!(normalized.contains("readable link"));
        assert!(normalized.contains("Obsidian label"));
        assert!(normalized.contains("Left"));
        assert!(!normalized.contains("Hidden metadata"));
        assert!(!normalized.contains("https://"));
        assert!(!normalized.contains('|'));
    }

    #[test]
    fn markdown_normalization_separates_blocks_lists_tables_and_code() {
        let source = "# Heading\nFirst paragraph.\n\nSecond paragraph.\n\n\
                      - Alpha\n- Beta\n\n\
                      | Left | Right |\n| --- | --- |\n| One | Two |\n\n\
                      `inline code`\n\n```text\nblock code\n```\n\n\
                      ![[hidden-image.png]] [[Target]]";
        let normalized = normalize_markdown_for_speech(source);
        let lines: Vec<&str> = normalized.lines().collect();

        assert!(lines.contains(&"Heading"));
        assert!(lines.contains(&"First paragraph."));
        assert!(lines.contains(&"Second paragraph."));
        assert!(lines.iter().any(|line| line.contains("Alpha")));
        assert!(lines.iter().any(|line| line.contains("Beta")));
        assert!(normalized.contains("Left, Right"));
        assert!(normalized.contains("One, Two"));
        assert!(normalized.contains("inline code"));
        assert!(normalized.contains("block code"));
        assert!(normalized.contains("Target"));
        assert!(!normalized.contains("hidden-image.png"));
    }

    #[test]
    fn utf16_decoding_accepts_both_endiannesses_and_rejects_malformed_input() {
        let text = "Привет 🌍";
        let units: Vec<u16> = text.encode_utf16().collect();
        let little_endian: Vec<u8> = units.iter().flat_map(|unit| unit.to_le_bytes()).collect();
        let big_endian: Vec<u8> = units.iter().flat_map(|unit| unit.to_be_bytes()).collect();

        assert_eq!(decode_utf16(&little_endian, true).unwrap(), text);
        assert_eq!(decode_utf16(&big_endian, false).unwrap(), text);
        assert!(decode_utf16(&[0x41], true).is_err());
        assert!(decode_utf16(&[0x00, 0xD8], true).is_err());
    }

    #[test]
    fn supported_text_file_decodes_utf8_and_bom_marked_encodings() {
        let text = "Hello, Привет, 你好 🌍";
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "aivorelay-tts-decode-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).unwrap();
        let cases = [
            ("utf8.txt", text.as_bytes().to_vec(), "utf-8"),
            (
                "utf8-bom.txt",
                [vec![0xEF, 0xBB, 0xBF], text.as_bytes().to_vec()].concat(),
                "utf-8-bom",
            ),
            (
                "utf16-le.txt",
                [
                    vec![0xFF, 0xFE],
                    text.encode_utf16()
                        .flat_map(u16::to_le_bytes)
                        .collect::<Vec<_>>(),
                ]
                .concat(),
                "utf-16-le",
            ),
            (
                "utf16-be.txt",
                [
                    vec![0xFE, 0xFF],
                    text.encode_utf16()
                        .flat_map(u16::to_be_bytes)
                        .collect::<Vec<_>>(),
                ]
                .concat(),
                "utf-16-be",
            ),
        ];

        for (name, bytes, expected_encoding) in cases {
            let path = directory.join(name);
            fs::write(&path, bytes).unwrap();
            let (decoded, encoding) = read_supported_text_file(&path).unwrap();
            assert_eq!(decoded, text);
            assert_eq!(encoding, expected_encoding);
        }

        let malformed_path = directory.join("malformed-utf16.txt");
        fs::write(&malformed_path, [0xFF, 0xFE, 0x41]).unwrap();
        assert!(read_supported_text_file(&malformed_path).is_err());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn supported_text_reader_rejects_input_above_memory_safety_limit() {
        let oversized = vec![b'a'; MAX_TTS_TEXT_INPUT_BYTES + 1];
        let error = read_tts_text_bytes_bounded(
            std::io::Cursor::new(oversized),
            Path::new("oversized.txt"),
        )
        .expect_err("oversized TTS text must be rejected before chunking");

        assert!(error.to_string().contains("8 MiB safety limit"));
    }

    #[test]
    fn watched_path_snapshot_respects_recursive_setting() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "aivorelay-tts-watch-snapshot-{}-{nonce}",
            std::process::id()
        ));
        let nested = directory.join("nested");
        fs::create_dir_all(&nested).unwrap();
        let root_text = directory.join("root.txt");
        let root_markdown = directory.join("root.md");
        let ignored = directory.join("ignored.json");
        let nested_text = nested.join("nested.txt");
        fs::write(&root_text, "root").unwrap();
        fs::write(&root_markdown, "# Root").unwrap();
        fs::write(&ignored, "{}").unwrap();
        fs::write(&nested_text, "nested").unwrap();

        let flat = collect_supported_text_paths(&directory, false).unwrap();
        assert!(flat.contains(&root_text));
        assert!(flat.contains(&root_markdown));
        assert!(!flat.contains(&ignored));
        assert!(!flat.contains(&nested_text));

        let recursive = collect_supported_text_paths(&directory, true).unwrap();
        assert!(recursive.contains(&root_text));
        assert!(recursive.contains(&root_markdown));
        assert!(!recursive.contains(&ignored));
        assert!(recursive.contains(&nested_text));

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn tts_batch_scan_excludes_nested_output_and_keeps_relative_paths() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let input = std::env::temp_dir().join(format!(
            "aivorelay-tts-batch-scan-{}-{nonce}",
            std::process::id()
        ));
        let nested = input.join("book");
        let output = input.join("audio");
        fs::create_dir_all(&nested).unwrap();
        fs::create_dir_all(&output).unwrap();
        let root_text = input.join("INTRO.TXT");
        let nested_markdown = nested.join("chapter.md");
        let generated_text = output.join("must-not-scan.txt");
        fs::write(&root_text, "Introduction").unwrap();
        fs::write(&nested_markdown, "# Chapter").unwrap();
        fs::write(&generated_text, "Output subtree").unwrap();

        let input = fs::canonicalize(input).unwrap();
        let output = fs::canonicalize(output).unwrap();
        let mut paths = Vec::new();
        let mut warnings = Vec::new();
        collect_batch_text_paths(&input, &output, true, &mut paths, &mut warnings).unwrap();

        assert!(warnings.is_empty());
        assert_eq!(paths.len(), 2);
        assert!(paths.iter().any(|path| path.ends_with("INTRO.TXT")));
        assert!(paths.iter().any(|path| path.ends_with("chapter.md")));
        assert!(!paths.iter().any(|path| path.ends_with("must-not-scan.txt")));
        let chapter = paths
            .iter()
            .find(|path| path.ends_with("chapter.md"))
            .unwrap();
        assert_eq!(
            fs::canonicalize(chapter)
                .unwrap()
                .strip_prefix(&input)
                .unwrap(),
            Path::new("book").join("chapter.md")
        );

        fs::remove_dir_all(input).unwrap();
    }

    #[test]
    fn tts_batch_output_planner_preserves_folders_and_avoids_collisions() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let output = std::env::temp_dir().join(format!(
            "aivorelay-tts-batch-output-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(output.join("book")).unwrap();
        fs::write(output.join("book").join("chapter.mp3"), b"existing").unwrap();
        let mut reserved = HashSet::new();

        let first = allocate_batch_output_path(
            &output,
            Path::new("book").join("chapter.md").as_path(),
            "mp3",
            &mut reserved,
        )
        .unwrap();
        let second = allocate_batch_output_path(
            &output,
            Path::new("book").join("chapter.txt").as_path(),
            "mp3",
            &mut reserved,
        )
        .unwrap();

        assert_eq!(first, output.join("book").join("chapter-2.mp3"));
        assert_eq!(second, output.join("book").join("chapter-3.mp3"));

        fs::remove_dir_all(output).unwrap();
    }

    #[test]
    fn repeated_watcher_events_queue_one_conversion_until_path_is_removed() {
        let mut seen = HashSet::new();
        let path = PathBuf::from(r"C:\TTS\input\chapter.txt");

        assert!(queue_watched_path(&mut seen, path.clone()));
        assert!(!queue_watched_path(&mut seen, path.clone()));
        seen.remove(&path);
        assert!(queue_watched_path(&mut seen, path));
    }

    #[test]
    fn disk_capacity_threshold_rejects_insufficient_space() {
        let reserve_bytes = 512_u64 * 1024 * 1024;
        let required = reserve_bytes + 1_024;

        assert!(!disk_capacity_is_sufficient(required - 1, required));
        assert!(disk_capacity_is_sufficient(required, required));
    }

    #[test]
    fn watched_input_path_must_remain_inside_the_configured_root() {
        let directory = std::env::temp_dir().join(format!(
            "aivorelay-tts-watch-containment-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let outside = directory.with_file_name(format!(
            "{}-outside",
            directory.file_name().unwrap().to_string_lossy()
        ));
        fs::create_dir_all(&directory).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let input = outside.join("outside.txt");
        fs::write(&input, "must not be read").unwrap();

        let error = read_watched_input_no_follow(&input, &directory, true)
            .expect_err("watched inputs outside the configured root must be rejected");
        assert!(error.to_string().contains("outside the configured folder"));

        fs::remove_dir_all(&directory).unwrap();
        fs::remove_dir_all(&outside).unwrap();
    }

    #[test]
    fn semantic_chunking_preserves_crlf_and_detects_paragraphs() {
        let source = "First paragraph.\r\n\r\nSecond paragraph.";
        let chunks = semantic_chunks(source, 25, 30);

        assert_eq!(
            chunks
                .iter()
                .map(|chunk| chunk.text.as_str())
                .collect::<String>(),
            source
        );
        assert_eq!(chunks[0].boundary_after, TtsBoundary::Paragraph);
    }

    #[test]
    fn semantic_chunking_handles_punctuation_only_input() {
        let source = "…？！,,,;;;——";
        let chunks = semantic_chunks(source, 3, 4);

        assert_eq!(
            chunks
                .iter()
                .map(|chunk| chunk.text.as_str())
                .collect::<String>(),
            source
        );
        assert!(chunks
            .iter()
            .all(|chunk| !chunk.text.is_empty() && chunk.character_count <= 4));
    }

    #[test]
    fn semantic_chunking_honors_each_provider_limit_for_unbroken_tokens() {
        for hard_limit in [
            SONIOX_CHARACTER_LIMIT,
            DEEPGRAM_CHARACTER_LIMIT,
            OPENAI_CHARACTER_LIMIT,
            LOCAL_TTS_PROVIDER_LIMIT,
            WINDOWS_TTS_PROVIDER_LIMIT,
        ] {
            let source = "界".repeat(hard_limit + 37);
            let chunks = semantic_chunks(&source, hard_limit + 500, hard_limit);

            assert_eq!(
                chunks
                    .iter()
                    .map(|chunk| chunk.text.as_str())
                    .collect::<String>(),
                source
            );
            assert!(chunks
                .iter()
                .all(|chunk| chunk.character_count <= hard_limit));
            assert!(chunks.len() >= 2);
        }
    }

    #[test]
    fn attempt_status_does_not_describe_offline_failures_as_network_errors() {
        assert_eq!(attempt_status_label(TtsProvider::Windows, None), "local");
        assert_eq!(attempt_status_label(TtsProvider::LocalQwen, None), "local");
        assert_eq!(attempt_status_label(TtsProvider::OpenAi, None), "network");
        assert_eq!(
            attempt_status_label(TtsProvider::OpenAi, Some(StatusCode::TOO_MANY_REQUESTS)),
            "429"
        );
    }

    #[test]
    fn shared_cloud_pcm_decoder_rejects_empty_and_odd_responses() {
        let empty = decode_cloud_pcm_response(StatusCode::OK, None, &[]).unwrap_err();
        assert_eq!(empty.safe_message, "Provider returned empty PCM audio");

        let odd = decode_cloud_pcm_response(StatusCode::OK, None, &[0x01]).unwrap_err();
        assert_eq!(
            odd.safe_message,
            "Provider returned malformed 16-bit PCM audio"
        );
    }

    #[test]
    fn shared_cloud_pcm_decoder_preserves_non_success_error_details_and_retry_after() {
        let retry_after = Duration::from_secs(7);
        let error = decode_cloud_pcm_response(
            StatusCode::BAD_GATEWAY,
            Some(retry_after),
            br#"{"error":{"message":"upstream returned invalid audio"}}"#,
        )
        .expect_err("non-success provider responses must not be decoded as PCM");

        assert_eq!(error.status, Some(StatusCode::BAD_GATEWAY));
        assert_eq!(error.safe_message, "upstream returned invalid audio");
        assert!(error.transient);
        assert_eq!(error.retry_after, Some(retry_after));
    }

    #[test]
    fn shared_cloud_pcm_decoder_reads_valid_little_endian_samples() {
        let pcm = decode_cloud_pcm_response(
            StatusCode::OK,
            None,
            &[
                0x00, 0x80, // i16::MIN
                0xff, 0xff, // -1
                0x00, 0x00, // 0
                0xff, 0x7f, // i16::MAX
            ],
        )
        .expect("valid little-endian PCM should decode through the shared cloud path");

        assert_eq!(pcm, vec![i16::MIN, -1, 0, i16::MAX]);
    }

    #[test]
    fn interactive_cloud_stream_publishes_only_complete_pcm_segments() {
        let samples = [1_i16, 2, 3, 4, 5, 6, 7, 8];
        let bytes = samples
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect::<Vec<_>>();
        let mut emitted_bytes = 0;
        let mut published = Vec::<Vec<i16>>::new();
        {
            let mut callback = |pcm: &[i16]| {
                published.push(pcm.to_vec());
                Ok(())
            };
            publish_complete_pcm_segments(&bytes[..6], &mut emitted_bytes, 8, &mut callback)
                .expect("an incomplete segment should remain buffered");
        }
        assert!(published.is_empty());
        assert_eq!(emitted_bytes, 0);

        {
            let mut callback = |pcm: &[i16]| {
                published.push(pcm.to_vec());
                Ok(())
            };
            publish_complete_pcm_segments(&bytes[..12], &mut emitted_bytes, 8, &mut callback)
                .expect("one complete segment should be published");
        }
        assert_eq!(published, vec![vec![1, 2, 3, 4]]);
        assert_eq!(emitted_bytes, 8);
        let expected_tail = samples[4..6]
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect::<Vec<_>>();
        assert_eq!(&bytes[emitted_bytes..12], expected_tail.as_slice());

        {
            let mut callback = |pcm: &[i16]| {
                published.push(pcm.to_vec());
                Ok(())
            };
            publish_complete_pcm_segments(&bytes, &mut emitted_bytes, 8, &mut callback)
                .expect("the next complete segment should be published once");
        }
        assert_eq!(published, vec![vec![1, 2, 3, 4], vec![5, 6, 7, 8]]);
        assert_eq!(emitted_bytes, bytes.len());
    }

    #[test]
    fn provider_dispatch_names_cover_cloud_local_and_system_providers() {
        assert_eq!(provider_name(TtsProvider::Soniox), "Soniox");
        assert_eq!(provider_name(TtsProvider::Deepgram), "Deepgram");
        assert_eq!(provider_name(TtsProvider::OpenAi), "OpenAI");
        assert_eq!(provider_name(TtsProvider::LocalQwen), "Local Qwen3-TTS");
        assert_eq!(provider_name(TtsProvider::LocalKokoro), "Local Kokoro");
        assert_eq!(provider_name(TtsProvider::Windows), "Windows voices");
    }

    #[test]
    fn local_and_system_voice_instructions_are_inactive() {
        assert!(!TtsProvider::LocalQwen.supports_instructions("any-model"));
        assert!(!TtsProvider::LocalKokoro.supports_instructions("any-model"));
        assert!(!TtsProvider::Windows.supports_instructions("any-model"));
    }

    #[test]
    fn stale_operation_ids_cannot_cancel_a_newer_operation() {
        let active = AtomicU64::new(27);
        let current = TtsState {
            operation_id: 27,
            phase: TtsPhase::Synthesizing,
            ..TtsState::default()
        };

        assert!(!try_cancel_operation(&active, &current, 26));
        assert_eq!(active.load(Ordering::SeqCst), 27);
        assert!(try_cancel_operation(&active, &current, 27));
        assert_eq!(active.load(Ordering::SeqCst), 28);
        assert!(!operation_is_active(&active, 27));
    }

    #[test]
    fn completed_state_is_not_cancellable_while_ready_state_is() {
        let active = AtomicU64::new(31);
        let completed = TtsState {
            operation_id: 31,
            phase: TtsPhase::Completed,
            ..TtsState::default()
        };

        assert!(!try_cancel_operation(&active, &completed, 31));
        assert!(operation_is_active(&active, 31));
        assert_eq!(completed.phase, TtsPhase::Completed);

        let running = TtsState {
            operation_id: 31,
            phase: TtsPhase::Ready,
            ..TtsState::default()
        };
        assert!(try_cancel_operation(&active, &running, 31));
        assert!(!operation_is_active(&active, 31));
    }

    #[test]
    fn foreground_operation_lock_returns_the_documented_busy_error() {
        let lock = Arc::new(tokio::sync::Mutex::new(()));
        let first = try_reserve_foreground_operation_lock(Arc::clone(&lock))
            .expect("first foreground operation should reserve the lock");
        let error = try_reserve_foreground_operation_lock(Arc::clone(&lock))
            .err()
            .expect("a second foreground operation must be rejected as busy");
        assert_eq!(
            error.to_string(),
            "Another text-to-speech operation is already running"
        );
        drop(first);
        assert!(try_reserve_foreground_operation_lock(lock).is_ok());
    }

    #[test]
    fn wav_encoding_writes_valid_pcm_header_and_payload() {
        let samples = [i16::MIN, -1, 0, 1, i16::MAX];
        let wav = encode_wav(&samples, PROVIDER_PCM_SAMPLE_RATE);

        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[12..16], b"fmt ");
        assert_eq!(u16::from_le_bytes([wav[20], wav[21]]), 1);
        assert_eq!(u16::from_le_bytes([wav[22], wav[23]]), 1);
        assert_eq!(
            u32::from_le_bytes([wav[24], wav[25], wav[26], wav[27]]),
            PROVIDER_PCM_SAMPLE_RATE
        );
        assert_eq!(&wav[36..40], b"data");
        assert_eq!(
            u32::from_le_bytes([wav[40], wav[41], wav[42], wav[43]]),
            (samples.len() * 2) as u32
        );
        let decoded: Vec<i16> = wav[44..]
            .chunks_exact(2)
            .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        assert_eq!(decoded, samples);
    }

    #[test]
    fn exhausted_openai_quota_is_not_retried() {
        assert!(!is_transient_status(
            StatusCode::TOO_MANY_REQUESTS,
            "You exceeded your current quota (code: insufficient_quota)"
        ));
        assert!(is_transient_status(
            StatusCode::TOO_MANY_REQUESTS,
            "Rate limit reached; retry later"
        ));
    }

    #[test]
    fn test_normalize_openai_compatible_base_url_ipv6() {
        assert_eq!(
            normalize_openai_compatible_base_url("[::1]:8000/v1"),
            "http://[::1]:8000/v1"
        );
        assert_eq!(
            normalize_openai_compatible_base_url("127.0.0.1:8000/v1"),
            "http://127.0.0.1:8000/v1"
        );
        assert_eq!(
            normalize_openai_compatible_base_url("api.example.com/v1"),
            "https://api.example.com/v1"
        );
    }
}
