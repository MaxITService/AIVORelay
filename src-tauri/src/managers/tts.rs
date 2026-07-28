//! Provider-independent text-to-speech synthesis and file conversion.
//!
//! Provider responses are requested as raw signed 16-bit little-endian mono
//! PCM at 24 kHz. Interactive synthesis writes each completed chunk as a WAV
//! cache asset. File conversion assembles PCM first and encodes exactly one
//! final WAV or MP3 stream.

use crate::managers::local_tts::{
    LocalTtsKind, LocalTtsRuntime, LocalTtsStatus, LOCAL_TTS_LANGUAGES, LOCAL_TTS_PROVIDER_LIMIT,
    LOCAL_TTS_VOICES,
};
use crate::managers::provider_error::{parse_provider_error, safe_text};
use crate::managers::tts_history::{
    metadata_from_settings, TtsHistoryManager, TtsHistoryScope, TtsHistorySourceKind,
};
use crate::managers::tts_resume::{self, ResumeOrigin, ResumeWorkspace, WatcherResumeTask};
use crate::managers::windows_tts::{self, WINDOWS_TTS_PROVIDER_LIMIT};
use crate::settings::{
    apply_text_replacements, TtsKeySource, TtsOutputFormat, TtsProvider, TtsSettings,
};
use anyhow::{anyhow, Context, Result};
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
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

pub const TTS_EVENT_STATE: &str = "tts://state";
pub const TTS_EVENT_CHUNK_READY: &str = "tts://chunk-ready";
pub const TTS_EVENT_PROGRESS: &str = "tts://progress";

pub const SONIOX_CHARACTER_LIMIT: usize = 5_000;
pub const DEEPGRAM_CHARACTER_LIMIT: usize = 2_000;
pub const OPENAI_CHARACTER_LIMIT: usize = 4_096;
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
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_RETRY_DELAY: Duration = Duration::from_secs(60);
const INTERACTIVE_CACHE_STALE_AGE: Duration = Duration::from_secs(24 * 60 * 60);

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

