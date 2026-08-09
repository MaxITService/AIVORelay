use crate::managers::edge_tts::{
    voice_language as edge_voice_language, DEFAULT_EDGE_TTS_VOICE, EDGE_TTS_MODEL,
};
use crate::managers::local_kokoro::{KOKORO_MODEL_REPOSITORY, KOKORO_MODEL_REVISION};
use crate::managers::local_tts::{
    LocalTtsKind, LocalTtsStatus, LOCAL_TTS_MODEL_REPO, LOCAL_TTS_MODEL_REVISION,
};
use crate::managers::tts::{
    FileConversionResult, TextFileInspection, TtsBatchFilePlan, TtsBatchScanRequest,
    TtsBatchScanResult, TtsChunkReady, TtsManager, TtsOperationKind, TtsPhase, TtsState,
    TtsVoiceCatalog, MAX_TTS_TEXT_INPUT_BYTES, SONIOX_TTS_API_KEY_MAX_CHARS,
    SUPPORTED_MP3_BITRATES, TTS_EVENT_BATCH_PROGRESS, TTS_EVENT_CHUNK_READY, TTS_EVENT_STATE,
};
use crate::managers::tts_history::{
    metadata_from_settings, NewTtsHistoryEntry, TtsHistoryManager, TtsHistoryScope,
    TtsHistorySourceKind,
};
use crate::managers::tts_llm;
use crate::managers::tts_resume::UiFileJobSummary;
use crate::managers::windows_tts::{self, WindowsVoiceCatalog};
use crate::settings::{
    get_settings, write_settings, LlmPostProcessBenchmarkResult, TtsLlmScope, TtsOperationScope,
    TtsOutputFormat, TtsPlaybackEffect, TtsProvider, TtsScopeSynthesisSettings, TtsSettings,
    TtsSynthesisConfig, APPLE_INTELLIGENCE_PROVIDER_ID, DEFAULT_TTS_CARTESIA_VOICE,
    DEFAULT_TTS_ELEVENLABS_MODEL, DEFAULT_TTS_ELEVENLABS_VOICE,
    DEFAULT_TTS_MURF_GEN2_VOICE, DEFAULT_TTS_MURF_VOICE, DEFAULT_TTS_OPENAI_VOICE,
    DEFAULT_TTS_SONIOX_VOICE,
};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tauri::{AppHandle, Emitter, Listener, Manager};

const TTS_OVERLAY_EVENT: &str = "tts-overlay-state";
const TTS_OVERLAY_CONTROL_EVENT: &str = "tts-overlay-control";
const TTS_FIRST_PLAYBACK_WARM_TARGET_MS: u64 = 3_000;
const TTS_FIRST_PLAYBACK_COLD_TARGET_MS: u64 = 4_000;
const TTS_CHUNK_READY_TO_PLAYING_TARGET_MS: u64 = 250;
const TTS_LLM_BENCHMARK_LOG_MAX_ENTRIES: usize = 100;
const TTS_LLM_BENCHMARK_LOG_MAX_BYTES: usize = 2 * 1024 * 1024;
static TTS_HISTORY_REPLAY_GENERATION: AtomicU64 = AtomicU64::new(0);

#[tauri::command]
#[specta::specta]
pub async fn get_windows_tts_voice_catalog() -> WindowsVoiceCatalog {
    windows_tts::voice_catalog().await
}

