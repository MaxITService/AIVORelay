use crate::managers::tts::{
    FileConversionResult, TextFileInspection, TtsChunkReady, TtsManager, TtsOperationKind,
    TtsPhase, TtsState, SUPPORTED_MP3_BITRATES, TTS_EVENT_CHUNK_READY, TTS_EVENT_STATE,
};
use crate::managers::tts_history::{
    metadata_from_settings, NewTtsHistoryEntry, TtsHistoryManager, TtsHistorySourceKind,
};
use crate::settings::{get_settings, write_settings, TtsOutputFormat, TtsProvider, TtsSettings};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tauri::{AppHandle, Emitter, Listener, Manager};

const TTS_OVERLAY_EVENT: &str = "tts-overlay-state";
const TTS_FIRST_PLAYBACK_WARM_TARGET_MS: u64 = 3_000;
const TTS_FIRST_PLAYBACK_COLD_TARGET_MS: u64 = 4_000;
const TTS_CHUNK_READY_TO_PLAYING_TARGET_MS: u64 = 250;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TtsOverlayChunk {
    pub index: usize,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TtsOverlayState {
    pub operation_id: String,
    pub status: String,
    pub provider: String,
    pub text_preview: String,
    pub chunks: Vec<TtsOverlayChunk>,
    pub current_chunk: usize,
    pub total_chunks: usize,
    pub retry_attempt: u8,
    pub error: Option<String>,
    pub play_pause_hotkey: String,
    pub stop_hotkey: String,
    pub autoplay: bool,
}

impl Default for TtsOverlayState {
    fn default() -> Self {
        Self {
            operation_id: String::new(),
            status: "idle".to_string(),
            provider: String::new(),
            text_preview: String::new(),
            chunks: Vec::new(),
            current_chunk: 0,
            total_chunks: 0,
            retry_attempt: 0,
            error: None,
            play_pause_hotkey: String::new(),
            stop_hotkey: String::new(),
            autoplay: true,
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

fn apply_manager_state(app: &AppHandle, manager_state: TtsState) {
    let runtime = app.state::<TtsOverlayRuntime>();
    let snapshot = {
        let mut runtime = runtime.inner.lock();
        let next_operation_id = manager_state.operation_id.to_string();
        if !runtime.state.operation_id.is_empty() && runtime.state.operation_id != next_operation_id
        {
            runtime.state.chunks.clear();
            runtime.playback_status = None;
        }
        runtime.state.operation_id = next_operation_id;
        runtime.state.provider = manager_state
            .provider
            .map(|provider| provider.as_str().to_string())
            .unwrap_or_default();
        runtime.state.current_chunk = manager_state.completed_chunks;
        runtime.state.total_chunks = manager_state.total_chunks;
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
        if !runtime
            .state
            .chunks
            .iter()
            .any(|existing| existing.index == overlay_index)
        {
            runtime.state.chunks.push(TtsOverlayChunk {
                index: overlay_index,
                path: chunk.wav_path.to_string_lossy().into_owned(),
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
        runtime.latency.start(
            activation_started_at,
            overlay_was_cold,
            input_characters,
            settings.autoplay,
        );
        runtime.state = TtsOverlayState {
            operation_id: String::new(),
            status: "loading".to_string(),
            provider: settings.provider.as_str().to_string(),
            text_preview: preview,
            chunks: Vec::new(),
            current_chunk: 0,
            total_chunks: 0,
            retry_attempt: 0,
            error: None,
            play_pause_hotkey: settings.play_pause_hotkey.clone(),
            stop_hotkey: settings.stop_hotkey.clone(),
            autoplay: settings.autoplay,
        };
        runtime.state.clone()
    };
    emit_overlay_state(app, &snapshot);
}

fn report_overlay_error(app: &AppHandle, error: impl std::fmt::Display) {
    let message = error.to_string();
    let runtime = app.state::<TtsOverlayRuntime>();
    let snapshot = {
        let mut runtime = runtime.inner.lock();
        runtime.playback_status = None;
        runtime.state.status = "error".to_string();
        runtime.state.error = Some(message);
        runtime.state.clone()
    };
    emit_overlay_state(app, &snapshot);
}

fn history_metadata(
    settings: &TtsSettings,
    source_text: String,
    source_kind: TtsHistorySourceKind,
    group_id: String,
    external_output_path: Option<PathBuf>,
) -> NewTtsHistoryEntry {
    metadata_from_settings(
        settings,
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
) {
    if !settings.history_enabled {
        return;
    }
    let history = app.state::<Arc<TtsHistoryManager>>();
    if let Err(error) = history.save_success(metadata, audio_path) {
        report_history_error(
            app,
            "TTS completed but its History copy could not be saved",
            error,
            show_error_in_overlay,
        );
    }
}

pub async fn start_tts_text(app: AppHandle, text: String) -> Result<(), String> {
    start_tts_text_at(app, text, Instant::now()).await
}

pub(crate) async fn start_tts_text_at(
    app: AppHandle,
    text: String,
    activation_started_at: Instant,
) -> Result<(), String> {
    let settings = get_settings(&app).tts;
    if !settings.enabled {
        return Err("Text to Speech is disabled".to_string());
    }
    if text.trim().is_empty() {
        return Err("There is no clipboard text to read".to_string());
    }

    let manager = app.state::<Arc<TtsManager>>().inner().clone();
    let operation_guard = manager
        .try_reserve_foreground_operation()
        .map_err(|error| error.to_string())?;
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
    let synthesis = match manager
        .synthesize_interactive_reserved(&text, &settings, operation_guard)
        .await
    {
        Ok(synthesis) => synthesis,
        Err(error) => {
            report_overlay_error(&app, &error);
            return Err(error.to_string());
        }
    };
    if settings.history_enabled {
        if let Some(audio_path) = synthesis.combined_audio_path {
            save_passive_history(
                &app,
                &settings,
                history_metadata(
                    &settings,
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
        _ => Err("Unsupported TTS provider".to_string()),
    }
}

fn normalize_settings(mut settings: TtsSettings) -> TtsSettings {
    settings.soniox_model = nonempty_setting(settings.soniox_model, "tts-rt-v1");
    settings.soniox_language = nonempty_setting(settings.soniox_language, "en");
    settings.soniox_voice = nonempty_setting(settings.soniox_voice, "Maya");
    settings.deepgram_model = nonempty_setting(settings.deepgram_model, "aura-2-thalia-en");
    settings.openai_model = nonempty_setting(settings.openai_model, "gpt-4o-mini-tts");
    settings.openai_voice = nonempty_setting(settings.openai_voice, "marin");
    let hard_limit = TtsManager::provider_character_limit(settings.provider) as u32;
    settings.interactive_target_chars = settings.interactive_target_chars.clamp(50, hard_limit);
    settings.file_target_chars = settings.file_target_chars.clamp(50, hard_limit);
    settings.retry_count = settings.retry_count.min(10);
    settings.retry_base_delay_ms = settings.retry_base_delay_ms.clamp(100, 30_000);
    settings.inter_chunk_pause_ms = settings.inter_chunk_pause_ms.min(5_000);
    settings.paragraph_pause_ms = settings.paragraph_pause_ms.min(10_000);
    settings.watch_settle_delay_ms = settings.watch_settle_delay_ms.clamp(100, 60_000);
    settings.disk_reserve_mb = settings.disk_reserve_mb.min(1_048_576);
    settings.history_max_entries = settings.history_max_entries.clamp(1, 100_000);
    settings.history_max_storage_mb = settings.history_max_storage_mb.clamp(1, 1_048_576);
    settings.speed = match settings.provider {
        TtsProvider::Soniox => settings.speed.clamp(0.7, 1.3),
        TtsProvider::Deepgram => settings.speed.clamp(0.7, 1.5),
        TtsProvider::OpenAi => settings.speed.clamp(0.25, 4.0),
    };
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
    settings
}

fn nonempty_setting(value: String, fallback: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        fallback.to_string()
    } else {
        value.to_string()
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
pub fn update_tts_settings(app: AppHandle, settings: TtsSettings) -> Result<TtsSettings, String> {
    let settings = normalize_settings(settings);
    let mut app_settings = get_settings(&app);
    let previous = app_settings.tts.clone();
    let watcher_configuration_changed = watcher_configuration_changed(&previous, &settings);
    let retention_configuration_changed = previous.history_max_entries
        != settings.history_max_entries
        || previous.history_max_storage_mb != settings.history_max_storage_mb;
    app_settings.tts = settings.clone();
    write_settings(&app, app_settings);

    if watcher_configuration_changed {
        let manager = app.state::<Arc<TtsManager>>().inner().clone();
        if let Err(error) = manager.sync_folder_watcher() {
            let mut rollback = get_settings(&app);
            rollback.tts = previous;
            write_settings(&app, rollback);
            let _ = manager.sync_folder_watcher();
            return Err(error.to_string());
        }
    }
    if retention_configuration_changed {
        if let Some(history) = app.try_state::<Arc<TtsHistoryManager>>() {
            if let Err(error) = history.enforce_retention() {
                report_history_error(
                    &app,
                    "TTS History limits were saved, but old results could not be removed",
                    error,
                    false,
                );
            }
        }
    }

    Ok(settings)
}

#[tauri::command]
#[specta::specta]
pub fn tts_has_api_key(provider: String) -> Result<bool, String> {
    let provider = parse_provider(&provider)?;
    Ok(crate::secure_keys::has_tts_api_key(provider.as_str()))
}

#[tauri::command]
#[specta::specta]
pub fn tts_set_api_key(provider: String, api_key: String) -> Result<(), String> {
    let provider = parse_provider(&provider)?;
    if api_key.trim().is_empty() {
        return Err("API key cannot be empty".to_string());
    }
    crate::secure_keys::set_tts_api_key(provider.as_str(), api_key.trim())
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn tts_clear_api_key(provider: String) -> Result<(), String> {
    let provider = parse_provider(&provider)?;
    crate::secure_keys::clear_tts_api_key(provider.as_str()).map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn inspect_tts_text_file(app: AppHandle, path: PathBuf) -> Result<TextFileInspection, String> {
    let settings = get_settings(&app).tts;
    app.state::<Arc<TtsManager>>()
        .inspect_text_file(path, &settings)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn convert_tts_text_file(
    app: AppHandle,
    request: ConvertTtsTextFileRequest,
) -> Result<ConvertTtsTextFileResponse, String> {
    let mut settings = get_settings(&app).tts;
    settings.output_format = request.output_format;
    settings.mp3_bitrate_kbps = if SUPPORTED_MP3_BITRATES.contains(&request.mp3_bitrate) {
        request.mp3_bitrate
    } else {
        256
    };
    let manager = app.state::<Arc<TtsManager>>().inner().clone();
    let history_source = if settings.history_enabled {
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
    let result = manager
        .convert_text_file(&request.input_path, &request.output_path, &settings)
        .await
        .map_err(|error| error.to_string())?;
    if let Some(source_text) = history_source {
        save_passive_history(
            &app,
            &settings,
            history_metadata(
                &settings,
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
pub fn get_tts_overlay_state(app: AppHandle) -> TtsOverlayState {
    app.state::<TtsOverlayRuntime>().inner.lock().state.clone()
}

#[tauri::command]
#[specta::specta]
pub fn cancel_tts_operation(app: AppHandle, operation_id: Option<String>) -> Result<(), String> {
    let requested = operation_id
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "A TTS operation ID is required".to_string())?
        .parse::<u64>()
        .map_err(|_| "The TTS operation ID is invalid".to_string())?;
    // Cancellation is intentionally idempotent. A completed overlay can still
    // stop local playback, but its stale ID must never cancel a newer export.
    app.state::<Arc<TtsManager>>().cancel_operation(requested);
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
}