#[derive(Debug)]
struct ProviderAttemptError {
    status: Option<StatusCode>,
    safe_message: String,
    transient: bool,
    retry_after: Option<Duration>,
}

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
    finalization_lock: parking_lot::Mutex<()>,
    local_tts: LocalTtsRuntime,
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
            finalization_lock: parking_lot::Mutex::new(()),
            local_tts,
        }))
    }

    pub fn provider_character_limit(provider: TtsProvider) -> usize {
        match provider {
            TtsProvider::Soniox => SONIOX_CHARACTER_LIMIT,
            TtsProvider::Deepgram => DEEPGRAM_CHARACTER_LIMIT,
            TtsProvider::OpenAi => OPENAI_CHARACTER_LIMIT,
            TtsProvider::LocalQwen => LOCAL_TTS_PROVIDER_LIMIT,
            TtsProvider::Windows => WINDOWS_TTS_PROVIDER_LIMIT,
        }
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

    pub fn validate_settings(settings: &TtsSettings) -> Result<()> {
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
        if settings.provider == TtsProvider::Windows {
            validate_max_chars("Windows voice ID", &settings.windows_voice_id, 1_024)?;
            validate_max_chars(
                "Windows voice language",
                &settings.windows_voice_language,
                128,
            )?;
        }
        Self::validate_openai_instructions(&settings.openai_instructions)?;
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
        Ok(())
    }

    pub fn local_tts_status(&self, kind: LocalTtsKind) -> LocalTtsStatus {
        match kind {
            LocalTtsKind::Qwen => self.local_tts.status(),
        }
    }

    pub async fn install_local_tts(
        &self,
        kind: LocalTtsKind,
        disk_reserve_mb: u32,
    ) -> Result<LocalTtsStatus> {
        match kind {
            LocalTtsKind::Qwen => self.local_tts.install(disk_reserve_mb).await,
        }
    }

    pub fn cancel_local_tts_install(&self, kind: LocalTtsKind) -> bool {
        match kind {
            LocalTtsKind::Qwen => self.local_tts.cancel_install(),
        }
    }

    pub async fn delete_local_tts(&self, kind: LocalTtsKind) -> Result<()> {
        match kind {
            LocalTtsKind::Qwen => self.local_tts.delete().await,
        }
    }

    pub fn current_state(&self) -> TtsState {
        self.state.read().clone()
    }

    pub async fn resolve_operation_settings(&self, settings: &TtsSettings) -> Result<TtsSettings> {
        let mut resolved = settings.clone();
        if resolved.provider == TtsProvider::Windows {
            let max_attempts = resolved.retry_count.min(10).saturating_add(1);
            let mut attempt = 1;
            let voice = loop {
                match windows_tts::resolve_voice_selection(&resolved.windows_voice_id).await {
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
                        tokio::time::sleep(delay).await;
                        attempt = attempt.saturating_add(1);
                    }
                    Err(error) => return Err(anyhow!(error.safe_message)),
                }
            };
            resolved.windows_voice_id = voice.id;
            resolved.windows_voice_language = voice.language;
        }
        Ok(resolved)
    }

    pub fn try_reserve_foreground_operation(&self) -> Result<tokio::sync::OwnedMutexGuard<()>> {
        Arc::clone(&self.foreground_operation_lock)
            .try_lock_owned()
            .map_err(|_| anyhow!("Another text-to-speech operation is already running"))
    }

    /// Cancels exactly one currently running operation.
    pub fn cancel_operation(&self, operation_id: u64) -> bool {
        let _finalization_guard = self.finalization_lock.lock();
        let current = self.current_state();
        if current.operation_id != operation_id
            || !matches!(
                current.phase,
                TtsPhase::Preparing | TtsPhase::Synthesizing | TtsPhase::Retrying | TtsPhase::Ready
            )
        {
            return false;
        }
        if self
            .active_operation_id
            .compare_exchange(
                operation_id,
                operation_id.saturating_add(1),
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
            .is_err()
        {
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

    pub fn chunk_interactive(text: &str, settings: &TtsSettings) -> Vec<TtsChunk> {
        let hard_limit = Self::provider_character_limit(settings.provider);
        semantic_chunks(
            text,
            (settings.interactive_target_chars as usize).clamp(50, hard_limit),
            hard_limit,
        )
    }

    pub fn chunk_file(text: &str, settings: &TtsSettings) -> Vec<TtsChunk> {
        let hard_limit = Self::provider_character_limit(settings.provider);
        semantic_chunks(
            text,
            (settings.file_target_chars as usize).clamp(50, hard_limit),
            hard_limit,
        )
    }

    pub fn inspect_text_file(
        &self,
        path: impl AsRef<Path>,
        settings: &TtsSettings,
    ) -> Result<TextFileInspection> {
        let path = path.as_ref();
        validate_input_extension(path)?;
        let (source, encoding) = read_supported_text_file(path)?;
        let source_for_speech = normalize_source_text(path, &source);
        let processed = Self::preprocess_text(&source_for_speech, settings);
        let chunks = Self::chunk_file(&processed, settings);
        Ok(TextFileInspection {
            path: path.to_path_buf(),
            source_character_count: source.chars().count(),
            processed_character_count: processed.chars().count(),
            chunk_count: chunks.len(),
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

    /// Creates, replaces, or removes the folder watcher from the current saved
    /// settings. No OS watcher exists while the feature is disabled.
    pub fn sync_folder_watcher(self: &Arc<Self>) -> Result<()> {
        use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};

        let settings = crate::settings::get_settings(&self.app_handle).tts;
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
                    if !seen.lock().insert(path.clone()) {
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
            if is_supported_text_path(&path) && self.watched_paths.lock().insert(path.clone()) {
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
            let initial_settings = crate::settings::get_settings(&self.app_handle).tts;
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
            let settings = crate::settings::get_settings(&self.app_handle).tts;
            if !settings.watch_folder_enabled
                || self.watcher_generation.load(Ordering::SeqCst) != generation
            {
                return Ok(());
            }
            let settings = self.resolve_operation_settings(&settings).await?;
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
                    .convert_decoded_text_file(
                        &canonical_input,
                        &source,
                        &output_path,
                        &settings,
                        None,
                        ResumeOrigin::Watcher {
                            source_path: canonical_input.clone(),
                            output_path: output_path.clone(),
                        },
                        require_resume_checkpoint,
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

        if let Err(error) = result {
            self.watched_paths.lock().remove(&input_path);
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

    pub async fn synthesize_interactive_reserved(
        self: &Arc<Self>,
        text: &str,
        settings: &TtsSettings,
        _operation_guard: tokio::sync::OwnedMutexGuard<()>,
    ) -> Result<InteractiveSynthesis> {
        ensure_enabled(settings)?;
        let resolved_settings = self.resolve_operation_settings(settings).await?;
        let settings = &resolved_settings;
        let processed = Self::preprocess_text(text, settings);
        let chunks = Self::chunk_interactive(&processed, settings);
        if chunks.is_empty() {
            return Err(anyhow!("There is no speakable text"));
        }
        if settings.interactive_history_enabled {
            validate_output_settings(settings)?;
        }

        let operation_id = self.begin_operation(
            TtsOperationKind::Interactive,
            settings.provider,
            chunks.len(),
        );
        let operation_cache = self.cache_root.join(format!("operation-{operation_id}"));
        if let Err(error) = reset_interactive_cache(&self.cache_root)
            .and_then(|_| fs::create_dir_all(&operation_cache).map_err(anyhow::Error::from))
        {
            self.fail_operation(operation_id, error.to_string());
            return Err(error).context("Failed to create the TTS cache directory");
        }
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
                let pcm = self
                    .synthesize_chunk_with_retry(
                        operation_id,
                        chunk,
                        chunks.len(),
                        settings,
                        &api_key,
                    )
                    .await?;
                self.ensure_active(operation_id)?;

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
                };
                ready.push(event.clone());
                let _ = self.app_handle.emit(TTS_EVENT_CHUNK_READY, &event);

                if let Some(raw_file) = history_raw_file.as_mut() {
                    write_i16_le(raw_file, &pcm)?;
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
                        &history_raw_path,
                        settings.disk_reserve_mb,
                        (pcm.len() as u64)
                            .saturating_mul(2)
                            .saturating_add(pause_bytes),
                    )?;
                    if pause_ms > 0 {
                        write_silence(raw_file, pause_ms, PROVIDER_PCM_SAMPLE_RATE)?;
                    }
                    raw_file
                        .flush()
                        .context("Failed to flush interactive history PCM")?;
                }
                self.mark_chunk_completed(operation_id, chunk.index, chunks.len());
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
        result
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
        ensure_enabled(settings)?;
        validate_input_extension(input_path.as_ref())?;
        let (source, _encoding) = read_supported_text_file(input_path.as_ref())?;
        self.convert_decoded_text_file(
            input_path.as_ref(),
            &source,
            output_path.as_ref(),
            settings,
            None,
            ResumeOrigin::Manual,
            false,
        )
        .await
    }

    /// Uses an app-owned stable resume namespace instead of an output-sibling
    /// workspace. This is for managed conversions whose temporary destination
    /// changes between processes, such as History regeneration without an
    /// explicit output path.
    pub async fn convert_text_file_with_resume_namespace(
        self: &Arc<Self>,
        input_path: impl AsRef<Path>,
        output_path: impl AsRef<Path>,
        settings: &TtsSettings,
        resume_namespace: &str,
    ) -> Result<FileConversionResult> {
        ensure_enabled(settings)?;
        validate_input_extension(input_path.as_ref())?;
        let (source, _encoding) = read_supported_text_file(input_path.as_ref())?;
        self.convert_decoded_text_file(
            input_path.as_ref(),
            &source,
            output_path.as_ref(),
            settings,
            Some(resume_namespace),
            ResumeOrigin::Manual,
            false,
        )
        .await
    }

    pub fn discard_managed_resume_namespace(&self, resume_namespace: &str) -> Result<()> {
        tts_resume::discard_managed(&self.cache_root, resume_namespace)
    }

    async fn convert_decoded_text_file(
        self: &Arc<Self>,
        input_path: &Path,
        source: &str,
        output_path: &Path,
        settings: &TtsSettings,
        resume_namespace: Option<&str>,
        resume_origin: ResumeOrigin,
        require_resume_checkpoint: bool,
    ) -> Result<FileConversionResult> {
        ensure_enabled(settings)?;
        let _operation_guard = self
            .foreground_operation_lock
            .try_lock()
            .map_err(|_| anyhow!("Another text-to-speech operation is already running"))?;
        let resolved_settings = self.resolve_operation_settings(settings).await?;
        let settings = &resolved_settings;
        validate_input_extension(input_path)?;
        validate_output_extension(output_path, settings.output_format)?;
        validate_output_settings(settings)?;
        let source_for_speech = normalize_source_text(input_path, source);
        let processed = Self::preprocess_text(&source_for_speech, settings);
        let chunks = Self::chunk_file(&processed, settings);
        if chunks.is_empty() {
            return Err(anyhow!("There is no speakable text to convert"));
        }
        if output_path.exists() {
            return Err(anyhow!(
                "Output file already exists: {}",
                output_path.display()
            ));
        }
        let synthesis_signature = tts_resume::synthesis_signature(&chunks, settings)?;
        let mut resume_workspace = if let Some(namespace) = resume_namespace {
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
            return Err(anyhow!(
                "Output file appeared while waiting for the TTS resume workspace: {}",
                output_path.display()
            ));
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
            processed.chars().count(),
            chunks.len(),
            resume_workspace.committed_bytes(),
            settings,
        )?;

        let operation_id = self.begin_operation(
            TtsOperationKind::FileConversion,
            settings.provider,
            chunks.len(),
        );
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
                crate::no_clobber::publish_new_file(&encoded_partial, output_path).with_context(
                    || {
                        format!(
                            "Failed to publish completed audio file {}",
                            output_path.display()
                        )
                    },
                )?;
                self.mark_completed_locked(operation_id)?;
            }

            Ok(FileConversionResult {
                operation_id,
                output_path: output_path.to_path_buf(),
                source_character_count: source.chars().count(),
                processed_character_count: processed.chars().count(),
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
        if cancelled || (result.is_ok() && resume_namespace.is_none()) {
            resume_workspace.discard();
        }
        self.finish_result(operation_id, &result);
        result
    }

    fn begin_operation(
        &self,
        kind: TtsOperationKind,
        provider: TtsProvider,
        total_chunks: usize,
    ) -> u64 {
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
        self.emit_state(&state);
        operation_id
    }

    fn ensure_active(&self, operation_id: u64) -> Result<()> {
        if self.active_operation_id.load(Ordering::SeqCst) != operation_id {
            Err(anyhow!("Text-to-speech operation cancelled"))
        } else {
            Ok(())
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

            match self
                .synthesize_once(operation_id, &chunk.text, settings, api_key)
                .await
            {
                Ok(pcm) if pcm.is_empty() => {
                    return Err(anyhow!(
                        "{} returned empty audio for chunk {}",
                        provider_name(settings.provider),
                        chunk.index
                    ));
                }
                Ok(pcm) => return Ok(pcm),
                Err(error) => {
                    self.ensure_active(operation_id)?;
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
    ) -> std::result::Result<Vec<i16>, ProviderAttemptError> {
        if text.is_empty() {
            return Err(ProviderAttemptError {
                status: None,
                safe_message: "Refusing to send an empty TTS request".to_string(),
                transient: false,
                retry_after: None,
            });
        }
        if text.chars().count() > Self::provider_character_limit(settings.provider) {
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

        let request = match settings.provider {
            TtsProvider::Soniox => {
                self.client
                    .post(SONIOX_TTS_URL)
                    .bearer_auth(api_key)
                    .json(&json!({
                        "model": nonempty_or(&settings.soniox_model, "tts-rt-v1"),
                        "language": nonempty_or(&settings.soniox_language, "en"),
                        "voice": nonempty_or(&settings.soniox_voice, "Adrian"),
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
                    "voice": nonempty_or(&settings.openai_voice, "alloy"),
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
            TtsProvider::LocalQwen => unreachable!("local provider returned before HTTP dispatch"),
            TtsProvider::Windows => unreachable!("Windows provider returned before HTTP dispatch"),
        };

        let response = tokio::select! {
            response = request.send() => response.map_err(network_error)?,
            _ = self.wait_for_cancellation(operation_id) => {
                return Err(cancelled_attempt_error());
            }
        };
        let status = response.status();
        let retry_after = parse_retry_after(response.headers().get(RETRY_AFTER));
        let bytes = tokio::select! {
            bytes = response.bytes() => bytes.map_err(network_error)?,
            _ = self.wait_for_cancellation(operation_id) => {
                return Err(cancelled_attempt_error());
            }
        };
        if !status.is_success() {
            let message = parse_provider_error(&bytes, status);
            return Err(ProviderAttemptError {
                status: Some(status),
                transient: is_transient_status(status, &message),
                safe_message: message,
                retry_after,
            });
        }
        if bytes.len() % 2 != 0 {
            return Err(ProviderAttemptError {
                status: Some(status),
                safe_message: "Provider returned malformed 16-bit PCM audio".to_string(),
                transient: false,
                retry_after: None,
            });
        }
        Ok(bytes
            .chunks_exact(2)
            .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
            .collect())
    }

    async fn wait_for_cancellation(&self, operation_id: u64) {
        while self.active_operation_id.load(Ordering::SeqCst) == operation_id {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
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
        if !chunk_text.is_empty() {
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
        TtsProvider::LocalQwen => unreachable!("handled above"),
        TtsProvider::Windows => unreachable!("handled above"),
    };
    let key = match (settings.provider, source) {
        (provider, TtsKeySource::Separate) => {
            crate::secure_keys::get_tts_api_key(provider.as_str())
        }
        (TtsProvider::Soniox, TtsKeySource::Shared) => crate::secure_keys::get_soniox_api_key(),
        (TtsProvider::Deepgram, TtsKeySource::Shared) => crate::secure_keys::get_deepgram_api_key(),
        (TtsProvider::OpenAi, TtsKeySource::Shared) => {
            crate::secure_keys::get_post_process_api_key("openai")
        }
        (TtsProvider::LocalQwen, _) => unreachable!("handled above"),
        (TtsProvider::Windows, _) => unreachable!("handled above"),
    };
    let key = key.trim();
    if key.is_empty() {
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
        TtsProvider::LocalQwen => "Local Qwen3-TTS",
        TtsProvider::Windows => "Windows voices",
    }
}

fn ensure_enabled(settings: &TtsSettings) -> Result<()> {
    if !settings.enabled {
        return Err(anyhow!("Text-to-speech is disabled"));
    }
    TtsManager::validate_settings(settings)?;
    Ok(())
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

fn nonempty_or<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.trim().is_empty() {
        fallback
    } else {
        value.trim()
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
    let bytes =
        fs::read(path).with_context(|| format!("Failed to read text file {}", path.display()))?;
    decode_supported_text_bytes(bytes)
}

fn decode_supported_text_bytes(bytes: Vec<u8>) -> Result<(String, String)> {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return Ok((
            String::from_utf8(bytes[3..].to_vec()).context("Invalid UTF-8 text after BOM")?,
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
    let mut file = options
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
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
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
    if available < required {
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
    if available < required {
        return Err(anyhow!(
            "TTS conversion stopped to protect disk space: {:.1} MiB available, {:.1} MiB required",
            available as f64 / (1024.0 * 1024.0),
            required as f64 / (1024.0 * 1024.0)
        ));
    }
    Ok(())
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
}