#[tauri::command]
#[specta::specta]
pub async fn get_tts_voice_catalog(
    app: AppHandle,
    provider: TtsProvider,
    scope: Option<TtsOperationScope>,
    model: Option<String>,
) -> Result<TtsVoiceCatalog, String> {
    let mut settings = get_settings(&app)
        .tts
        .effective_for_scope(scope.unwrap_or(TtsOperationScope::Interactive));
    settings.provider = provider;
    if provider == TtsProvider::Murf {
        let model = model
            .as_deref()
            .unwrap_or(&settings.murf_model)
            .trim();
        if !matches!(model, "falcon-2" | "gen2") {
            return Err(format!("Unsupported Murf TTS model: {model}"));
        }
        settings.murf_model = model.to_string();
    }
    app.state::<Arc<TtsManager>>()
        .voice_catalog(&settings)
        .await
        .map_err(|error| error.to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TtsOverlayChunk {
    pub index: usize,
    pub path: String,
    pub pause_after_ms: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TtsOverlayIdentity {
    provider: String,
    model: String,
    voice: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TtsOverlayState {
    pub operation_id: String,
    pub status: String,
    pub provider: String,
    pub model: String,
    pub voice: String,
    pub text_preview: String,
    pub chunks: Vec<TtsOverlayChunk>,
    pub current_chunk: usize,
    pub total_chunks: usize,
    pub retry_attempt: u8,
    pub error: Option<String>,
    pub play_pause_hotkey: String,
    pub play_history_when_overlay_closed: bool,
    pub stop_hotkey: String,
    pub autoplay: bool,
    pub auto_hide_enabled: bool,
    pub auto_hide_delay_seconds: u32,
    pub playback_pitch: f32,
    pub playback_effect: TtsPlaybackEffect,
}

impl Default for TtsOverlayState {
    fn default() -> Self {
        Self {
            operation_id: String::new(),
            status: "idle".to_string(),
            provider: String::new(),
            model: String::new(),
            voice: String::new(),
            text_preview: String::new(),
            chunks: Vec::new(),
            current_chunk: 0,
            total_chunks: 0,
            retry_attempt: 0,
            error: None,
            play_pause_hotkey: String::new(),
            play_history_when_overlay_closed: false,
            stop_hotkey: String::new(),
            autoplay: true,
            auto_hide_enabled: true,
            auto_hide_delay_seconds: 4,
            playback_pitch: 1.0,
            playback_effect: TtsPlaybackEffect::None,
        }
    }
}

#[derive(Default)]
struct TtsLatencyTrace {
    activation_started_at: Option<Instant>,
    overlay_was_cold: bool,
    first_chunk_ready_at: Option<Instant>,
    first_playback_recorded: bool,
    had_retry: bool,
    input_characters: usize,
    autoplay: bool,
}

struct TtsPlaybackLatency {
    activation_to_playing_ms: u64,
    activation_to_chunk_ready_ms: Option<u64>,
    chunk_ready_to_playing_ms: Option<u64>,
    overlay_was_cold: bool,
    had_retry: bool,
    input_characters: usize,
    autoplay: bool,
}

impl TtsLatencyTrace {
    fn start(
        &mut self,
        activation_started_at: Instant,
        overlay_was_cold: bool,
        input_characters: usize,
        autoplay: bool,
    ) {
        *self = Self {
            activation_started_at: Some(activation_started_at),
            overlay_was_cold,
            first_chunk_ready_at: None,
            first_playback_recorded: false,
            had_retry: false,
            input_characters,
            autoplay,
        };
    }

    fn record_first_chunk_ready(&mut self, now: Instant) -> Option<u64> {
        if self.first_chunk_ready_at.is_some() {
            return None;
        }
        self.first_chunk_ready_at = Some(now);
        self.activation_started_at
            .map(|started| elapsed_millis(started, now))
    }

    fn record_first_playback(
        &mut self,
        now: Instant,
        current_chunk: Option<usize>,
    ) -> Option<TtsPlaybackLatency> {
        if self.first_playback_recorded || current_chunk != Some(0) {
            return None;
        }
        let started = self.activation_started_at?;
        self.first_playback_recorded = true;
        Some(TtsPlaybackLatency {
            activation_to_playing_ms: elapsed_millis(started, now),
            activation_to_chunk_ready_ms: self
                .first_chunk_ready_at
                .map(|ready| elapsed_millis(started, ready)),
            chunk_ready_to_playing_ms: self
                .first_chunk_ready_at
                .map(|ready| elapsed_millis(ready, now)),
            overlay_was_cold: self.overlay_was_cold,
            had_retry: self.had_retry,
            input_characters: self.input_characters,
            autoplay: self.autoplay,
        })
    }
}

fn elapsed_millis(started: Instant, finished: Instant) -> u64 {
    finished
        .saturating_duration_since(started)
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

#[derive(Default)]
struct TtsOverlayRuntimeInner {
    state: TtsOverlayState,
    playback_status: Option<String>,
    latency: TtsLatencyTrace,
    segmented_playback: bool,
}

#[derive(Default)]
pub struct TtsOverlayRuntime {
    inner: Mutex<TtsOverlayRuntimeInner>,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ConvertTtsTextFileRequest {
    pub input_path: PathBuf,
    pub output_path: PathBuf,
    pub output_format: TtsOutputFormat,
    pub mp3_bitrate: u16,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct ConvertTtsTextFileResponse {
    pub operation_id: String,
    pub output_path: String,
    pub source_character_count: usize,
    pub processed_character_count: usize,
    pub chunk_count: usize,
    pub resumed_chunks: usize,
    pub output_format: TtsOutputFormat,
    pub mp3_bitrate_kbps: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TtsFileJobConversionResponse {
    pub job_id: String,
    pub conversion: ConvertTtsTextFileResponse,
}

impl From<FileConversionResult> for ConvertTtsTextFileResponse {
    fn from(value: FileConversionResult) -> Self {
        Self {
            operation_id: value.operation_id.to_string(),
            output_path: value.output_path.to_string_lossy().into_owned(),
            source_character_count: value.source_character_count,
            processed_character_count: value.processed_character_count,
            chunk_count: value.chunk_count,
            resumed_chunks: value.resumed_chunks,
            output_format: value.output_format,
            mp3_bitrate_kbps: value.mp3_bitrate_kbps,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ConvertTtsBatchRequest {
    pub client_id: String,
    pub scan: TtsBatchScanResult,
    pub mp3_bitrate: u16,
}

#[derive(Debug, Clone, Copy, Serialize, Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TtsBatchFileStatus {
    Queued,
    Processing,
    Completed,
    Skipped,
    Failed,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TtsBatchFileResult {
    pub index: usize,
    pub input_path: PathBuf,
    pub relative_path: PathBuf,
    pub output_path: PathBuf,
    pub status: TtsBatchFileStatus,
    pub error: Option<String>,
    pub warning: Option<String>,
    pub operation_id: Option<String>,
    pub resumed_chunks: usize,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TtsBatchSummary {
    pub client_id: String,
    pub batch_id: String,
    pub total: usize,
    pub finished: usize,
    pub completed: usize,
    pub skipped: usize,
    pub failed: usize,
    pub cancelled: bool,
    pub files: Vec<TtsBatchFileResult>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TtsBatchProgress {
    pub client_id: String,
    pub batch_id: String,
    pub total: usize,
    pub finished: usize,
    pub completed: usize,
    pub skipped: usize,
    pub failed: usize,
    pub cancelled: bool,
    pub done: bool,
    pub message: Option<String>,
    pub file: Option<TtsBatchFileResult>,
}

pub fn install_tts_event_bridge(app: &AppHandle) {
    let state_app = app.clone();
    app.listen(TTS_EVENT_STATE, move |event| {
        match serde_json::from_str::<TtsState>(event.payload()) {
            Ok(state) if state.kind == Some(TtsOperationKind::Interactive) => {
                apply_manager_state(&state_app, state);
            }
            Ok(_) => {}
            Err(error) => log::warn!("Failed to parse TTS state event: {error}"),
        }
    });

    let chunk_app = app.clone();
    app.listen(
        TTS_EVENT_CHUNK_READY,
        move |event| match serde_json::from_str::<TtsChunkReady>(event.payload()) {
            Ok(chunk) => apply_chunk_ready(&chunk_app, chunk),
            Err(error) => log::warn!("Failed to parse TTS chunk event: {error}"),
        },
    );
}

fn emit_overlay_state(app: &AppHandle, state: &TtsOverlayState) {
    let _ = app.emit(TTS_OVERLAY_EVENT, state);
}

pub fn play_pause_or_replay_latest_history(app: &AppHandle) -> Result<(), String> {
    let settings = get_settings(app).tts;
    if !settings.enabled {
        return Err("Text to Speech is disabled".to_string());
    }
    if !settings.interactive_history_enabled {
        return Err(
            "Enable Interactive TTS History before using the Play history fallback".to_string(),
        );
    }
    if !settings.play_history_when_overlay_closed {
        return Err("The Play history fallback is disabled".to_string());
    }

    if app
        .get_webview_window(crate::overlay::TTS_OVERLAY_WINDOW_LABEL)
        .is_some_and(|window| window.is_visible().unwrap_or(false))
    {
        app.emit(TTS_OVERLAY_CONTROL_EVENT, "play_pause")
            .map_err(|error| format!("Failed to control the TTS overlay: {error}"))?;
        return Ok(());
    }

    let history = app
        .try_state::<Arc<TtsHistoryManager>>()
        .ok_or_else(|| "Interactive TTS History is unavailable".to_string())?;
    let entry = history
        .list_entries(TtsHistoryScope::Interactive)
        .map_err(|error| error.to_string())?
        .into_iter()
        .next()
        .ok_or_else(|| {
            "Interactive TTS History is empty. Read clipboard or selected text first.".to_string()
        })?;
    let audio_path = history
        .retained_audio_path(entry.id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "The newest Interactive History result no longer exists".to_string())?;
    if !audio_path.is_file() {
        return Err(format!(
            "The retained audio for Interactive History result {} is missing",
            entry.id
        ));
    }

    let preview: String = entry.source_text.chars().take(240).collect();
    let preview = if entry.source_text.chars().count() > 240 {
        format!("{}…", preview.trim_end())
    } else {
        preview
    };
    let operation_id = u64::MAX
        .saturating_sub(TTS_HISTORY_REPLAY_GENERATION.fetch_add(1, Ordering::Relaxed))
        .to_string();
    let snapshot = {
        let runtime = app.state::<TtsOverlayRuntime>();
        let mut runtime = runtime.inner.lock();
        runtime.playback_status = None;
        runtime.latency = TtsLatencyTrace::default();
        runtime.segmented_playback = false;
        runtime.state = TtsOverlayState {
            operation_id,
            status: "ready".to_string(),
            provider: entry.provider.as_str().to_string(),
            model: entry.model,
            voice: entry.voice,
            text_preview: preview,
            chunks: vec![TtsOverlayChunk {
                index: 0,
                path: audio_path.to_string_lossy().into_owned(),
                pause_after_ms: 0,
            }],
            current_chunk: 0,
            total_chunks: 1,
            retry_attempt: 0,
            error: None,
            play_pause_hotkey: settings.play_pause_hotkey,
            play_history_when_overlay_closed: settings.play_history_when_overlay_closed,
            stop_hotkey: settings.stop_hotkey,
            autoplay: true,
            auto_hide_enabled: settings.overlay_auto_hide_enabled,
            auto_hide_delay_seconds: settings.overlay_auto_hide_delay_seconds,
            playback_pitch: settings.playback_pitch,
            playback_effect: settings.playback_effect,
        };
        runtime.state.clone()
    };
    crate::overlay::show_tts_overlay_window(app);
    emit_overlay_state(app, &snapshot);
    Ok(())
}

fn apply_manager_state(app: &AppHandle, manager_state: TtsState) {
    let runtime = app.state::<TtsOverlayRuntime>();
    let snapshot = {
        let mut runtime = runtime.inner.lock();
        let next_operation_id = manager_state.operation_id.to_string();
        if !runtime.state.operation_id.is_empty() && runtime.state.operation_id != next_operation_id
        {
            runtime.state.chunks.clear();
            runtime.playback_status = None;
            runtime.segmented_playback = false;
        }
        runtime.state.operation_id = next_operation_id;
        runtime.state.provider = manager_state
            .provider
            .map(|provider| provider.as_str().to_string())
            .unwrap_or_default();
        if !runtime.segmented_playback {
            runtime.state.current_chunk = manager_state.completed_chunks;
            runtime.state.total_chunks = manager_state.total_chunks;
        }
        runtime.state.retry_attempt = manager_state.current_attempt;
        if manager_state.phase == TtsPhase::Retrying || manager_state.current_attempt > 1 {
            runtime.latency.had_retry = true;
        }
        runtime.state.error = (manager_state.phase == TtsPhase::Error)
            .then(|| manager_state.message.clone())
            .flatten();

        runtime.state.status = match manager_state.phase {
            TtsPhase::Error => "error".to_string(),
            TtsPhase::Cancelled => "stopped".to_string(),
            TtsPhase::Retrying => "retrying".to_string(),
            TtsPhase::Preprocessing => "preprocessing".to_string(),
            _ if runtime.playback_status.as_deref() == Some("playing") => "playing".to_string(),
            _ if runtime.playback_status.as_deref() == Some("paused") => "paused".to_string(),
            TtsPhase::Idle => "idle".to_string(),
            TtsPhase::Preparing | TtsPhase::Synthesizing => "loading".to_string(),
            TtsPhase::Ready => "ready".to_string(),
            TtsPhase::Completed => "completed".to_string(),
        };
        runtime.state.clone()
    };
    emit_overlay_state(app, &snapshot);
}

fn apply_chunk_ready(app: &AppHandle, chunk: TtsChunkReady) {
    let runtime = app.state::<TtsOverlayRuntime>();
    let mut first_chunk_ready_ms = None;
    let snapshot = {
        let mut runtime = runtime.inner.lock();
        if runtime.state.operation_id != chunk.operation_id.to_string() {
            return;
        }
        if chunk.chunk_index == 1 {
            first_chunk_ready_ms = runtime.latency.record_first_chunk_ready(Instant::now());
        }
        let overlay_index = chunk.chunk_index.saturating_sub(1);
        if chunk.total_chunks == 0 {
            runtime.segmented_playback = true;
        }
        if let Some(existing) = runtime
            .state
            .chunks
            .iter_mut()
            .find(|existing| existing.index == overlay_index)
        {
            existing.path = chunk.wav_path.to_string_lossy().into_owned();
            existing.pause_after_ms = chunk.pause_after_ms;
        } else {
            runtime.state.chunks.push(TtsOverlayChunk {
                index: overlay_index,
                path: chunk.wav_path.to_string_lossy().into_owned(),
                pause_after_ms: chunk.pause_after_ms,
            });
            runtime.state.chunks.sort_by_key(|item| item.index);
        }
        runtime.state.total_chunks = chunk.total_chunks;
        if runtime.playback_status.is_none() {
            runtime.state.status = "ready".to_string();
        }
        runtime.state.clone()
    };
    if let Some(elapsed_ms) = first_chunk_ready_ms {
        log::info!(
            "TTS latency milestone=first_chunk_ready activation_to_ready_ms={} operation_id={}",
            elapsed_ms,
            chunk.operation_id
        );
    }
    emit_overlay_state(app, &snapshot);
}

fn prepare_overlay(
    app: &AppHandle,
    text: &str,
    settings: &TtsSettings,
    activation_started_at: Instant,
    overlay_was_cold: bool,
) {
    let identity = overlay_identity(settings);
    let preview: String = text.chars().take(240).collect();
    let input_characters = text.chars().count();
    let preview = if text.chars().count() > 240 {
        format!("{}…", preview.trim_end())
    } else {
        preview
    };
    let runtime = app.state::<TtsOverlayRuntime>();
    let snapshot = {
        let mut runtime = runtime.inner.lock();
        runtime.playback_status = None;
        runtime.segmented_playback = false;
        runtime.latency.start(
            activation_started_at,
            overlay_was_cold,
            input_characters,
            settings.autoplay,
        );
        runtime.state = TtsOverlayState {
            operation_id: String::new(),
            status: "loading".to_string(),
            provider: identity.provider,
            model: identity.model,
            voice: identity.voice,
            text_preview: preview,
            chunks: Vec::new(),
            current_chunk: 0,
            total_chunks: 0,
            retry_attempt: 0,
            error: None,
            play_pause_hotkey: settings.play_pause_hotkey.clone(),
            play_history_when_overlay_closed: settings.play_history_when_overlay_closed,
            stop_hotkey: settings.stop_hotkey.clone(),
            autoplay: settings.autoplay,
            auto_hide_enabled: settings.overlay_auto_hide_enabled,
            auto_hide_delay_seconds: settings.overlay_auto_hide_delay_seconds,
            playback_pitch: settings.playback_pitch,
            playback_effect: settings.playback_effect,
        };
        runtime.state.clone()
    };
    emit_overlay_state(app, &snapshot);
}

fn update_overlay_identity(app: &AppHandle, settings: &TtsSettings) {
    let identity = overlay_identity(settings);
    let snapshot = {
        let runtime = app.state::<TtsOverlayRuntime>();
        let mut runtime = runtime.inner.lock();
        runtime.state.provider = identity.provider;
        runtime.state.model = identity.model;
        runtime.state.voice = identity.voice;
        runtime.state.clone()
    };
    emit_overlay_state(app, &snapshot);
}

fn overlay_identity(settings: &TtsSettings) -> TtsOverlayIdentity {
    let (model, voice) = match settings.provider {
        TtsProvider::Soniox => (settings.soniox_model.clone(), settings.soniox_voice.clone()),
        TtsProvider::Deepgram => (
            settings.deepgram_model.clone(),
            settings.deepgram_model.clone(),
        ),
        TtsProvider::OpenAi => (settings.openai_model.clone(), settings.openai_voice.clone()),
        TtsProvider::Murf => (settings.murf_model.clone(), settings.murf_voice.clone()),
        TtsProvider::ElevenLabs => (
            settings.elevenlabs_model.clone(),
            settings.elevenlabs_voice.clone(),
        ),
        TtsProvider::Cartesia => (
            settings.cartesia_model.clone(),
            settings.cartesia_voice.clone(),
        ),
        TtsProvider::OpenAiCompatible => (
            settings.openai_compatible_model.clone(),
            settings.openai_compatible_voice.clone(),
        ),
        TtsProvider::Edge => (EDGE_TTS_MODEL.to_string(), settings.edge_voice.clone()),
        TtsProvider::LocalQwen => (
            format!("{LOCAL_TTS_MODEL_REPO}@{LOCAL_TTS_MODEL_REVISION}"),
            settings.local_qwen_voice.clone(),
        ),
        TtsProvider::LocalKokoro => (
            format!("{KOKORO_MODEL_REPOSITORY}@{KOKORO_MODEL_REVISION}"),
            settings.local_kokoro_voice.clone(),
        ),
        TtsProvider::Windows => (
            "windows.media.speechsynthesis".to_string(),
            settings.windows_voice_id.clone(),
        ),
    };
    TtsOverlayIdentity {
        provider: settings.provider.as_str().to_string(),
        model,
        voice,
    }
}

pub(crate) fn report_tts_error(app: &AppHandle, error: impl std::fmt::Display) {
    let message = error.to_string();
    let settings = get_settings(app)
        .tts
        .effective_for_scope(TtsOperationScope::Interactive);
    let identity = overlay_identity(&settings);
    let runtime = app.state::<TtsOverlayRuntime>();
    let snapshot = {
        let mut runtime = runtime.inner.lock();
        runtime.playback_status = None;
        runtime.latency = TtsLatencyTrace::default();
        runtime.segmented_playback = false;
        runtime.state = TtsOverlayState {
            operation_id: String::new(),
            status: "error".to_string(),
            provider: identity.provider,
            model: identity.model,
            voice: identity.voice,
            text_preview: String::new(),
            chunks: Vec::new(),
            current_chunk: 0,
            total_chunks: 0,
            retry_attempt: 0,
            error: Some(message.clone()),
            play_pause_hotkey: settings.play_pause_hotkey,
            play_history_when_overlay_closed: settings.play_history_when_overlay_closed,
            stop_hotkey: settings.stop_hotkey,
            autoplay: settings.autoplay,
            auto_hide_enabled: settings.overlay_auto_hide_enabled,
            auto_hide_delay_seconds: settings.overlay_auto_hide_delay_seconds,
            playback_pitch: settings.playback_pitch,
            playback_effect: settings.playback_effect,
        };
        runtime.state.clone()
    };
    crate::overlay::show_tts_overlay_window(app);
    emit_overlay_state(app, &snapshot);
    let _ = app.emit("tts-error", message);
}

fn history_metadata(
    settings: &TtsSettings,
    scope: TtsHistoryScope,
    source_text: String,
    source_kind: TtsHistorySourceKind,
    group_id: String,
    external_output_path: Option<PathBuf>,
) -> NewTtsHistoryEntry {
    metadata_from_settings(
        settings,
        scope,
        source_text,
        source_kind,
        group_id,
        external_output_path,
    )
}

fn report_history_error(
    app: &AppHandle,
    context: &str,
    error: impl std::fmt::Display,
    show_in_overlay: bool,
) {
    let detail = error.to_string();
    let message = format!("{context}: {detail}");
    log::error!("{message}");
    let _ = app.emit("tts-history-error", &detail);
    if show_in_overlay {
        let runtime = app.state::<TtsOverlayRuntime>();
        let snapshot = {
            let mut runtime = runtime.inner.lock();
            runtime.state.error = Some(message);
            runtime.state.clone()
        };
        emit_overlay_state(app, &snapshot);
    }
}

fn save_passive_history(
    app: &AppHandle,
    settings: &TtsSettings,
    metadata: NewTtsHistoryEntry,
    audio_path: PathBuf,
    show_error_in_overlay: bool,
) -> Option<String> {
    let enabled = match metadata.scope {
        TtsHistoryScope::Interactive => settings.interactive_history_enabled,
        TtsHistoryScope::File => settings.file_history_enabled,
    };
    if !enabled {
        return None;
    }
    let history = app.state::<Arc<TtsHistoryManager>>();
    if let Err(error) = history.save_success(metadata, audio_path) {
        let detail = error.to_string();
        report_history_error(
            app,
            "TTS completed but its History copy could not be saved",
            &detail,
            show_error_in_overlay,
        );
        Some(detail)
    } else {
        None
    }
}

pub(crate) async fn start_tts_text_at(
    app: AppHandle,
    text: String,
    activation_started_at: Instant,
) -> Result<(), String> {
    let settings = get_settings(&app)
        .tts
        .effective_for_scope(TtsOperationScope::Interactive);
    if !settings.enabled {
        let error = "Text to Speech is disabled".to_string();
        report_tts_error(&app, &error);
        return Err(error);
    }
    if text.trim().is_empty() {
        let error = "There is no clipboard text to read".to_string();
        report_tts_error(&app, &error);
        return Err(error);
    }
    if text.len() > MAX_TTS_TEXT_INPUT_BYTES {
        let error = "TTS text input exceeds the 8 MiB safety limit".to_string();
        report_tts_error(&app, &error);
        return Err(error);
    }

    let manager = app.state::<Arc<TtsManager>>().inner().clone();
    let operation_guard = match manager.try_reserve_foreground_operation() {
        Ok(operation_guard) => operation_guard,
        Err(error) => {
            let error = error.to_string();
            report_tts_error(&app, &error);
            return Err(error);
        }
    };
    let overlay_was_cold = app
        .get_webview_window(crate::overlay::TTS_OVERLAY_WINDOW_LABEL)
        .is_none();
    prepare_overlay(
        &app,
        &text,
        &settings,
        activation_started_at,
        overlay_was_cold,
    );
    crate::overlay::show_tts_overlay_window(&app);
    let identity_app = app.clone();
    let resolved = match manager
        .synthesize_interactive_reserved(
            &text,
            &settings,
            operation_guard,
            move |resolved_settings| update_overlay_identity(&identity_app, resolved_settings),
        )
        .await
    {
        Ok(resolved) => resolved,
        Err(error) => {
            if manager.current_state().phase != TtsPhase::Cancelled {
                report_tts_error(&app, &error);
            }
            return Err(error.to_string());
        }
    };
    let synthesis = resolved.value;
    let settings = resolved.settings;
    if settings.interactive_history_enabled {
        if let Some(audio_path) = synthesis.combined_audio_path {
            let _ = save_passive_history(
                &app,
                &settings,
                history_metadata(
                    &settings,
                    TtsHistoryScope::Interactive,
                    text,
                    TtsHistorySourceKind::Text,
                    format!(
                        "interactive-{}-{}",
                        chrono::Utc::now().timestamp_millis(),
                        synthesis.operation_id
                    ),
                    None,
                ),
                audio_path,
                true,
            );
        } else {
            report_history_error(
                &app,
                "TTS completed but its History copy could not be saved",
                "the combined interactive audio was not created",
                true,
            );
        }
    }
    Ok(())
}

fn parse_provider(provider: &str) -> Result<TtsProvider, String> {
    match provider.trim().to_ascii_lowercase().as_str() {
        "soniox" => Ok(TtsProvider::Soniox),
        "deepgram" => Ok(TtsProvider::Deepgram),
        "openai" | "open_ai" => Ok(TtsProvider::OpenAi),
        "openai_compatible" | "openai-compatible" => Ok(TtsProvider::OpenAiCompatible),
        "murf" | "murf_ai" | "murf-ai" => Ok(TtsProvider::Murf),
        "elevenlabs" | "eleven_labs" | "eleven-labs" => Ok(TtsProvider::ElevenLabs),
        "cartesia" => Ok(TtsProvider::Cartesia),
        "edge" | "edge_tts" | "edge-tts" => Ok(TtsProvider::Edge),
        "local_qwen" | "qwen" | "qwen3" => Ok(TtsProvider::LocalQwen),
        "local_kokoro" | "kokoro" | "kokoro82m" => Ok(TtsProvider::LocalKokoro),
        "windows" | "winrt" => Ok(TtsProvider::Windows),
        _ => Err("Unsupported TTS provider".to_string()),
    }
}

fn normalize_settings(mut settings: TtsSettings) -> TtsSettings {
    settings.soniox_model = nonempty_setting(settings.soniox_model, "tts-rt-v1");
    settings.soniox_language = nonempty_setting(settings.soniox_language, "en");
    settings.soniox_voice = nonempty_setting(settings.soniox_voice, DEFAULT_TTS_SONIOX_VOICE);
    settings.deepgram_model = nonempty_setting(settings.deepgram_model, "aura-2-thalia-en");
    settings.openai_model = nonempty_setting(settings.openai_model, "gpt-4o-mini-tts");
    settings.openai_voice = nonempty_setting(settings.openai_voice, DEFAULT_TTS_OPENAI_VOICE);
    settings.murf_key_source = crate::settings::TtsKeySource::Separate;
    settings.elevenlabs_key_source = crate::settings::TtsKeySource::Separate;
    settings.cartesia_key_source = crate::settings::TtsKeySource::Separate;
    settings.murf_model = nonempty_setting(settings.murf_model, "falcon-2");
    settings.murf_voice = normalize_murf_voice(&settings.murf_model, settings.murf_voice);
    settings.murf_language = nonempty_setting(settings.murf_language, "en-US");
    settings.murf_rate = settings.murf_rate.clamp(-50, 50);
    settings.murf_pitch = settings.murf_pitch.clamp(-50, 50);
    settings.murf_variation = settings.murf_variation.min(5);
    settings.murf_style = normalize_optional_setting(settings.murf_style);
    settings.elevenlabs_model = nonempty_setting(
        settings.elevenlabs_model,
        DEFAULT_TTS_ELEVENLABS_MODEL,
    );
    settings.elevenlabs_voice = nonempty_setting(
        settings.elevenlabs_voice,
        DEFAULT_TTS_ELEVENLABS_VOICE,
    );
    settings.elevenlabs_language =
        nonempty_setting(settings.elevenlabs_language, "en").to_ascii_lowercase();
    settings.elevenlabs_stability =
        finite_clamped(settings.elevenlabs_stability, 0.5, 0.0, 1.0);
    settings.elevenlabs_similarity_boost =
        finite_clamped(settings.elevenlabs_similarity_boost, 0.75, 0.0, 1.0);
    settings.elevenlabs_style = finite_clamped(settings.elevenlabs_style, 0.0, 0.0, 1.0);
    settings.cartesia_model = nonempty_setting(settings.cartesia_model, "sonic-3.5");
    settings.cartesia_voice =
        nonempty_setting(settings.cartesia_voice, DEFAULT_TTS_CARTESIA_VOICE);
    settings.cartesia_language =
        nonempty_setting(settings.cartesia_language, "en").to_ascii_lowercase();
    settings.cartesia_emotion = normalize_optional_setting(settings.cartesia_emotion);
    settings.cartesia_volume =
        finite_clamped(settings.cartesia_volume, 1.0, 0.5, 2.0);
    settings.edge_voice = nonempty_setting(settings.edge_voice, DEFAULT_EDGE_TTS_VOICE);
    settings.edge_voice_language = edge_voice_language(&settings.edge_voice);
    settings.local_qwen_voice = nonempty_setting(settings.local_qwen_voice, "Ryan");
    settings.local_qwen_language = nonempty_setting(settings.local_qwen_language, "Auto");
    settings.local_kokoro_voice = nonempty_setting(settings.local_kokoro_voice, "af_maple");
    settings.local_kokoro_language = nonempty_setting(settings.local_kokoro_language, "English");
    settings.windows_voice_id = settings.windows_voice_id.trim().to_string();
    settings.windows_voice_language = settings.windows_voice_language.trim().to_string();
    settings.llm_preprocessing.provider_id =
        nonempty_setting(settings.llm_preprocessing.provider_id, "openrouter");
    settings.llm_preprocessing.model = settings.llm_preprocessing.model.trim().to_string();
    settings.llm_preprocessing.custom_base_url = settings
        .llm_preprocessing
        .custom_base_url
        .trim()
        .trim_end_matches('/')
        .to_string();
    settings.llm_preprocessing.reasoning_budget = settings
        .llm_preprocessing
        .reasoning_budget
        .clamp(1_024, 1_000_000);
    settings.llm_preprocessing.chunk_target_chars = settings
        .llm_preprocessing
        .chunk_target_chars
        .clamp(1_000, 50_000);
    settings.llm_preprocessing.retry_count = settings.llm_preprocessing.retry_count.min(10);
    settings.llm_preprocessing.retry_base_delay_ms = settings
        .llm_preprocessing
        .retry_base_delay_ms
        .clamp(100, 30_000);
    settings.llm_preprocessing.request_timeout_seconds = settings
        .llm_preprocessing
        .request_timeout_seconds
        .clamp(10, 600);
    normalize_tts_llm_prompts(
        &mut settings.llm_preprocessing.interactive_prompts,
        &mut settings.llm_preprocessing.interactive_selected_prompt_id,
        crate::settings::TtsLlmPreprocessingSettings::default().interactive_prompts,
    );
    normalize_tts_llm_prompts(
        &mut settings.llm_preprocessing.file_prompts,
        &mut settings.llm_preprocessing.file_selected_prompt_id,
        crate::settings::TtsLlmPreprocessingSettings::default().file_prompts,
    );
    trim_tts_llm_benchmark_log(&mut settings.llm_preprocessing.interactive_benchmark_log);
    trim_tts_llm_benchmark_log(&mut settings.llm_preprocessing.file_benchmark_log);
    let hard_limit = TtsManager::settings_character_limit(&settings) as u32;
    settings.interactive_target_chars = settings.interactive_target_chars.clamp(50, hard_limit);
    settings.file_target_chars = settings.file_target_chars.clamp(50, hard_limit);
    settings.retry_count = settings.retry_count.min(10);
    settings.retry_base_delay_ms = settings.retry_base_delay_ms.clamp(100, 30_000);
    settings.inter_chunk_pause_ms = settings.inter_chunk_pause_ms.min(5_000);
    settings.paragraph_pause_ms = settings.paragraph_pause_ms.min(10_000);
    settings.overlay_auto_hide_delay_seconds =
        settings.overlay_auto_hide_delay_seconds.clamp(1, 300);
    settings.watch_settle_delay_ms = settings.watch_settle_delay_ms.clamp(100, 60_000);
    settings.disk_reserve_mb = settings.disk_reserve_mb.min(1_048_576);
    settings.interactive_history_max_entries =
        settings.interactive_history_max_entries.clamp(1, 100_000);
    settings.interactive_history_max_storage_mb = settings
        .interactive_history_max_storage_mb
        .clamp(1, 1_048_576);
    settings.file_history_max_entries = settings.file_history_max_entries.clamp(1, 100_000);
    settings.file_history_max_storage_mb = settings.file_history_max_storage_mb.clamp(1, 1_048_576);
    settings.speed = match settings.provider {
        TtsProvider::Soniox => settings.speed.clamp(0.7, 1.3),
        TtsProvider::Deepgram => settings.speed.clamp(0.7, 1.5),
        TtsProvider::OpenAi => settings.speed.clamp(0.25, 4.0),
        TtsProvider::OpenAiCompatible => settings.speed.clamp(0.25, 4.0),
        TtsProvider::Murf => 1.0,
        TtsProvider::ElevenLabs if settings.elevenlabs_model == "eleven_v3" => 1.0,
        TtsProvider::ElevenLabs => settings.speed.clamp(0.7, 1.2),
        TtsProvider::Cartesia => settings.speed.clamp(0.6, 1.5),
        TtsProvider::Edge => settings.speed.clamp(0.5, 2.0),
        TtsProvider::LocalQwen => settings.speed.clamp(0.5, 2.0),
        TtsProvider::LocalKokoro => settings.speed.clamp(0.5, 2.0),
        TtsProvider::Windows => settings.speed.clamp(0.5, 2.0),
    };
    settings.playback_pitch = settings.playback_pitch.clamp(0.5, 2.0);
    if !SUPPORTED_MP3_BITRATES.contains(&settings.mp3_bitrate_kbps) {
        settings.mp3_bitrate_kbps = 256;
    }
    let mut prompt_names = std::collections::HashSet::new();
    let mut prompt_ids = std::collections::HashSet::new();
    settings.prompt_presets.retain(|preset| {
        !preset.id.trim().is_empty()
            && !preset.name.trim().is_empty()
            && prompt_ids.insert(preset.id.clone())
            && prompt_names.insert(preset.name.trim().to_lowercase())
    });
    if let Some(selected) = settings
        .prompt_presets
        .iter()
        .find(|preset| preset.id == settings.selected_prompt_id)
    {
        settings.openai_instructions = selected.instructions.clone();
    } else {
        settings.selected_prompt_id.clear();
    }
    settings.ensure_synthesis_scopes();
    normalize_synthesis_scope(
        &mut settings.interactive_synthesis,
        &settings.prompt_presets,
        &settings.synthesis_presets,
    );
    normalize_synthesis_scope(
        &mut settings.file_synthesis,
        &settings.prompt_presets,
        &settings.synthesis_presets,
    );
    normalize_synthesis_presets(&mut settings);
    settings
}

fn normalize_synthesis_config(
    config: &mut TtsSynthesisConfig,
    prompt_presets: &[crate::settings::TtsPromptPreset],
) {
    match config.provider {
        TtsProvider::Soniox => {
            config.model = nonempty_setting(std::mem::take(&mut config.model), "tts-rt-v1");
            config.voice =
                nonempty_setting(std::mem::take(&mut config.voice), DEFAULT_TTS_SONIOX_VOICE);
            config.language = nonempty_setting(std::mem::take(&mut config.language), "en")
                .to_ascii_lowercase();
        }
        TtsProvider::Deepgram => {
            config.model = nonempty_setting(std::mem::take(&mut config.model), "aura-2-thalia-en");
            config.voice = config.model.clone();
            config.language.clear();
        }
        TtsProvider::OpenAi => {
            config.model = nonempty_setting(std::mem::take(&mut config.model), "gpt-4o-mini-tts");
            config.voice =
                nonempty_setting(std::mem::take(&mut config.voice), DEFAULT_TTS_OPENAI_VOICE);
            config.language.clear();
        }
        TtsProvider::OpenAiCompatible => {
            config.model = nonempty_setting(std::mem::take(&mut config.model), "tts-1");
            config.voice = nonempty_setting(std::mem::take(&mut config.voice), "alloy");
            config.language.clear();
        }
        TtsProvider::Murf => {
            config.model = nonempty_setting(std::mem::take(&mut config.model), "falcon-2");
            config.voice = normalize_murf_voice(&config.model, std::mem::take(&mut config.voice));
            config.language = nonempty_setting(std::mem::take(&mut config.language), "en-US");
            config.key_source = crate::settings::TtsKeySource::Separate;
        }
        TtsProvider::ElevenLabs => {
            config.model = nonempty_setting(
                std::mem::take(&mut config.model),
                DEFAULT_TTS_ELEVENLABS_MODEL,
            );
            config.voice = nonempty_setting(
                std::mem::take(&mut config.voice),
                DEFAULT_TTS_ELEVENLABS_VOICE,
            );
            config.language = nonempty_setting(std::mem::take(&mut config.language), "en")
                .to_ascii_lowercase();
            config.key_source = crate::settings::TtsKeySource::Separate;
        }
        TtsProvider::Cartesia => {
            config.model = nonempty_setting(std::mem::take(&mut config.model), "sonic-3.5");
            config.voice = nonempty_setting(
                std::mem::take(&mut config.voice),
                DEFAULT_TTS_CARTESIA_VOICE,
            );
            config.language = nonempty_setting(std::mem::take(&mut config.language), "en");
            config.key_source = crate::settings::TtsKeySource::Separate;
        }
        TtsProvider::Edge => {
            config.model = EDGE_TTS_MODEL.to_string();
            config.voice = nonempty_setting(
                std::mem::take(&mut config.voice),
                crate::managers::edge_tts::DEFAULT_EDGE_TTS_VOICE,
            );
            config.language = edge_voice_language(&config.voice);
        }
        TtsProvider::LocalQwen => {
            config.model = "qwen3-tts-12hz-0.6b-customvoice".to_string();
            config.voice = nonempty_setting(std::mem::take(&mut config.voice), "Ryan");
            config.language = nonempty_setting(std::mem::take(&mut config.language), "Auto");
        }
        TtsProvider::LocalKokoro => {
            config.model = "kokoro-82m".to_string();
            config.voice = nonempty_setting(std::mem::take(&mut config.voice), "af_maple");
            config.language = nonempty_setting(std::mem::take(&mut config.language), "English");
        }
        TtsProvider::Windows => {
            config.model = "windows.media.speechsynthesis".to_string();
            config.voice = config.voice.trim().to_string();
            config.language = config.language.trim().to_string();
        }
    }
    config.speed = match config.provider {
        TtsProvider::Soniox => config.speed.clamp(0.7, 1.3),
        TtsProvider::Deepgram => config.speed.clamp(0.7, 1.5),
        TtsProvider::OpenAi | TtsProvider::OpenAiCompatible => config.speed.clamp(0.25, 4.0),
        TtsProvider::Murf => 1.0,
        TtsProvider::ElevenLabs if config.model == "eleven_v3" => 1.0,
        TtsProvider::ElevenLabs => config.speed.clamp(0.7, 1.2),
        TtsProvider::Cartesia => config.speed.clamp(0.6, 1.5),
        TtsProvider::Edge
        | TtsProvider::LocalQwen
        | TtsProvider::LocalKokoro
        | TtsProvider::Windows => config.speed.clamp(0.5, 2.0),
    };
    config.murf_rate = config.murf_rate.clamp(-50, 50);
    config.murf_pitch = config.murf_pitch.clamp(-50, 50);
    config.murf_variation = config.murf_variation.min(5);
    config.murf_style = normalize_optional_setting(config.murf_style.take());
    config.elevenlabs_stability =
        finite_clamped(config.elevenlabs_stability, 0.5, 0.0, 1.0);
    config.elevenlabs_similarity_boost =
        finite_clamped(config.elevenlabs_similarity_boost, 0.75, 0.0, 1.0);
    config.elevenlabs_style = finite_clamped(config.elevenlabs_style, 0.0, 0.0, 1.0);
    config.cartesia_emotion = normalize_optional_setting(config.cartesia_emotion.take());
    config.cartesia_volume = finite_clamped(config.cartesia_volume, 1.0, 0.5, 2.0);
    config.target_chars = config.target_chars.clamp(
        50,
        TtsManager::provider_character_limit(config.provider, &config.model) as u32,
    );
    config.retry_count = config.retry_count.min(10);
    config.retry_base_delay_ms = config.retry_base_delay_ms.clamp(100, 30_000);
    config.inter_chunk_pause_ms = config.inter_chunk_pause_ms.min(5_000);
    config.paragraph_pause_ms = config.paragraph_pause_ms.min(10_000);
    if !SUPPORTED_MP3_BITRATES.contains(&config.mp3_bitrate_kbps) {
        config.mp3_bitrate_kbps = 256;
    }
    if let Some(selected) = prompt_presets
        .iter()
        .find(|preset| preset.id == config.voice_prompt_preset_id)
    {
        config.voice_instructions = selected.instructions.clone();
    } else {
        config.voice_prompt_preset_id.clear();
    }
}

fn normalize_synthesis_scope(
    scope: &mut TtsScopeSynthesisSettings,
    prompt_presets: &[crate::settings::TtsPromptPreset],
    synthesis_presets: &[crate::settings::TtsSynthesisPreset],
) {
    let mut keys = std::collections::HashSet::new();
    for entry in &mut scope.models {
        normalize_synthesis_config(&mut entry.config, prompt_presets);
    }
    scope.normalize_provider_memories();
    scope
        .models
        .retain(|entry| keys.insert(entry.model_key.clone()));
    if scope.models.len() > 100 {
        let excess = scope.models.len() - 100;
        scope.models.drain(..excess);
    }
    if !scope
        .models
        .iter()
        .any(|entry| entry.model_key == scope.active_model_key)
    {
        scope.active_model_key = scope
            .models
            .first()
            .map(|entry| entry.model_key.clone())
            .unwrap_or_default();
    }
    if !synthesis_presets
        .iter()
        .any(|preset| preset.id == scope.selected_preset_id)
    {
        scope.selected_preset_id.clear();
    }
    scope.normalize_provider_memories();
}

fn normalize_synthesis_presets(settings: &mut TtsSettings) {
    let mut ids = std::collections::HashSet::new();
    let mut names = std::collections::HashSet::new();
    for preset in &mut settings.synthesis_presets {
        preset.id = preset.id.trim().to_string();
        preset.name = preset.name.trim().to_string();
        normalize_synthesis_config(&mut preset.config, &settings.prompt_presets);
    }
    settings.synthesis_presets.retain(|preset| {
        !preset.id.is_empty()
            && !preset.name.is_empty()
            && ids.insert(preset.id.clone())
            && names.insert(preset.name.to_lowercase())
    });
    settings.synthesis_presets.truncate(100);
    let presets = settings.synthesis_presets.clone();
    normalize_synthesis_scope(
        &mut settings.interactive_synthesis,
        &settings.prompt_presets,
        &presets,
    );
    normalize_synthesis_scope(
        &mut settings.file_synthesis,
        &settings.prompt_presets,
        &presets,
    );
}

fn validate_synthesis_collections(settings: &TtsSettings) -> Result<(), String> {
    const MAX_SAVED_MODELS_PER_SCOPE: usize = 100;
    const MAX_SYNTHESIS_PRESETS: usize = 100;
    const MAX_ID_CHARS: usize = 256;
    const MAX_NAME_CHARS: usize = 256;

    if settings.synthesis_presets.len() > MAX_SYNTHESIS_PRESETS {
        return Err(format!(
            "TTS synthesis presets must not exceed {MAX_SYNTHESIS_PRESETS} entries"
        ));
    }
    let mut preset_ids = std::collections::HashSet::new();
    let mut preset_names = std::collections::HashSet::new();
    for preset in &settings.synthesis_presets {
        let id = preset.id.trim();
        let name = preset.name.trim();
        if id.is_empty() {
            return Err("TTS synthesis preset ID cannot be empty".to_string());
        }
        if name.is_empty() {
            return Err("TTS synthesis preset name cannot be empty".to_string());
        }
        if id.chars().count() > MAX_ID_CHARS {
            return Err(format!(
                "TTS synthesis preset ID must not exceed {MAX_ID_CHARS} characters"
            ));
        }
        if name.chars().count() > MAX_NAME_CHARS {
            return Err(format!(
                "TTS synthesis preset name must not exceed {MAX_NAME_CHARS} characters"
            ));
        }
        if !preset_ids.insert(id.to_string()) {
            return Err("TTS synthesis preset IDs must be unique".to_string());
        }
        if !preset_names.insert(name.to_lowercase()) {
            return Err("TTS synthesis preset names must be unique".to_string());
        }
    }

    for (scope_name, scope) in [
        ("Interactive", &settings.interactive_synthesis),
        ("File Operations", &settings.file_synthesis),
    ] {
        if scope.models.len() > MAX_SAVED_MODELS_PER_SCOPE {
            return Err(format!(
                "{scope_name} TTS settings must not exceed {MAX_SAVED_MODELS_PER_SCOPE} saved models"
            ));
        }
        let mut model_keys = std::collections::HashSet::new();
        for entry in &scope.models {
            if !model_keys.insert(entry.config.model_key()) {
                return Err(format!(
                    "{scope_name} TTS settings contain more than one profile for the same synthesis model identity"
                ));
            }
        }
    }
    Ok(())
}

fn validate_all_synthesis_configs(settings: &TtsSettings) -> Result<(), String> {
    for (scope_name, scope) in [
        ("Interactive", &settings.interactive_synthesis),
        ("File Operations", &settings.file_synthesis),
    ] {
        for entry in &scope.models {
            let mut candidate = settings.clone();
            entry
                .config
                .apply_to(&mut candidate, TtsOperationScope::Interactive);
            TtsManager::validate_synthesis_settings(&candidate).map_err(|error| {
                format!(
                    "{scope_name} TTS settings for '{}': {error}",
                    entry.model_key
                )
            })?;
        }
    }
    for preset in &settings.synthesis_presets {
        let mut candidate = settings.clone();
        preset
            .config
            .apply_to(&mut candidate, TtsOperationScope::Interactive);
        TtsManager::validate_synthesis_settings(&candidate)
            .map_err(|error| format!("TTS synthesis preset '{}': {error}", preset.name.trim()))?;
    }
    Ok(())
}

fn is_tts_model_identity_field(field: &str) -> bool {
    matches!(
        field,
        "soniox_model"
            | "deepgram_model"
            | "openai_model"
            | "murf_model"
            | "elevenlabs_model"
            | "cartesia_model"
            | "openai_compatible_model"
    )
}

fn is_tts_synthesis_field(field: &str) -> bool {
    matches!(
        field,
        "provider"
            | "soniox_key_source"
            | "deepgram_key_source"
            | "openai_key_source"
            | "openai_compatible_key_source"
            | "murf_key_source"
            | "elevenlabs_key_source"
            | "cartesia_key_source"
            | "soniox_model"
            | "soniox_language"
            | "soniox_voice"
            | "deepgram_model"
            | "openai_model"
            | "openai_voice"
            | "openai_compatible_base_url"
            | "openai_compatible_allow_insecure_http"
            | "openai_compatible_model"
            | "openai_compatible_voice"
            | "murf_model"
            | "murf_voice"
            | "murf_language"
            | "murf_rate"
            | "murf_pitch"
            | "murf_variation"
            | "murf_style"
            | "elevenlabs_model"
            | "elevenlabs_voice"
            | "elevenlabs_language"
            | "elevenlabs_stability"
            | "elevenlabs_similarity_boost"
            | "elevenlabs_style"
            | "elevenlabs_use_speaker_boost"
            | "elevenlabs_apply_text_normalization"
            | "cartesia_model"
            | "cartesia_voice"
            | "cartesia_language"
            | "cartesia_emotion"
            | "cartesia_volume"
            | "edge_voice"
            | "edge_voice_language"
            | "local_qwen_voice"
            | "local_qwen_language"
            | "local_kokoro_voice"
            | "local_kokoro_language"
            | "windows_voice_id"
            | "windows_voice_language"
            | "openai_instructions"
            | "selected_prompt_id"
            | "prompt_presets"
            | "speed"
            | "preprocessing_enabled"
            | "preprocessing_rules"
            | "interactive_target_chars"
            | "file_target_chars"
            | "retry_count"
            | "retry_base_delay_ms"
            | "inter_chunk_pause_ms"
            | "paragraph_pause_ms"
            | "output_format"
            | "mp3_bitrate_kbps"
            | "synthesis_preset_load"
    )
}

fn normalize_tts_llm_prompts(
    prompts: &mut Vec<crate::settings::LLMPrompt>,
    selected_prompt_id: &mut String,
    defaults: Vec<crate::settings::LLMPrompt>,
) {
    if prompts.is_empty() {
        *prompts = defaults;
    }
    if !prompts
        .iter()
        .any(|prompt| prompt.id == *selected_prompt_id)
    {
        *selected_prompt_id = prompts
            .first()
            .map(|prompt| prompt.id.clone())
            .unwrap_or_default();
    }
}

fn trim_tts_llm_benchmark_log(log: &mut Vec<LlmPostProcessBenchmarkResult>) {
    let mut retained_bytes = 0usize;
    let mut retained_entries = 0usize;
    for entry in log.iter().take(TTS_LLM_BENCHMARK_LOG_MAX_ENTRIES) {
        let entry_bytes = serde_json::to_vec(entry)
            .map(|serialized| serialized.len())
            .unwrap_or(TTS_LLM_BENCHMARK_LOG_MAX_BYTES.saturating_add(1));
        let next_bytes = retained_bytes.saturating_add(entry_bytes);
        if next_bytes > TTS_LLM_BENCHMARK_LOG_MAX_BYTES {
            break;
        }
        retained_bytes = next_bytes;
        retained_entries += 1;
    }
    log.truncate(retained_entries);
}

fn nonempty_setting(value: String, fallback: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        fallback.to_string()
    } else {
        value.to_string()
    }
}

fn normalize_murf_voice(model: &str, value: String) -> String {
    let value = value.trim();
    let gen2 = model == "gen2";
    let incompatible_builtin = if gen2 {
        value == DEFAULT_TTS_MURF_VOICE
    } else {
        value == DEFAULT_TTS_MURF_GEN2_VOICE
    };
    if value.is_empty() || incompatible_builtin {
        if gen2 {
            DEFAULT_TTS_MURF_GEN2_VOICE.to_string()
        } else {
            DEFAULT_TTS_MURF_VOICE.to_string()
        }
    } else {
        value.to_string()
    }
}

fn normalize_optional_setting(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn finite_clamped(value: f32, fallback: f32, minimum: f32, maximum: f32) -> f32 {
    if value.is_finite() {
        value.clamp(minimum, maximum)
    } else {
        fallback
    }
}

fn watcher_configuration_changed(previous: &TtsSettings, current: &TtsSettings) -> bool {
    previous.watch_folder_enabled != current.watch_folder_enabled
        || ((previous.watch_folder_enabled || current.watch_folder_enabled)
            && (previous.watch_input_directory.trim() != current.watch_input_directory.trim()
                || previous.watch_output_directory.trim() != current.watch_output_directory.trim()
                || previous.watch_recursive != current.watch_recursive))
}

#[tauri::command]
#[specta::specta]
pub fn update_tts_settings(
    app: AppHandle,
    settings: TtsSettings,
    scope: Option<TtsOperationScope>,
    changed_field: Option<String>,
) -> Result<TtsSettings, String> {
    validate_synthesis_collections(&settings)?;
    let mut settings = normalize_settings(settings);
    if let Some(scope) = scope {
        let changed_field = changed_field.as_deref().unwrap_or_default();
        if changed_field == "provider" {
            settings.select_scope_provider(scope, settings.provider)?;
        } else if changed_field == "synthesis_preset_load" {
            let preset_id = settings.scope_synthesis(scope).selected_preset_id.clone();
            settings.load_synthesis_preset(scope, &preset_id)?;
        } else if is_tts_model_identity_field(changed_field) {
            let requested = settings.clone();
            settings.select_scope_model(scope, &requested);
        } else if is_tts_synthesis_field(changed_field) {
            let requested = settings.clone();
            settings.capture_scope_settings(scope, &requested);
            settings
                .scope_synthesis_mut(scope)
                .selected_preset_id
                .clear();
        }
        settings = normalize_settings(settings);
    } else {
        // Compatibility for older callers that still submit only the legacy
        // flat TTS object. Treat its synthesis fields as Interactive settings.
        let requested = settings.clone();
        settings.capture_scope_settings(TtsOperationScope::Interactive, &requested);
        settings = normalize_settings(settings);
    }
    validate_all_synthesis_configs(&settings)?;
    if !settings.interactive_history_enabled {
        settings.play_history_when_overlay_closed = false;
    } else if settings.play_history_when_overlay_closed {
        let has_interactive_history = app
            .try_state::<Arc<TtsHistoryManager>>()
            .ok_or_else(|| "Interactive TTS History is unavailable".to_string())?
            .list_entries(TtsHistoryScope::Interactive)
            .map_err(|error| error.to_string())?
            .into_iter()
            .next()
            .is_some();
        if !has_interactive_history {
            settings.play_history_when_overlay_closed = false;
        }
    }
    for validation_scope in [TtsOperationScope::Interactive, TtsOperationScope::File] {
        TtsManager::validate_settings(&settings.effective_for_scope(validation_scope))
            .map_err(|error| error.to_string())?;
    }
    settings.apply_scope_to_flat(TtsOperationScope::Interactive);
    let response = settings.effective_for_scope(scope.unwrap_or(TtsOperationScope::Interactive));
    let mut app_settings = get_settings(&app);
    let previous = app_settings.tts.clone();
    let watcher_configuration_changed = watcher_configuration_changed(&previous, &settings);
    let interactive_retention_changed = previous.interactive_history_max_entries
        != settings.interactive_history_max_entries
        || previous.interactive_history_max_storage_mb
            != settings.interactive_history_max_storage_mb;
    let file_retention_changed = previous.file_history_max_entries
        != settings.file_history_max_entries
        || previous.file_history_max_storage_mb != settings.file_history_max_storage_mb;
    crate::shortcut::prepare_tts_play_history_fallback_binding(&app, &mut app_settings, &settings)?;
    app_settings.tts = settings.clone();
    write_settings(&app, app_settings);
    let overlay_snapshot = {
        let runtime = app.state::<TtsOverlayRuntime>();
        let mut runtime = runtime.inner.lock();
        runtime.state.play_pause_hotkey = settings.play_pause_hotkey.clone();
        runtime.state.play_history_when_overlay_closed = settings.play_history_when_overlay_closed;
        runtime.state.stop_hotkey = settings.stop_hotkey.clone();
        runtime.state.playback_pitch = settings.playback_pitch;
        runtime.state.playback_effect = settings.playback_effect;
        runtime.state.clone()
    };
    emit_overlay_state(&app, &overlay_snapshot);

    if watcher_configuration_changed {
        let manager = app.state::<Arc<TtsManager>>().inner().clone();
        if let Err(error) = manager.sync_folder_watcher() {
            let mut rollback = get_settings(&app);
            let _ = crate::shortcut::prepare_tts_play_history_fallback_binding(
                &app,
                &mut rollback,
                &previous,
            );
            rollback.tts = previous;
            write_settings(&app, rollback);
            let _ = manager.sync_folder_watcher();
            return Err(error.to_string());
        }
    }
    if let Some(history) = app.try_state::<Arc<TtsHistoryManager>>() {
        for scope in [
            interactive_retention_changed.then_some(TtsHistoryScope::Interactive),
            file_retention_changed.then_some(TtsHistoryScope::File),
        ]
        .into_iter()
        .flatten()
        {
            if let Err(error) = history.enforce_retention(scope) {
                report_history_error(
                    &app,
                    "TTS History limits were saved, but old results could not be removed",
                    error,
                    false,
                );
            }
        }
    }

    Ok(response)
}

#[tauri::command]
#[specta::specta]
pub fn tts_has_api_key(provider: String) -> Result<bool, String> {
    let provider = parse_provider(&provider)?;
    if !provider.requires_api_key() {
        return Ok(false);
    }
    Ok(crate::secure_keys::has_tts_api_key(provider.as_str()))
}

#[tauri::command]
#[specta::specta]
pub fn tts_set_api_key(provider: String, api_key: String) -> Result<(), String> {
    let provider = parse_provider(&provider)?;
    if !provider.requires_api_key() {
        return Err(format!("{} does not use an API key", provider.as_str()));
    }
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return Err("API key cannot be empty".to_string());
    }
    if provider == TtsProvider::Soniox && api_key.chars().count() > SONIOX_TTS_API_KEY_MAX_CHARS {
        return Err(format!(
            "Soniox TTS API key must not exceed {} characters",
            SONIOX_TTS_API_KEY_MAX_CHARS
        ));
    }
    crate::secure_keys::set_tts_api_key(provider.as_str(), api_key)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn tts_clear_api_key(provider: String) -> Result<(), String> {
    let provider = parse_provider(&provider)?;
    if !provider.requires_api_key() {
        return Ok(());
    }
    crate::secure_keys::clear_tts_api_key(provider.as_str()).map_err(|error| error.to_string())
}

fn validate_tts_llm_provider(app: &AppHandle, provider_id: &str) -> Result<String, String> {
    let provider_id = provider_id.trim();
    if provider_id.is_empty() {
        return Err("TTS AI cleanup provider cannot be empty".to_string());
    }
    let settings = get_settings(app);
    let provider = settings
        .post_process_provider(provider_id)
        .ok_or_else(|| format!("Unknown TTS AI cleanup provider: {provider_id}"))?;
    if provider.id == APPLE_INTELLIGENCE_PROVIDER_ID {
        return Err("Apple Intelligence is not yet supported by TTS AI text cleanup".to_string());
    }
    Ok(provider.id.clone())
}

#[tauri::command]
#[specta::specta]
pub fn tts_llm_has_api_key(app: AppHandle, provider_id: String) -> Result<bool, String> {
    let provider_id = validate_tts_llm_provider(&app, &provider_id)?;
    Ok(crate::secure_keys::has_tts_llm_api_key(&provider_id))
}

#[tauri::command]
#[specta::specta]
pub fn tts_llm_set_api_key(
    app: AppHandle,
    provider_id: String,
    api_key: String,
) -> Result<(), String> {
    let provider_id = validate_tts_llm_provider(&app, &provider_id)?;
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return Err("API key cannot be empty".to_string());
    }
    crate::secure_keys::set_tts_llm_api_key(&provider_id, api_key)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn tts_llm_clear_api_key(app: AppHandle, provider_id: String) -> Result<(), String> {
    let provider_id = validate_tts_llm_provider(&app, &provider_id)?;
    crate::secure_keys::clear_tts_llm_api_key(&provider_id).map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn fetch_tts_llm_models(app: AppHandle) -> Result<Vec<String>, String> {
    let settings = get_settings(&app);
    let (provider, api_key) =
        tts_llm::resolve_provider_and_key(&settings).map_err(|error| error.to_string())?;
    let mut models = crate::llm_client::fetch_models(&provider, api_key.clone())
        .await
        .map_err(|error| tts_llm::safe_provider_error(&error, &api_key))?;
    models.sort();
    models.dedup();
    Ok(models)
}

#[tauri::command]
#[specta::specta]
pub async fn run_tts_llm_benchmark(
    app: AppHandle,
    scope: TtsLlmScope,
) -> Result<LlmPostProcessBenchmarkResult, String> {
    let mut settings = get_settings(&app);
    match scope {
        TtsLlmScope::Interactive => settings.tts.llm_preprocessing.interactive_enabled = true,
        TtsLlmScope::File => settings.tts.llm_preprocessing.file_enabled = true,
    }
    let config = tts_llm::resolve_config(&settings, scope)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "TTS AI cleanup is not configured".to_string())?;
    let user_message = match scope {
        TtsLlmScope::Interactive => settings
            .tts
            .llm_preprocessing
            .interactive_benchmark_text
            .clone(),
        TtsLlmScope::File => settings.tts.llm_preprocessing.file_benchmark_text.clone(),
    };
    if user_message.trim().is_empty() {
        return Err("TTS AI cleanup benchmark text cannot be empty".to_string());
    }
    let timestamp_ms = chrono::Utc::now().timestamp_millis();
    let started = Instant::now();
    let response = tts_llm::preprocess_text(&user_message, &config, |_| {}).await;
    let duration_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    let (response_text, error) = match response {
        Ok(response) => (response, None),
        Err(error) => (String::new(), Some(error.to_string())),
    };
    let output_chars = response_text.chars().count();
    let chars_per_second = if duration_ms == 0 {
        output_chars as f64
    } else {
        output_chars as f64 / (duration_ms as f64 / 1_000.0)
    };
    Ok(LlmPostProcessBenchmarkResult {
        timestamp_ms,
        provider_id: config.provider.id,
        provider_label: config.provider.label,
        model: config.model,
        duration_ms,
        chars_per_second,
        input_chars: user_message.chars().count(),
        output_chars,
        success: error.is_none(),
        system_prompt: config.instructions,
        user_message,
        response_text,
        error,
    })
}

#[tauri::command]
#[specta::specta]
pub fn get_local_tts_status(
    kind: LocalTtsKind,
    manager: tauri::State<'_, Arc<TtsManager>>,
) -> Result<LocalTtsStatus, String> {
    Ok(manager.local_tts_status(kind))
}

#[tauri::command]
#[specta::specta]
pub async fn install_local_tts(
    kind: LocalTtsKind,
    source_trusted: bool,
    risk_acknowledged: bool,
    app: AppHandle,
    manager: tauri::State<'_, Arc<TtsManager>>,
) -> Result<LocalTtsStatus, String> {
    require_local_tts_install_consent(source_trusted, risk_acknowledged)?;
    let reserve_mb = get_settings(&app).tts.disk_reserve_mb;
    manager
        .install_local_tts(kind, reserve_mb)
        .await
        .map_err(|error| error.to_string())
}

fn require_local_tts_install_consent(
    source_trusted: bool,
    risk_acknowledged: bool,
) -> Result<(), String> {
    if !source_trusted {
        return Err("Confirm that you trust the selected model source before downloading".into());
    }
    if !risk_acknowledged {
        return Err(
            "Confirm that you understand the local model installation risks before downloading"
                .into(),
        );
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn cancel_local_tts_install(
    kind: LocalTtsKind,
    manager: tauri::State<'_, Arc<TtsManager>>,
) -> Result<bool, String> {
    Ok(manager.cancel_local_tts_install(kind))
}

#[tauri::command]
#[specta::specta]
pub async fn delete_local_tts(
    kind: LocalTtsKind,
    manager: tauri::State<'_, Arc<TtsManager>>,
) -> Result<(), String> {
    manager
        .delete_local_tts(kind)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn inspect_tts_text_file(
    app: AppHandle,
    path: PathBuf,
) -> Result<TextFileInspection, String> {
    let settings = get_settings(&app)
        .tts
        .effective_for_scope(TtsOperationScope::File);
    let manager = app.state::<Arc<TtsManager>>().inner().clone();
    manager
        .inspect_text_file(path, &settings)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn convert_tts_text_file(
    app: AppHandle,
    request: ConvertTtsTextFileRequest,
) -> Result<ConvertTtsTextFileResponse, String> {
    let mut settings = get_settings(&app)
        .tts
        .effective_for_scope(TtsOperationScope::File);
    settings.output_format = request.output_format;
    settings.mp3_bitrate_kbps = if SUPPORTED_MP3_BITRATES.contains(&request.mp3_bitrate) {
        request.mp3_bitrate
    } else {
        256
    };
    let manager = app.state::<Arc<TtsManager>>().inner().clone();
    let history_source = if settings.file_history_enabled {
        Some(
            manager
                .read_original_text_file(&request.input_path)
                .map_err(|error| error.to_string())?,
        )
    } else {
        None
    };
    let history_source_kind = request
        .input_path
        .extension()
        .and_then(|extension| extension.to_str())
        .filter(|extension| extension.eq_ignore_ascii_case("md"))
        .map(|_| TtsHistorySourceKind::Markdown)
        .unwrap_or(TtsHistorySourceKind::Text);
    let resolved = manager
        .convert_text_file_resolved(&request.input_path, &request.output_path, &settings)
        .await
        .map_err(|error| error.to_string())?;
    let result = resolved.value;
    settings = resolved.settings;
    if let Some(source_text) = history_source {
        let _ = save_passive_history(
            &app,
            &settings,
            history_metadata(
                &settings,
                TtsHistoryScope::File,
                source_text,
                history_source_kind,
                format!(
                    "file-{}-{}",
                    chrono::Utc::now().timestamp_millis(),
                    result.operation_id
                ),
                Some(result.output_path.clone()),
            ),
            result.output_path.clone(),
            false,
        );
    }
    Ok(ConvertTtsTextFileResponse::from(result))
}

#[tauri::command]
#[specta::specta]
pub async fn list_tts_file_jobs(
    manager: tauri::State<'_, Arc<TtsManager>>,
) -> Result<Vec<UiFileJobSummary>, String> {
    let manager = manager.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.list_ui_file_jobs())
        .await
        .map_err(|error| format!("TTS job recovery scan failed: {error}"))?
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn start_tts_file_job(
    app: AppHandle,
    request: ConvertTtsTextFileRequest,
) -> Result<TtsFileJobConversionResponse, String> {
    let mut settings = get_settings(&app)
        .tts
        .effective_for_scope(TtsOperationScope::File);
    settings.output_format = request.output_format;
    settings.mp3_bitrate_kbps = if SUPPORTED_MP3_BITRATES.contains(&request.mp3_bitrate) {
        request.mp3_bitrate
    } else {
        256
    };
    let manager = app.state::<Arc<TtsManager>>().inner().clone();
    let (job_id, resolved) = manager
        .create_ui_file_job(&request.input_path, &request.output_path, &settings)
        .await
        .map_err(|error| error.to_string())?;
    let result = resolved.value;
    finish_ui_file_job(&app, &manager, &job_id, &resolved.settings, &result);
    Ok(TtsFileJobConversionResponse {
        job_id,
        conversion: ConvertTtsTextFileResponse::from(result),
    })
}

#[tauri::command]
#[specta::specta]
pub async fn resume_tts_file_job(
    app: AppHandle,
    job_id: String,
) -> Result<TtsFileJobConversionResponse, String> {
    let manager = app.state::<Arc<TtsManager>>().inner().clone();
    let resolved = manager
        .resume_ui_file_job(&job_id)
        .await
        .map_err(|error| error.to_string())?;
    let result = resolved.value;
    finish_ui_file_job(&app, &manager, &job_id, &resolved.settings, &result);
    Ok(TtsFileJobConversionResponse {
        job_id,
        conversion: ConvertTtsTextFileResponse::from(result),
    })
}

#[tauri::command]
#[specta::specta]
pub fn discard_tts_file_job(
    manager: tauri::State<'_, Arc<TtsManager>>,
    job_id: String,
) -> Result<(), String> {
    manager
        .discard_ui_file_job(&job_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn export_tts_file_job_partial(
    manager: tauri::State<'_, Arc<TtsManager>>,
    job_id: String,
    destination: PathBuf,
) -> Result<usize, String> {
    let manager = manager.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        manager.export_ui_file_job_partial(&job_id, &destination)
    })
    .await
    .map_err(|error| format!("Partial TTS export task failed: {error}"))?
    .map_err(|error| error.to_string())
}

fn finish_ui_file_job(
    app: &AppHandle,
    manager: &Arc<TtsManager>,
    job_id: &str,
    settings: &TtsSettings,
    result: &FileConversionResult,
) {
    if settings.file_history_enabled {
        match manager.ui_file_job_history_source(job_id) {
            Ok((source_text, source_path)) => {
                let source_kind = source_path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .filter(|extension| extension.eq_ignore_ascii_case("md"))
                    .map(|_| TtsHistorySourceKind::Markdown)
                    .unwrap_or(TtsHistorySourceKind::Text);
                let _ = save_passive_history(
                    app,
                    settings,
                    history_metadata(
                        settings,
                        TtsHistoryScope::File,
                        source_text,
                        source_kind,
                        format!(
                            "file-job-{}-{}",
                            chrono::Utc::now().timestamp_millis(),
                            result.operation_id
                        ),
                        Some(result.output_path.clone()),
                    ),
                    result.output_path.clone(),
                    false,
                );
            }
            Err(error) => log::warn!("Unable to load completed TTS job History source: {error}"),
        }
    }
    if let Err(error) = manager.complete_ui_file_job(job_id) {
        log::warn!("Unable to remove completed TTS job {job_id}: {error}");
    }
}

#[tauri::command]
#[specta::specta]
pub async fn scan_tts_batch_files(
    manager: tauri::State<'_, Arc<TtsManager>>,
    request: TtsBatchScanRequest,
) -> Result<TtsBatchScanResult, String> {
    let manager = manager.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.scan_batch_files(request))
        .await
        .map_err(|error| format!("TTS batch scan task failed: {error}"))?
        .map_err(|error| error.to_string())
}

fn batch_file_result(
    index: usize,
    plan: &TtsBatchFilePlan,
    status: TtsBatchFileStatus,
    error: Option<String>,
) -> TtsBatchFileResult {
    TtsBatchFileResult {
        index,
        input_path: plan.input_path.clone(),
        relative_path: plan.relative_path.clone(),
        output_path: plan.output_path.clone(),
        status,
        error,
        warning: None,
        operation_id: None,
        resumed_chunks: 0,
    }
}

fn emit_batch_progress(
    app: &AppHandle,
    client_id: &str,
    batch_id: &str,
    total: usize,
    files: &[TtsBatchFileResult],
    cancelled: bool,
    done: bool,
    message: Option<String>,
    file: Option<TtsBatchFileResult>,
) {
    let progress = TtsBatchProgress {
        client_id: client_id.to_string(),
        batch_id: batch_id.to_string(),
        total,
        finished: files.len(),
        completed: files
            .iter()
            .filter(|file| file.status == TtsBatchFileStatus::Completed)
            .count(),
        skipped: files
            .iter()
            .filter(|file| file.status == TtsBatchFileStatus::Skipped)
            .count(),
        failed: files
            .iter()
            .filter(|file| file.status == TtsBatchFileStatus::Failed)
            .count(),
        cancelled,
        done,
        message,
        file,
    };
    if let Err(error) = app.emit(TTS_EVENT_BATCH_PROGRESS, progress) {
        log::warn!("Failed to emit TTS batch progress: {error}");
    }
}

#[tauri::command]
#[specta::specta]
pub async fn convert_tts_batch(
    app: AppHandle,
    request: ConvertTtsBatchRequest,
) -> Result<TtsBatchSummary, String> {
    let client_id = request.client_id.trim();
    if client_id.is_empty() || client_id.len() > 128 {
        return Err("A valid TTS batch client ID is required".to_string());
    }
    let client_id = client_id.to_string();
    if request.scan.files.is_empty() {
        return Err("The scanned batch contains no supported text files".to_string());
    }
    let manager = app.state::<Arc<TtsManager>>().inner().clone();
    let lease = manager.begin_batch().map_err(|error| error.to_string())?;
    let batch_id = lease.id().to_string();
    let cancellation = Arc::clone(lease.cancellation());
    let total = request.scan.files.len();
    let mut settings = get_settings(&app)
        .tts
        .effective_for_scope(TtsOperationScope::File);
    settings.output_format = request.scan.output_format;
    settings.mp3_bitrate_kbps = if SUPPORTED_MP3_BITRATES.contains(&request.mp3_bitrate) {
        request.mp3_bitrate
    } else {
        256
    };
    let mut files = Vec::with_capacity(total);
    emit_batch_progress(
        &app,
        &client_id,
        &batch_id,
        total,
        &files,
        false,
        false,
        Some("Batch conversion queued".to_string()),
        None,
    );

    for (index, plan) in request.scan.files.iter().enumerate() {
        if cancellation.is_cancelled() {
            for (remaining_index, remaining) in request.scan.files.iter().enumerate().skip(index) {
                files.push(batch_file_result(
                    remaining_index,
                    remaining,
                    TtsBatchFileStatus::Skipped,
                    Some("Batch cancelled before this file started".to_string()),
                ));
            }
            break;
        }
        if let Some(scan_error) = plan.scan_error.clone() {
            let result =
                batch_file_result(index, plan, TtsBatchFileStatus::Failed, Some(scan_error));
            files.push(result.clone());
            emit_batch_progress(
                &app,
                &client_id,
                &batch_id,
                total,
                &files,
                false,
                false,
                None,
                Some(result),
            );
            continue;
        }
        if plan.output_path.exists() {
            let result = batch_file_result(
                index,
                plan,
                TtsBatchFileStatus::Skipped,
                Some(format!(
                    "Output file already exists: {}",
                    plan.output_path.display()
                )),
            );
            files.push(result.clone());
            emit_batch_progress(
                &app,
                &client_id,
                &batch_id,
                total,
                &files,
                false,
                false,
                None,
                Some(result),
            );
            continue;
        }

        let queued = batch_file_result(index, plan, TtsBatchFileStatus::Queued, None);
        emit_batch_progress(
            &app,
            &client_id,
            &batch_id,
            total,
            &files,
            false,
            false,
            Some("Waiting for the current text-to-speech operation".to_string()),
            Some(queued),
        );
        let operation_guard = match manager
            .wait_for_batch_foreground_slot(cancellation.as_ref())
            .await
        {
            Ok(guard) => guard,
            Err(error) => {
                files.push(batch_file_result(
                    index,
                    plan,
                    TtsBatchFileStatus::Skipped,
                    Some(error.to_string()),
                ));
                for (remaining_index, remaining) in
                    request.scan.files.iter().enumerate().skip(index + 1)
                {
                    files.push(batch_file_result(
                        remaining_index,
                        remaining,
                        TtsBatchFileStatus::Skipped,
                        Some("Batch cancelled before this file started".to_string()),
                    ));
                }
                break;
            }
        };
        let processing = batch_file_result(index, plan, TtsBatchFileStatus::Processing, None);
        emit_batch_progress(
            &app,
            &client_id,
            &batch_id,
            total,
            &files,
            false,
            false,
            None,
            Some(processing),
        );

        match manager
            .convert_batch_file_resolved(
                plan,
                request.scan.input_directory.as_deref(),
                request.scan.recursive,
                &request.scan.output_directory,
                &settings,
                operation_guard,
                cancellation.as_ref(),
            )
            .await
        {
            Ok((resolved, source_text, job_id)) => {
                let result = resolved.value;
                let resolved_settings = resolved.settings;
                let source_kind = plan
                    .input_path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .filter(|extension| extension.eq_ignore_ascii_case("md"))
                    .map(|_| TtsHistorySourceKind::Markdown)
                    .unwrap_or(TtsHistorySourceKind::Text);
                let warning = save_passive_history(
                    &app,
                    &resolved_settings,
                    history_metadata(
                        &resolved_settings,
                        TtsHistoryScope::File,
                        source_text,
                        source_kind,
                        format!(
                            "batch-{}-{}",
                            chrono::Utc::now().timestamp_millis(),
                            result.operation_id
                        ),
                        Some(result.output_path.clone()),
                    ),
                    result.output_path.clone(),
                    false,
                );
                let mut completed =
                    batch_file_result(index, plan, TtsBatchFileStatus::Completed, None);
                completed.output_path = result.output_path;
                completed.operation_id = Some(result.operation_id.to_string());
                completed.resumed_chunks = result.resumed_chunks;
                completed.warning = warning;
                if let Err(error) = manager.complete_ui_file_job(&job_id) {
                    log::warn!("Unable to remove completed batch TTS job {job_id}: {error}");
                }
                files.push(completed.clone());
                emit_batch_progress(
                    &app,
                    &client_id,
                    &batch_id,
                    total,
                    &files,
                    false,
                    false,
                    None,
                    Some(completed),
                );
            }
            Err(error) => {
                let message = error.to_string();
                let status = if message.contains("Output file already exists") {
                    TtsBatchFileStatus::Skipped
                } else {
                    TtsBatchFileStatus::Failed
                };
                let failed = batch_file_result(index, plan, status, Some(message));
                files.push(failed.clone());
                emit_batch_progress(
                    &app,
                    &client_id,
                    &batch_id,
                    total,
                    &files,
                    cancellation.is_cancelled(),
                    false,
                    None,
                    Some(failed),
                );
            }
        }
    }

    let cancelled = cancellation.is_cancelled();
    let summary = TtsBatchSummary {
        client_id: client_id.clone(),
        batch_id: batch_id.clone(),
        total,
        finished: files.len(),
        completed: files
            .iter()
            .filter(|file| file.status == TtsBatchFileStatus::Completed)
            .count(),
        skipped: files
            .iter()
            .filter(|file| file.status == TtsBatchFileStatus::Skipped)
            .count(),
        failed: files
            .iter()
            .filter(|file| file.status == TtsBatchFileStatus::Failed)
            .count(),
        cancelled,
        files,
    };
    emit_batch_progress(
        &app,
        &client_id,
        &batch_id,
        total,
        &summary.files,
        cancelled,
        true,
        Some(if cancelled {
            "Batch conversion cancelled".to_string()
        } else {
            "Batch conversion finished".to_string()
        }),
        None,
    );
    drop(lease);
    Ok(summary)
}

#[tauri::command]
#[specta::specta]
pub fn cancel_tts_batch(app: AppHandle, batch_id: String) -> Result<bool, String> {
    let batch_id = batch_id
        .trim()
        .parse::<u64>()
        .map_err(|_| "The TTS batch ID is invalid".to_string())?;
    Ok(app.state::<Arc<TtsManager>>().cancel_batch(batch_id))
}

#[tauri::command]
#[specta::specta]
pub fn get_tts_overlay_state(app: AppHandle) -> TtsOverlayState {
    app.state::<TtsOverlayRuntime>().inner.lock().state.clone()
}

#[tauri::command]
#[specta::specta]
pub fn cancel_tts_operation(app: AppHandle, operation_id: Option<String>) -> Result<(), String> {
    let manager = app.state::<Arc<TtsManager>>();
    let requested = if let Some(operation_id) = operation_id.filter(|value| !value.trim().is_empty())
    {
        operation_id
            .parse::<u64>()
            .map_err(|_| "The TTS operation ID is invalid".to_string())?
    } else {
        let current = manager.current_state();
        if current.kind != Some(TtsOperationKind::Interactive) {
            return Ok(());
        }
        current.operation_id
    };
    // Cancellation is intentionally idempotent. A completed overlay can still
    // stop local playback, but its stale ID must never cancel a newer export.
    manager.cancel_operation(requested);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn tts_overlay_playback_state(
    app: AppHandle,
    operation_id: String,
    status: String,
    current_chunk: Option<usize>,
) -> Result<(), String> {
    if !matches!(
        status.as_str(),
        "playing" | "paused" | "stopped" | "completed"
    ) {
        return Err("Unsupported TTS playback state".to_string());
    }
    let runtime = app.state::<TtsOverlayRuntime>();
    let mut first_playback_latency = None;
    let snapshot = {
        let mut runtime = runtime.inner.lock();
        if runtime.state.operation_id != operation_id {
            return Err("The requested TTS operation is no longer active".to_string());
        }
        if status == "playing" {
            first_playback_latency = runtime
                .latency
                .record_first_playback(Instant::now(), current_chunk);
        }
        runtime.playback_status = Some(status.clone());
        runtime.state.status = status;
        // Autoplay is a one-shot request. Once the renderer has started or
        // explicitly controlled playback, a renderer reload must not replay
        // the retained audio snapshot.
        runtime.state.autoplay = false;
        if let Some(current_chunk) = current_chunk {
            runtime.state.current_chunk = current_chunk.saturating_add(1);
        }
        runtime.state.clone()
    };
    if let Some(latency) = first_playback_latency {
        let reference_target_ms = if latency.overlay_was_cold {
            TTS_FIRST_PLAYBACK_COLD_TARGET_MS
        } else {
            TTS_FIRST_PLAYBACK_WARM_TARGET_MS
        };
        let ready_to_playing_ms = latency
            .chunk_ready_to_playing_ms
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let chunk_ready_ms = latency
            .activation_to_chunk_ready_ms
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let target_applies = latency.autoplay
            && !latency.had_retry
            && (300..=350).contains(&latency.input_characters);
        let target_met = target_applies && latency.activation_to_playing_ms <= reference_target_ms;
        let local_target_met = latency
            .chunk_ready_to_playing_ms
            .is_some_and(|value| value <= TTS_CHUNK_READY_TO_PLAYING_TARGET_MS);
        let log_message = format!(
            "TTS latency milestone=first_playback activation_to_playing_ms={} activation_to_chunk_ready_ms={} chunk_ready_to_playing_ms={} overlay={} input_chars={} retry={} autoplay={} reference_target_ms={} target_applies={} target_met={} local_target_ms={} local_target_met={} operation_id={}",
            latency.activation_to_playing_ms,
            chunk_ready_ms,
            ready_to_playing_ms,
            if latency.overlay_was_cold { "cold" } else { "warm" },
            latency.input_characters,
            latency.had_retry,
            latency.autoplay,
            reference_target_ms,
            target_applies,
            target_met,
            TTS_CHUNK_READY_TO_PLAYING_TARGET_MS,
            local_target_met,
            operation_id
        );
        if target_applies && (!target_met || !local_target_met) {
            log::warn!("{log_message}");
        } else {
            log::info!("{log_message}");
        }
    }
    emit_overlay_state(&app, &snapshot);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn local_tts_install_requires_both_explicit_confirmations() {
        assert!(require_local_tts_install_consent(false, false).is_err());
        assert!(require_local_tts_install_consent(true, false).is_err());
        assert!(require_local_tts_install_consent(false, true).is_err());
        assert!(require_local_tts_install_consent(true, true).is_ok());
    }

    #[test]
    fn murf_voice_normalization_keeps_model_specific_builtin_defaults_compatible() {
        assert_eq!(
            normalize_murf_voice("gen2", DEFAULT_TTS_MURF_VOICE.to_string()),
            DEFAULT_TTS_MURF_GEN2_VOICE
        );
        assert_eq!(
            normalize_murf_voice("falcon-2", DEFAULT_TTS_MURF_GEN2_VOICE.to_string()),
            DEFAULT_TTS_MURF_VOICE
        );
        assert_eq!(
            normalize_murf_voice("gen2", "custom-voice".to_string()),
            "custom-voice"
        );
    }

    #[test]
    fn latency_trace_records_only_the_first_chunk_and_first_playback() {
        let started = Instant::now();
        let mut trace = TtsLatencyTrace::default();
        trace.start(started, true, 320, true);

        assert_eq!(
            trace.record_first_chunk_ready(started + Duration::from_millis(2_500)),
            Some(2_500)
        );
        assert_eq!(
            trace.record_first_chunk_ready(started + Duration::from_millis(2_600)),
            None
        );
        assert!(trace
            .record_first_playback(started + Duration::from_millis(2_650), Some(1))
            .is_none());

        let sample = trace
            .record_first_playback(started + Duration::from_millis(2_700), Some(0))
            .expect("first chunk playback should produce one latency sample");
        assert_eq!(sample.activation_to_playing_ms, 2_700);
        assert_eq!(sample.activation_to_chunk_ready_ms, Some(2_500));
        assert_eq!(sample.chunk_ready_to_playing_ms, Some(200));
        assert!(sample.overlay_was_cold);
        assert_eq!(sample.input_characters, 320);
        assert!(sample.autoplay);
        assert!(!sample.had_retry);
        assert!(trace
            .record_first_playback(started + Duration::from_millis(2_800), Some(0))
            .is_none());
    }

    #[test]
    fn overlay_identity_tracks_the_resolved_provider_settings() {
        let mut settings = TtsSettings {
            provider: TtsProvider::OpenAi,
            openai_model: "gpt-4o-mini-tts-test".to_string(),
            openai_voice: "coral".to_string(),
            ..TtsSettings::default()
        };
        assert_eq!(
            overlay_identity(&settings),
            TtsOverlayIdentity {
                provider: "openai".to_string(),
                model: "gpt-4o-mini-tts-test".to_string(),
                voice: "coral".to_string(),
            }
        );

        settings.provider = TtsProvider::Windows;
        settings.windows_voice_id = "resolved-windows-voice".to_string();
        assert_eq!(
            overlay_identity(&settings),
            TtsOverlayIdentity {
                provider: "windows".to_string(),
                model: "windows.media.speechsynthesis".to_string(),
                voice: "resolved-windows-voice".to_string(),
            }
        );
    }

    #[test]
    fn watcher_restarts_only_for_effective_watcher_configuration_changes() {
        let previous = TtsSettings::default();
        let mut current = previous.clone();
        current.openai_voice = "coral".to_string();
        assert!(!watcher_configuration_changed(&previous, &current));

        current.watch_input_directory = " C:\\TTS\\Input ".to_string();
        assert!(!watcher_configuration_changed(&previous, &current));

        current.watch_folder_enabled = true;
        assert!(watcher_configuration_changed(&previous, &current));

        let mut enabled_previous = current.clone();
        enabled_previous.watch_output_directory = "C:\\TTS\\Output".to_string();
        let mut enabled_current = enabled_previous.clone();
        enabled_current.openai_model = "gpt-4o-mini-tts-2026".to_string();
        assert!(!watcher_configuration_changed(
            &enabled_previous,
            &enabled_current
        ));

        enabled_current.watch_output_directory = "C:\\TTS\\Other".to_string();
        assert!(watcher_configuration_changed(
            &enabled_previous,
            &enabled_current
        ));
    }

    #[test]
    fn normalization_bounds_overlay_auto_hide_delay() {
        let mut settings = TtsSettings::default();
        settings.overlay_auto_hide_delay_seconds = 0;
        assert_eq!(
            normalize_settings(settings).overlay_auto_hide_delay_seconds,
            1
        );

        let mut settings = TtsSettings::default();
        settings.overlay_auto_hide_delay_seconds = u32::MAX;
        assert_eq!(
            normalize_settings(settings).overlay_auto_hide_delay_seconds,
            300
        );
    }

    #[test]
    fn normalization_restores_independent_tts_llm_prompt_scopes_and_bounds() {
        let mut settings = TtsSettings::default();
        settings.llm_preprocessing.interactive_prompts.clear();
        settings.llm_preprocessing.file_prompts.clear();
        settings.llm_preprocessing.interactive_selected_prompt_id = "missing".to_string();
        settings.llm_preprocessing.file_selected_prompt_id = "missing".to_string();
        settings.llm_preprocessing.chunk_target_chars = 10;
        settings.llm_preprocessing.retry_count = 99;
        settings.llm_preprocessing.request_timeout_seconds = 1;

        let normalized = normalize_settings(settings);
        assert!(normalized.llm_preprocessing.interactive_prompts.len() >= 4);
        assert!(normalized.llm_preprocessing.file_prompts.len() >= 4);
        assert!(normalized
            .llm_preprocessing
            .interactive_prompts
            .iter()
            .any(|prompt| {
                prompt.id == normalized.llm_preprocessing.interactive_selected_prompt_id
            }));
        assert!(normalized
            .llm_preprocessing
            .file_prompts
            .iter()
            .any(|prompt| prompt.id == normalized.llm_preprocessing.file_selected_prompt_id));
        assert_eq!(normalized.llm_preprocessing.chunk_target_chars, 1_000);
        assert_eq!(normalized.llm_preprocessing.retry_count, 10);
        assert_eq!(normalized.llm_preprocessing.request_timeout_seconds, 10);
        assert_ne!(
            normalized.llm_preprocessing.interactive_selected_prompt_id,
            normalized.llm_preprocessing.file_selected_prompt_id
        );
    }

    #[test]
    fn invalid_tts_llm_prompt_edits_are_rejected_instead_of_silently_deleted() {
        let mut empty_prompt = TtsSettings::default();
        empty_prompt.llm_preprocessing.interactive_prompts[0]
            .prompt
            .clear();
        let normalized = normalize_settings(empty_prompt);
        assert!(normalized.llm_preprocessing.interactive_prompts[0]
            .prompt
            .is_empty());
        let error = TtsManager::validate_settings(&normalized)
            .expect_err("empty prompt body must be rejected")
            .to_string();
        assert!(error.contains("cannot be empty"));

        let mut duplicate_name = TtsSettings::default();
        let first_name = duplicate_name.llm_preprocessing.file_prompts[0]
            .name
            .clone();
        duplicate_name.llm_preprocessing.file_prompts[1].name = first_name.to_uppercase();
        let error = TtsManager::validate_settings(&duplicate_name)
            .expect_err("duplicate prompt names must be rejected")
            .to_string();
        assert!(error.contains("prompt names must be unique"));
    }

    #[test]
    fn normalization_seeds_independent_interactive_and_file_synthesis_scopes() {
        let settings = TtsSettings {
            provider: TtsProvider::OpenAi,
            openai_model: "gpt-4o-mini-tts".to_string(),
            openai_voice: "marin".to_string(),
            interactive_target_chars: 320,
            file_target_chars: 1_750,
            ..TtsSettings::default()
        };

        let normalized = normalize_settings(settings);
        let interactive = normalized.effective_for_scope(TtsOperationScope::Interactive);
        let file = normalized.effective_for_scope(TtsOperationScope::File);

        assert_eq!(interactive.provider, TtsProvider::OpenAi);
        assert_eq!(file.provider, TtsProvider::OpenAi);
        assert_eq!(interactive.interactive_target_chars, 320);
        assert_eq!(file.file_target_chars, 1_750);
        assert_eq!(normalized.interactive_synthesis.models.len(), 1);
        assert_eq!(normalized.file_synthesis.models.len(), 1);
    }

    #[test]
    fn switching_models_restores_settings_per_model_and_scope() {
        let mut settings = normalize_settings(TtsSettings {
            provider: TtsProvider::OpenAi,
            openai_model: "gpt-4o-mini-tts".to_string(),
            openai_voice: "marin".to_string(),
            speed: 1.25,
            ..TtsSettings::default()
        });

        let deepgram_request = TtsSettings {
            provider: TtsProvider::Deepgram,
            deepgram_model: "aura-2-thalia-en".to_string(),
            speed: 0.9,
            ..settings.effective_for_scope(TtsOperationScope::Interactive)
        };
        settings.select_scope_model(TtsOperationScope::Interactive, &deepgram_request);
        let deepgram = settings.effective_for_scope(TtsOperationScope::Interactive);
        assert_eq!(deepgram.provider, TtsProvider::Deepgram);
        assert_eq!(deepgram.speed, 0.9);

        let openai_request = TtsSettings {
            provider: TtsProvider::OpenAi,
            openai_model: "gpt-4o-mini-tts".to_string(),
            ..deepgram
        };
        settings.select_scope_model(TtsOperationScope::Interactive, &openai_request);
        let restored = settings.effective_for_scope(TtsOperationScope::Interactive);
        assert_eq!(restored.provider, TtsProvider::OpenAi);
        assert_eq!(restored.openai_voice, "marin");
        assert_eq!(restored.speed, 1.25);

        let file = settings.effective_for_scope(TtsOperationScope::File);
        assert_eq!(file.provider, TtsProvider::OpenAi);
        assert_eq!(file.speed, 1.25);
    }

    #[test]
    fn synthesis_presets_are_shared_but_page_selection_is_independent() {
        let mut settings = normalize_settings(TtsSettings::default());
        let mut config =
            TtsSynthesisConfig::from_settings(&settings, TtsOperationScope::Interactive);
        config.provider = TtsProvider::OpenAi;
        config.model = "gpt-4o-mini-tts".to_string();
        config.voice = "coral".to_string();
        config.speed = 1.1;
        settings
            .synthesis_presets
            .push(crate::settings::TtsSynthesisPreset {
                id: "narrator".to_string(),
                name: "Narrator".to_string(),
                config,
            });

        settings
            .load_synthesis_preset(TtsOperationScope::File, "narrator")
            .expect("shared preset should load on File Operations");

        assert!(settings.interactive_synthesis.selected_preset_id.is_empty());
        assert_eq!(settings.file_synthesis.selected_preset_id, "narrator");
        let file = settings.effective_for_scope(TtsOperationScope::File);
        assert_eq!(file.provider, TtsProvider::OpenAi);
        assert_eq!(file.openai_voice, "coral");
        assert_eq!(file.speed, 1.1);
    }

    #[test]
    fn synthesis_profiles_exclude_llm_history_hotkey_and_path_settings() {
        let mut settings = TtsSettings::default();
        settings.llm_preprocessing.file_enabled = true;
        settings.llm_preprocessing.file_selected_prompt_id = "cleanup".to_string();
        settings.watch_input_directory = r"C:\TTS\input".to_string();
        settings.watch_output_directory = r"C:\TTS\output".to_string();
        settings.play_pause_hotkey = "Space".to_string();
        settings.file_history_enabled = true;

        let config = TtsSynthesisConfig::from_settings(&settings, TtsOperationScope::File);
        let serialized = serde_json::to_value(config).expect("serialize synthesis profile");

        for forbidden in [
            "llm_preprocessing",
            "file_history_enabled",
            "watch_input_directory",
            "watch_output_directory",
            "play_pause_hotkey",
            "api_key",
        ] {
            assert!(
                !serialized.get(forbidden).is_some(),
                "unexpected {forbidden}"
            );
        }
        assert_eq!(serialized["provider"], serde_json::json!("soniox"));
        assert_eq!(
            serialized["target_chars"],
            serde_json::json!(settings.file_target_chars)
        );
    }

    #[test]
    fn duplicate_synthesis_preset_names_are_rejected_case_insensitively() {
        let mut settings = normalize_settings(TtsSettings::default());
        let config = TtsSynthesisConfig::from_settings(&settings, TtsOperationScope::Interactive);
        settings.synthesis_presets = vec![
            crate::settings::TtsSynthesisPreset {
                id: "one".to_string(),
                name: "Audiobook".to_string(),
                config: config.clone(),
            },
            crate::settings::TtsSynthesisPreset {
                id: "two".to_string(),
                name: "AUDIOBOOK".to_string(),
                config,
            },
        ];

        let error = validate_synthesis_collections(&settings)
            .expect_err("duplicate preset names must be rejected");
        assert!(error.contains("names must be unique"));
    }

    #[test]
    fn tts_llm_benchmark_log_is_bounded_by_count_and_serialized_size() {
        let result = LlmPostProcessBenchmarkResult {
            timestamp_ms: 1,
            provider_id: "openai".to_string(),
            provider_label: "OpenAI".to_string(),
            model: "test-model".to_string(),
            duration_ms: 1,
            chars_per_second: 1.0,
            input_chars: 50_000,
            output_chars: 100_000,
            success: true,
            system_prompt: "prompt".to_string(),
            user_message: "input".repeat(1_000),
            response_text: "output".repeat(20_000),
            error: None,
        };
        let mut log = vec![result; TTS_LLM_BENCHMARK_LOG_MAX_ENTRIES];

        trim_tts_llm_benchmark_log(&mut log);

        assert!(!log.is_empty());
        assert!(log.len() < TTS_LLM_BENCHMARK_LOG_MAX_ENTRIES);
        let serialized_bytes = log
            .iter()
            .map(|entry| {
                serde_json::to_vec(entry)
                    .expect("serialize benchmark")
                    .len()
            })
            .sum::<usize>();
        assert!(serialized_bytes <= TTS_LLM_BENCHMARK_LOG_MAX_BYTES);
    }
}
