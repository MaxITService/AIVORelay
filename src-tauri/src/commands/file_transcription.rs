//! File transcription commands - transcribe audio files to text
//!
//! Supports common audio formats: wav, mp3, m4a, ogg, flac, webm
//! Uses the same transcription infrastructure as live recording.

use crate::actions::LIVE_SOUND_TRANSCRIPTION_BINDING_ID;
use crate::audio_toolkit::{
    apply_custom_words, clean_transcription_output, detect_output_language, OutputLanguageEvidence,
};
use crate::file_transcription_diarization::{
    create_diarized_transcript_session, normalize_raw_diarized_words,
    normalize_raw_speaker_blocks, reapply_diarized_transcript, render_diarized_subtitle_segments,
    render_diarized_transcript, DiarizedSubtitleSegment, DiarizedTranscriptBlock,
    DiarizedTranscriptProvider, FileTranscriptionSpeakerNameInput,
    FileTranscriptionSpeakerSession, RawDiarizedTranscriptWord, RawSpeakerBlock,
};
use crate::managers::deepgram_stt::{DeepgramSttManager, DeepgramTranscriptionOptions};
use crate::managers::remote_stt::{RemoteFileTranscription, RemoteSttManager};
use crate::managers::soniox_stt::{SonioxAsyncTranscriptionOptions, SonioxSttManager};
use crate::managers::transcription::{
    FileTranscriptionChunkTraceEntry, FileTranscriptionOverrideLoadDecision, TranscriptionManager,
};
use crate::session_manager::{ManagedSessionState, SessionState};
use crate::settings::{
    apply_output_whitespace_policy_for_settings, apply_stt_model_selection, get_settings,
    resolve_live_sound_provider, stt_model_selection_key, stt_model_selection_supports_file,
    write_settings, AppSettings, FileTranscriptionChunkingMode,
    FileTranscriptionModelConfig, SttModelSelection, TranscriptionProfile,
    TranscriptionProvider,
};
use crate::subtitle::{
    get_format_extension, segments_to_srt, segments_to_vtt, timed_tokens_to_subtitle_segments,
    OutputFormat, SubtitleSegment,
};
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

/// Result of a file transcription operation
#[derive(Serialize, Type)]
pub struct FileTranscriptionResult {
    /// The transcribed text (or formatted SRT/VTT content)
    pub text: String,
    /// Path where the file was saved (if save_to_file was true)
    pub saved_file_path: Option<String>,
    /// The segments with timestamps (only populated for SRT/VTT formats)
    pub segments: Option<Vec<SubtitleSegment>>,
    /// Optional informational message for UI display
    pub info_message: Option<String>,
    /// Optional smart-chunking trace for UI/debug display
    pub chunking_trace: Option<Vec<FileTranscriptionChunkTraceEntry>>,
    /// Temporary diarized speaker session for renaming/re-apply
    pub speaker_session: Option<FileTranscriptionSpeakerSession>,
}

#[derive(Serialize, Deserialize, Type, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct SonioxFileTranscriptionOptions {
    pub language_hints: Option<Vec<String>>,
    pub enable_speaker_diarization: Option<bool>,
    pub enable_language_identification: Option<bool>,
}

#[derive(Serialize, Deserialize, Type, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeepgramFileTranscriptionOptions {
    pub diarize: Option<bool>,
    pub multichannel: Option<bool>,
}

#[derive(Serialize, Type, Debug, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct FileTranscriptionRecordingState {
    pub is_recording: bool,
    pub recording_uses_local_model: bool,
    pub file_transcription_uses_local_model: bool,
    pub blocks_file_transcription: bool,
}

fn apply_file_transcription_selection(settings: &mut AppSettings) -> Result<(), String> {
    let Some(selection) = settings.file_transcription_model_selection.clone() else {
        return Ok(());
    };
    apply_stt_model_selection(settings, &selection)
}

fn global_stt_selection(settings: &AppSettings) -> SttModelSelection {
    let (model_id, provider_preset) = match settings.transcription_provider {
        TranscriptionProvider::Local => (settings.selected_model.clone(), String::new()),
        TranscriptionProvider::RemoteSoniox => (settings.soniox_model.clone(), String::new()),
        TranscriptionProvider::RemoteDeepgram => (settings.deepgram_model.clone(), String::new()),
        TranscriptionProvider::RemoteOpenAiCompatible => (
            settings.remote_stt.model_id.clone(),
            settings.remote_stt.provider_preset.clone(),
        ),
    };
    SttModelSelection {
        provider: settings.transcription_provider,
        model_id,
        provider_preset,
    }
}

fn default_file_profile_snapshot(
    settings: &AppSettings,
    selection: &SttModelSelection,
) -> TranscriptionProfile {
    let prompt = settings
        .transcription_prompts
        .get(selection.model_id.trim())
        .cloned()
        .unwrap_or_default();
    TranscriptionProfile {
        id: "file_transcription".to_string(),
        name: "Transcribe File".to_string(),
        language: settings.selected_language.clone(),
        translate_to_english: settings.translate_to_english,
        description: String::new(),
        system_prompt: prompt.clone(),
        stt_prompt_override_enabled: !prompt.trim().is_empty(),
        stt_model_selection_override: Some(selection.clone()),
        include_in_cycle: false,
        push_to_talk: false,
        preview_output_only_enabled: false,
        soniox_language_hints_strict: Some(settings.soniox_language_hints_strict),
        gemini_language_code_override: Some(settings.gemini_language_code.clone()),
        gemini_custom_vocabulary_override: Some(settings.gemini_custom_vocabulary.clone()),
        llm_post_process_enabled: settings.post_process_enabled,
        llm_prompt_override: None,
        llm_model_override: None,
        soniox_context_general_json: settings.soniox_context_general_json.clone(),
        soniox_context_text: settings.soniox_context_text.clone(),
        soniox_context_terms: settings.soniox_context_terms.clone(),
    }
}

fn initial_file_profile_snapshot(
    settings: &AppSettings,
    selection: &SttModelSelection,
) -> TranscriptionProfile {
    if settings.active_profile_id != "default" {
        if let Some(profile) = settings.transcription_profile(&settings.active_profile_id) {
            let mut snapshot = profile.clone();
            snapshot.id = "file_transcription".to_string();
            snapshot.name = "Transcribe File".to_string();
            snapshot.stt_model_selection_override = Some(selection.clone());
            return snapshot;
        }
    }
    default_file_profile_snapshot(settings, selection)
}

fn capture_file_model_config(
    settings: &AppSettings,
    profile_snapshot: Option<TranscriptionProfile>,
) -> FileTranscriptionModelConfig {
    FileTranscriptionModelConfig {
        profile_snapshot,
        chunking_mode: settings.file_transcription_chunking_mode,
        chunking_max_minutes: settings.file_transcription_chunking_max_minutes,
        soniox_language_hints: settings
            .file_soniox_language_hints
            .clone()
            .unwrap_or_else(|| settings.soniox_language_hints.clone()),
        soniox_enable_speaker_diarization: settings
            .file_soniox_enable_speaker_diarization
            .unwrap_or(settings.soniox_enable_speaker_diarization),
        soniox_enable_language_identification: settings
            .file_soniox_enable_language_identification
            .unwrap_or(settings.soniox_enable_language_identification),
        deepgram_diarize: settings
            .file_deepgram_diarize
            .unwrap_or(settings.deepgram_diarize),
        deepgram_multichannel: settings.file_deepgram_multichannel.unwrap_or(false),
        gemini_mode: settings.gemini_file_mode,
        gemini_diarization: settings.gemini_file_diarization,
    }
}

fn file_selections_structurally_compatible(
    current: &SttModelSelection,
    next: &SttModelSelection,
) -> bool {
    if current.provider != next.provider {
        return false;
    }
    if current.provider != TranscriptionProvider::RemoteOpenAiCompatible {
        return true;
    }

    let current_is_gemini = current.model_id.contains("gemini-3.5-transcribe");
    let next_is_gemini = next.model_id.contains("gemini-3.5-transcribe");
    current_is_gemini == next_is_gemini
}

fn seed_file_model_config(
    settings: &AppSettings,
    selection: &SttModelSelection,
) -> FileTranscriptionModelConfig {
    let profile_snapshot = Some(initial_file_profile_snapshot(settings, selection));
    let mut config = if settings
        .file_transcription_model_selection
        .as_ref()
        .is_some_and(|current| file_selections_structurally_compatible(current, selection))
    {
        capture_file_model_config(settings, profile_snapshot)
    } else {
        FileTranscriptionModelConfig {
            profile_snapshot,
            chunking_mode: FileTranscriptionChunkingMode::default(),
            chunking_max_minutes: 0.5,
            soniox_language_hints: settings.soniox_language_hints.clone(),
            soniox_enable_speaker_diarization: settings.soniox_enable_speaker_diarization,
            soniox_enable_language_identification: settings
                .soniox_enable_language_identification,
            deepgram_diarize: settings.deepgram_diarize,
            deepgram_multichannel: false,
            gemini_mode: crate::settings::GeminiTranscriptionMode::Smart,
            gemini_diarization: false,
        }
    };
    if selection.provider == TranscriptionProvider::RemoteOpenAiCompatible
        && selection.provider_preset != crate::url_security::REMOTE_STT_PRESET_GOOGLE
    {
        config.gemini_diarization = false;
    }
    config
}

fn apply_file_model_config(settings: &mut AppSettings, config: &FileTranscriptionModelConfig) {
    settings.file_transcription_chunking_mode = config.chunking_mode;
    settings.file_transcription_chunking_max_minutes = config.chunking_max_minutes;
    settings.file_soniox_language_hints = Some(config.soniox_language_hints.clone());
    settings.file_soniox_enable_speaker_diarization =
        Some(config.soniox_enable_speaker_diarization);
    settings.file_soniox_enable_language_identification =
        Some(config.soniox_enable_language_identification);
    settings.file_deepgram_diarize = Some(config.deepgram_diarize);
    settings.file_deepgram_multichannel = Some(config.deepgram_multichannel);
    settings.gemini_file_mode = config.gemini_mode;
    settings.gemini_file_diarization = config.gemini_diarization;
}

pub(crate) fn sync_active_file_model_config(settings: &mut AppSettings) {
    let Some(selection) = settings.file_transcription_model_selection.clone() else {
        return;
    };
    let key = stt_model_selection_key(&selection);
    let profile_snapshot = settings
        .file_transcription_model_configs
        .get(&key)
        .and_then(|config| config.profile_snapshot.clone())
        .or_else(|| Some(initial_file_profile_snapshot(settings, &selection)));
    let config = capture_file_model_config(settings, profile_snapshot);
    settings.file_transcription_model_configs.insert(key, config);
}

fn compatible_initial_file_selection(settings: &AppSettings) -> SttModelSelection {
    let profile_selection = settings
        .transcription_profile(&settings.active_profile_id)
        .and_then(|profile| profile.stt_model_selection_override.clone())
        .filter(stt_model_selection_supports_file);
    if let Some(selection) = profile_selection {
        return selection;
    }

    let global = global_stt_selection(settings);
    if stt_model_selection_supports_file(&global) {
        return global;
    }

    if !settings.selected_model.trim().is_empty() {
        return SttModelSelection {
            provider: TranscriptionProvider::Local,
            model_id: settings.selected_model.clone(),
            provider_preset: String::new(),
        };
    }

    SttModelSelection {
        provider: TranscriptionProvider::RemoteSoniox,
        model_id: SONIOX_LATEST_ASYNC_MODEL.to_string(),
        provider_preset: String::new(),
    }
}

#[tauri::command]
#[specta::specta]
pub fn initialize_file_transcription_model_settings(app: AppHandle) -> Result<(), String> {
    let mut settings = get_settings(&app);
    if settings.file_transcription_model_selection.is_none() {
        settings.file_transcription_model_selection =
            Some(compatible_initial_file_selection(&settings));
    }

    let selection = settings
        .file_transcription_model_selection
        .clone()
        .ok_or_else(|| "No compatible Transcribe File model is available.".to_string())?;
    if !stt_model_selection_supports_file(&selection) {
        settings.file_transcription_model_selection =
            Some(compatible_initial_file_selection(&settings));
    }
    let selection = settings.file_transcription_model_selection.clone().unwrap();
    let key = stt_model_selection_key(&selection);
    let config = settings
        .file_transcription_model_configs
        .get(&key)
        .cloned()
        .unwrap_or_else(|| seed_file_model_config(&settings, &selection));
    apply_file_model_config(&mut settings, &config);
    settings.file_transcription_model_configs.insert(key, config);
    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_file_transcription_model_selection(
    app: AppHandle,
    selection: SttModelSelection,
) -> Result<(), String> {
    if !stt_model_selection_supports_file(&selection) {
        return Err("This STT model is not compatible with Transcribe File.".to_string());
    }
    let mut settings = get_settings(&app);
    let mut candidate = settings.clone();
    apply_stt_model_selection(&mut candidate, &selection)?;

    sync_active_file_model_config(&mut settings);
    let key = stt_model_selection_key(&selection);
    let config = settings
        .file_transcription_model_configs
        .get(&key)
        .cloned()
        .unwrap_or_else(|| seed_file_model_config(&settings, &selection));
    apply_file_model_config(&mut settings, &config);
    settings.file_transcription_model_selection = Some(selection);
    settings.file_transcription_model_configs.insert(key, config);
    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_file_soniox_speaker_diarization_setting(
    app: AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.file_soniox_enable_speaker_diarization = Some(enabled);
    sync_active_file_model_config(&mut settings);
    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_file_soniox_language_hints_setting(
    app: AppHandle,
    language_hints: Vec<String>,
) -> Result<(), String> {
    let language_hints = crate::settings::normalize_soniox_terms(&language_hints);
    if language_hints.len()
        > crate::managers::soniox_stt::SONIOX_LANGUAGE_HINTS_MAX_COUNT
    {
        return Err(format!(
            "Soniox accepts at most {} language hints.",
            crate::managers::soniox_stt::SONIOX_LANGUAGE_HINTS_MAX_COUNT
        ));
    }
    let mut settings = get_settings(&app);
    settings.file_soniox_language_hints = Some(language_hints);
    sync_active_file_model_config(&mut settings);
    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_file_soniox_language_identification_setting(
    app: AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.file_soniox_enable_language_identification = Some(enabled);
    sync_active_file_model_config(&mut settings);
    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_file_deepgram_diarization_setting(
    app: AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.file_deepgram_diarize = Some(enabled);
    sync_active_file_model_config(&mut settings);
    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_file_deepgram_multichannel_setting(
    app: AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.file_deepgram_multichannel = Some(enabled);
    sync_active_file_model_config(&mut settings);
    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_file_transcription_chunking_mode_setting(
    app: AppHandle,
    mode: FileTranscriptionChunkingMode,
) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.file_transcription_chunking_mode = mode;
    sync_active_file_model_config(&mut settings);
    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_file_transcription_chunking_max_minutes_setting(
    app: AppHandle,
    minutes: f32,
) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.file_transcription_chunking_max_minutes = minutes.clamp(0.25, 10.0);
    sync_active_file_model_config(&mut settings);
    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn reapply_transcription_speaker_names(
    artifact_path: String,
    speaker_names: Vec<FileTranscriptionSpeakerNameInput>,
) -> Result<String, String> {
    reapply_diarized_transcript(&artifact_path, &speaker_names)
}

#[tauri::command]
#[specta::specta]
pub fn get_file_transcription_recording_state(
    app: AppHandle,
    model_override: Option<String>,
) -> FileTranscriptionRecordingState {
    let mut settings = get_settings(&app);
    let _ = apply_file_transcription_selection(&mut settings);
    let file_transcription_uses_local_model =
        file_transcription_uses_local_model(&settings, model_override.as_deref());
    let recording_uses_local_model = active_recording_uses_local_model(&app);
    let is_recording = app
        .state::<Arc<crate::managers::audio::AudioRecordingManager>>()
        .is_recording();

    FileTranscriptionRecordingState {
        is_recording,
        recording_uses_local_model,
        file_transcription_uses_local_model,
        blocks_file_transcription: file_transcription_uses_local_model
            && active_session_blocks_local_file_transcription(&app),
    }
}

/// Supported audio file extensions
pub(super) const SUPPORTED_EXTENSIONS: &[&str] = &["wav", "mp3", "m4a", "ogg", "flac", "webm"];
const SONIOX_LATEST_ASYNC_MODEL: &str = "stt-async-v5";
const FILE_TRANSCRIPTION_CANCELLED_MESSAGE: &str = "File transcription was cancelled";
const FILE_TRANSCRIPTION_MODEL_LOAD_POLL_INTERVAL: Duration = Duration::from_millis(50);

fn file_transcription_uses_local_model(
    settings: &AppSettings,
    model_override: Option<&str>,
) -> bool {
    model_override.is_some() || settings.transcription_provider == TranscriptionProvider::Local
}

fn active_recording_uses_local_model(app: &AppHandle) -> bool {
    let state = app.state::<ManagedSessionState>();
    let Ok(state_guard) = state.lock() else {
        return false;
    };

    match &*state_guard {
        crate::session_manager::SessionState::Recording {
            binding_id,
            captured_settings,
            ..
        } => {
            let provider = if binding_id == LIVE_SOUND_TRANSCRIPTION_BINDING_ID {
                resolve_live_sound_provider(captured_settings)
            } else {
                captured_settings.transcription_provider
            };
            provider == TranscriptionProvider::Local
        }
        _ => false,
    }
}

fn session_blocks_local_file_transcription(state: &SessionState) -> bool {
    match state {
        SessionState::Recording {
            binding_id,
            captured_settings,
            ..
        } => {
            let provider = if binding_id == LIVE_SOUND_TRANSCRIPTION_BINDING_ID {
                resolve_live_sound_provider(captured_settings)
            } else {
                captured_settings.transcription_provider
            };
            provider == TranscriptionProvider::Local
        }
        // Processing does not retain the captured provider. Conservatively
        // protect the single local engine until the operation returns to Idle.
        SessionState::Processing { .. } => true,
        SessionState::Idle => false,
    }
}

fn active_session_blocks_local_file_transcription(app: &AppHandle) -> bool {
    let state = app.state::<ManagedSessionState>();
    let Ok(state_guard) = state.lock() else {
        return true;
    };
    session_blocks_local_file_transcription(&state_guard)
}

fn ensure_file_transcription_not_cancelled(app: &AppHandle) -> Result<(), String> {
    let tm = app.state::<Arc<TranscriptionManager>>();
    if tm.is_file_transcription_cancel_requested() {
        return Err(FILE_TRANSCRIPTION_CANCELLED_MESSAGE.to_string());
    }
    Ok(())
}

/// Transcribe an audio file to text
///
/// # Arguments
/// * `file_path` - Path to the audio file
/// * `profile_id` - Retained for command compatibility; file settings use their saved snapshot
/// * `save_to_file` - If true, saves the transcription to a file in Documents folder
/// * `output_format` - Output format: "text" (default), "srt", or "vtt"
/// * `custom_words_enabled_override` - Optional override for applying custom words
/// * `soniox_options_override` - Optional Soniox async options for language hints and recognition flags
///
/// # Returns
/// FileTranscriptionResult with the transcribed text and optional saved file path
#[tauri::command]
#[specta::specta]
pub async fn transcribe_audio_file(
    app: AppHandle,
    file_path: String,
    _profile_id: Option<String>,
    save_to_file: bool,
    output_format: Option<OutputFormat>,
    mut model_override: Option<String>,
    custom_words_enabled_override: Option<bool>,
    soniox_options_override: Option<SonioxFileTranscriptionOptions>,
    deepgram_options_override: Option<DeepgramFileTranscriptionOptions>,
) -> Result<FileTranscriptionResult, String> {
    let path = PathBuf::from(&file_path);
    let format = output_format.unwrap_or_default();

    // Validate file exists
    if !path.exists() {
        return Err(format!("File not found: {}", file_path));
    }

    // Validate extension
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    if !SUPPORTED_EXTENSIONS.contains(&extension.as_str()) {
        return Err(format!(
            "Unsupported audio format: .{}. Supported formats: {}",
            extension,
            SUPPORTED_EXTENSIONS.join(", ")
        ));
    }

    info!(
        "Transcribing audio file: {} (format: {:?})",
        file_path, format
    );
    let transcription_started_at = Instant::now();

    // File settings are independent after their initial profile snapshot.
    let mut settings = get_settings(&app);
    apply_file_transcription_selection(&mut settings)?;
    if model_override.is_none() && settings.transcription_provider == TranscriptionProvider::Local {
        model_override = Some(settings.selected_model.clone());
    }
    let file_profile_snapshot = settings
        .file_transcription_model_selection
        .as_ref()
        .and_then(|selection| {
            settings
                .file_transcription_model_configs
                .get(&stt_model_selection_key(selection))
        })
        .and_then(|config| config.profile_snapshot.clone());
    let profile = file_profile_snapshot.as_ref();
    let apply_custom_words_enabled =
        custom_words_enabled_override.unwrap_or(settings.custom_words_enabled);
    let should_apply_custom_words = apply_custom_words_enabled && !settings.custom_words.is_empty();
    let mut info_message: Option<String> = None;
    let mut speaker_session: Option<FileTranscriptionSpeakerSession> = None;

    // Perform transcription - get segments for subtitle formats
    let needs_segments = matches!(format, OutputFormat::Srt | OutputFormat::Vtt);

    // If model_override is provided, we must use the local manager path with that model.
    // Otherwise, check if we should use remote.
    let use_remote = model_override.is_none()
        && settings.transcription_provider == TranscriptionProvider::RemoteOpenAiCompatible;
    let use_soniox = model_override.is_none()
        && settings.transcription_provider == TranscriptionProvider::RemoteSoniox;
    let use_deepgram = model_override.is_none()
        && settings.transcription_provider == TranscriptionProvider::RemoteDeepgram;
    let use_local = file_transcription_uses_local_model(&settings, model_override.as_deref());
    let is_gemini = use_remote
        && matches!(
            settings.remote_stt.provider_preset.as_str(),
            crate::url_security::REMOTE_STT_PRESET_GOOGLE
                | crate::url_security::REMOTE_STT_PRESET_VERCEL
        );
    if use_remote
        && needs_segments
        && !crate::managers::remote_stt::supports_subtitle_timestamps(&settings.remote_stt.model_id)
    {
        return Err(format!(
            "Model '{}' does not provide segment timestamps. Select Text output or a timestamp-capable Whisper model.",
            settings.remote_stt.model_id
        ));
    }
    if use_local && active_session_blocks_local_file_transcription(&app) {
        return Err(
            "Local file transcription is unavailable while another transcription is recording or processing."
                .to_string(),
        );
    }
    let gemini_config = if is_gemini {
        let os_locale = crate::input_source::get_language_from_input_source();
        let config = crate::gemini_config::resolve_effective_config(
            &settings,
            profile,
            crate::gemini_config::GeminiWorkflow::File {
                word_timestamps: needs_segments,
            },
            os_locale.as_deref(),
        )?;
        let source_duration = probe_audio_duration(&path)?;
        crate::gemini_config::validate_duration(source_duration, &config)?;
        if config.route == crate::gemini_config::GeminiRoute::GoogleDirect {
            crate::managers::remote_stt::validate_google_gemini_inline_request_duration(
                source_duration,
                &config,
            )?;
        }
        Some(config)
    } else {
        None
    };
    // Reserve the remote operation before decoding so Cancel can also stop a
    // file that has not reached network I/O yet.
    let remote_operation_id = use_remote.then(|| {
        app.state::<Arc<RemoteSttManager>>()
            .start_operation()
    });
    let _local_transcription_guard = if use_local {
        Some(
            app.state::<Arc<TranscriptionManager>>()
                .begin_file_transcription_operation(),
        )
    } else {
        None
    };
    let samples = if use_deepgram {
        Vec::new()
    } else {
        if use_local {
            ensure_file_transcription_not_cancelled(&app)?;
        }
        let samples = decode_audio_file(&path).map_err(|e| {
            error!("Failed to decode audio file: {}", e);
            format!("Failed to decode audio file: {}", e)
        })?;
        if use_local {
            ensure_file_transcription_not_cancelled(&app)?;
        }

        if samples.is_empty() {
            return Err("Audio file contains no audio data".to_string());
        }

        debug!("Decoded {} samples from audio file", samples.len());
        samples
    };
    let deepgram_audio_bytes = if use_deepgram {
        let bytes = std::fs::read(&path).map_err(|e| {
            error!("Failed to read audio file for Deepgram: {}", e);
            format!("Failed to read audio file: {}", e)
        })?;
        Some(bytes)
    } else {
        None
    };

    let mut local_execution_meta = None;
    let (transcription_text, segments) = if use_remote {
        // Remote STT; timestamp-capable models can also return subtitle segments.
        let remote_manager = app.state::<Arc<RemoteSttManager>>();
        let operation_id = remote_operation_id.expect("remote operation ID must be reserved");

        let translate_to_english =
            crate::managers::remote_stt::resolve_effective_translate_to_english(
                &settings,
                profile,
            );

        // Determine language: use profile setting if available, otherwise global setting
        let language = profile
            .as_ref()
            .map(|p| p.language.clone())
            .unwrap_or_else(|| settings.selected_language.clone());

        let prompt = crate::settings::resolve_stt_prompt(
            profile,
            &settings.transcription_prompts,
            &settings.remote_stt.model_id,
        );

        if let Some(config) = gemini_config.as_ref() {
            crate::gemini_config::validate_duration(samples.len() as f64 / 16_000.0, config)?;
        }

        let transcript = remote_manager
            .transcribe_file_with_operation(
                operation_id,
                &settings.remote_stt,
                &samples,
                prompt,
                Some(language.clone()),
                translate_to_english,
                needs_segments,
                gemini_config.clone(),
            )
            .await
            .map_err(|e| format!("Remote transcription failed: {}", e))?;

        if is_gemini && needs_segments {
            require_complete_gemini_timestamps(&transcript)?;
        }
        if gemini_config
            .as_ref()
            .is_some_and(|config| config.diarization)
        {
            require_complete_gemini_diarization(&transcript)?;
        }

        let output_language = OutputLanguageEvidence::from_requested_language(
            Some(language.as_str()),
            translate_to_english,
        );
        let output_language =
            resolved_output_language_for_text(&settings, &transcript.text, output_language);
        let (corrected, diarized_segments, new_speaker_session) = if gemini_config
            .as_ref()
            .is_some_and(|config| config.diarization)
        {
            if matches!(format, OutputFormat::Text) {
                if let Some((rendered_text, session)) = build_diarized_text_output(
                    DiarizedTranscriptProvider::Gemini,
                    transcript.speaker_blocks.clone(),
                    &format,
                    save_to_file,
                    &settings,
                    should_apply_custom_words,
                    &output_language,
                )? {
                    (rendered_text, None, session)
                } else {
                    (
                        apply_transcription_post_processing(
                            transcript.text,
                            &settings,
                            should_apply_custom_words,
                            &output_language,
                        ),
                        None,
                        None,
                    )
                }
            } else {
                let raw_words = transcript
                    .annotated_words
                    .iter()
                    .map(|word| RawDiarizedTranscriptWord {
                        speaker_key: word.speaker.clone(),
                        default_name: word.speaker.clone(),
                        text: word.text.clone(),
                        start: word.start,
                        end: word.end,
                    })
                    .collect();
                if let Some((rendered_text, subtitle_segments, session)) =
                    build_gemini_diarized_output(
                        raw_words,
                        &format,
                        save_to_file,
                        &settings,
                        should_apply_custom_words,
                        &output_language,
                    )?
                {
                    (rendered_text, subtitle_segments, session)
                } else {
                    (
                        apply_transcription_post_processing(
                            transcript.text,
                            &settings,
                            should_apply_custom_words,
                            &output_language,
                        ),
                        None,
                        None,
                    )
                }
            }
        } else {
            (
                apply_transcription_post_processing(
                    transcript.text,
                    &settings,
                    should_apply_custom_words,
                    &output_language,
                ),
                None,
                None,
            )
        };
        speaker_session = new_speaker_session;

        let segs = if needs_segments {
            if let Some(diarized_segments) = diarized_segments {
                Some(diarized_segments)
            } else {
                Some(post_process_remote_segments(
                    require_remote_segments(
                        transcript.segments,
                        &corrected,
                        "the selected remote model",
                    )?,
                    &settings,
                    should_apply_custom_words,
                    &output_language,
                ))
            }
        } else {
            None
        };

        (corrected, segs)
    } else if use_soniox {
        // Soniox remote STT - currently doesn't support segments
        let soniox_manager = app.state::<Arc<SonioxSttManager>>();
        let operation_id = soniox_manager.start_operation();
        let selected_soniox_model = settings.soniox_model.trim();
        let selected_model_for_message = if selected_soniox_model.is_empty() {
            "(empty)"
        } else {
            selected_soniox_model
        };

        if selected_soniox_model != SONIOX_LATEST_ASYNC_MODEL {
            info_message = Some(format!(
                "Soniox API detected. We are auto switching for the following model: {}. Selected model was '{}'. Reason: Transcribe File uses Soniox async endpoint (/v1/transcriptions), and latest-only mode enforces the latest async model.",
                SONIOX_LATEST_ASYNC_MODEL, selected_model_for_message
            ));
        }

        // Determine language: use profile setting if available, otherwise global setting
        let language = profile
            .as_ref()
            .map(|p| p.language.clone())
            .unwrap_or_else(|| settings.selected_language.clone());

        let soniox_options_override = soniox_options_override.unwrap_or_default();
        let language_hints =
            normalize_soniox_language_hints(soniox_options_override.language_hints.clone())
                .or_else(|| {
                    normalize_soniox_language_hints(Some(
                        settings
                            .file_soniox_language_hints
                            .clone()
                            .unwrap_or_else(|| settings.soniox_language_hints.clone()),
                    ))
                });
        let enable_speaker_diarization = soniox_options_override
            .enable_speaker_diarization
            .unwrap_or_else(|| {
                settings
                    .file_soniox_enable_speaker_diarization
                    .unwrap_or(settings.soniox_enable_speaker_diarization)
            });
        let enable_language_identification = soniox_options_override
            .enable_language_identification
            .unwrap_or_else(|| {
                settings
                    .file_soniox_enable_language_identification
                    .unwrap_or(settings.soniox_enable_language_identification)
            });
        let soniox_options = SonioxAsyncTranscriptionOptions {
            language_hints,
            context: crate::settings::resolve_soniox_context(profile, &settings),
            enable_speaker_diarization: Some(enable_speaker_diarization),
            enable_language_identification: Some(enable_language_identification),
        };

        #[cfg(target_os = "windows")]
        let api_key = crate::secure_keys::get_soniox_api_key();

        #[cfg(not(target_os = "windows"))]
        let api_key = String::new();

        let transcript = soniox_manager
            .transcribe_file_async(
                Some(operation_id),
                &api_key,
                SONIOX_LATEST_ASYNC_MODEL,
                settings.soniox_timeout_seconds,
                &samples,
                Some(language.as_str()),
                soniox_options,
            )
            .await
            .map_err(|e| format!("Soniox transcription failed: {}", e))?;

        if soniox_manager.is_cancelled(operation_id) {
            return Err("Soniox transcription was cancelled".to_string());
        }

        let timed_segments = if needs_segments {
            timed_tokens_to_subtitle_segments(&transcript.timed_tokens)
        } else {
            Vec::new()
        };
        // Soniox language values are hints and can still produce multilingual
        // output, so resolve the actual text instead of trusting one hint.
        let output_language = OutputLanguageEvidence::Multilingual;
        let output_language =
            resolved_output_language_for_text(&settings, &transcript.text, output_language);

        let (corrected, new_speaker_session) = if let Some((rendered_text, session)) =
            build_diarized_text_output(
                DiarizedTranscriptProvider::Soniox,
                transcript.speaker_blocks,
                &format,
                save_to_file,
                &settings,
                should_apply_custom_words,
                &output_language,
            )? {
            (rendered_text, session)
        } else {
            (
                apply_transcription_post_processing(
                    transcript.text,
                    &settings,
                    should_apply_custom_words,
                    &output_language,
                ),
                None,
            )
        };
        speaker_session = new_speaker_session;

        let segs = if needs_segments {
            Some(post_process_remote_segments(
                require_remote_segments(timed_segments, &corrected, "Soniox")?,
                &settings,
                should_apply_custom_words,
                &output_language,
            ))
        } else {
            None
        };

        (corrected, segs)
    } else if use_deepgram {
        let deepgram_manager = app.state::<Arc<DeepgramSttManager>>();
        let operation_id = deepgram_manager.start_operation();

        let language = profile
            .as_ref()
            .map(|p| p.language.clone())
            .unwrap_or_else(|| settings.selected_language.clone());

        #[cfg(target_os = "windows")]
        let api_key = crate::secure_keys::get_deepgram_api_key();

        #[cfg(not(target_os = "windows"))]
        let api_key = String::new();

        let deepgram_options = DeepgramTranscriptionOptions {
            interim_results: Some(settings.deepgram_interim_results),
            smart_format: Some(settings.deepgram_smart_format),
            diarize: Some(
                deepgram_options_override
                    .as_ref()
                    .and_then(|options| options.diarize)
                    .unwrap_or_else(|| {
                        settings
                            .file_deepgram_diarize
                            .unwrap_or(settings.deepgram_diarize)
                    }),
            ),
            multichannel: Some(
                deepgram_options_override
                    .as_ref()
                    .and_then(|options| options.multichannel)
                    .unwrap_or(settings.file_deepgram_multichannel.unwrap_or(false)),
            ),
        };
        let audio_bytes = deepgram_audio_bytes
            .as_deref()
            .ok_or_else(|| "Deepgram audio payload is missing".to_string())?;

        let transcript = deepgram_manager
            .transcribe_prerecorded_bytes(
                Some(operation_id),
                &api_key,
                &settings.deepgram_model,
                settings.deepgram_timeout_seconds,
                audio_bytes,
                Some(language.as_str()),
                deepgram_options,
            )
            .await
            .map_err(|e| format!("Deepgram transcription failed: {}", e))?;

        if deepgram_manager.is_cancelled(operation_id) {
            return Err("Deepgram transcription was cancelled".to_string());
        }

        let timed_segments = if needs_segments {
            timed_tokens_to_subtitle_segments(&transcript.timed_tokens)
        } else {
            Vec::new()
        };
        let output_language =
            OutputLanguageEvidence::from_requested_language(Some(language.as_str()), false);
        let output_language =
            resolved_output_language_for_text(&settings, &transcript.text, output_language);

        let (corrected, new_speaker_session) = if let Some((rendered_text, session)) =
            build_diarized_text_output(
                DiarizedTranscriptProvider::Deepgram,
                transcript.speaker_blocks,
                &format,
                save_to_file,
                &settings,
                should_apply_custom_words,
                &output_language,
            )? {
            (rendered_text, session)
        } else {
            (
                apply_transcription_post_processing(
                    transcript.text,
                    &settings,
                    should_apply_custom_words,
                    &output_language,
                ),
                None,
            )
        };
        speaker_session = new_speaker_session;

        let segs = if needs_segments {
            Some(post_process_remote_segments(
                require_remote_segments(timed_segments, &corrected, "Deepgram")?,
                &settings,
                should_apply_custom_words,
                &output_language,
            ))
        } else {
            None
        };

        (corrected, segs)
    } else {
        // Local transcription with segment support
        let tm = app.state::<Arc<TranscriptionManager>>();
        let loaded_model_before_override = tm.get_current_model();
        let override_changed_loaded_model = model_override
            .as_ref()
            .is_some_and(|model_id| loaded_model_before_override.as_deref() != Some(model_id));

        let restore_loaded_model = || -> Result<(), String> {
            if !override_changed_loaded_model {
                return Ok(());
            }

            match loaded_model_before_override.as_deref() {
                Some(previous_model_id) => tm.load_model(previous_model_id).map_err(|e| {
                    format!(
                        "Failed to restore previously loaded model '{}': {}",
                        previous_model_id, e
                    )
                }),
                None => tm
                    .unload_model()
                    .map_err(|e| format!("Failed to unload temporary override model: {}", e)),
            }
        };

        // If override is provided, load that model first
        if let Some(model_id) = &model_override {
            info!("Using override model: {}", model_id);
            // We need to ensure this model is loaded.
            // Note: The TM currently holds one loaded model. Switching it here might affect global state,
            // but file transcription is a distinct action.
            // However, load_model is async-ish in the background or blocking?
            // `load_model` in TM is synchronous (blocking) but `initiate_model_load` is async.
            // We need it loaded NOW.

            // First check if it's already the current one
            if override_changed_loaded_model {
                let (load_result_rx, load_decision_tx) = tm
                    .initiate_file_transcription_override_model_load(
                        model_id.clone(),
                        loaded_model_before_override.clone(),
                    );
                let load_result = loop {
                    if tm.is_file_transcription_cancel_requested() {
                        let _ =
                            load_decision_tx.send(FileTranscriptionOverrideLoadDecision::Restore);
                        return Err(FILE_TRANSCRIPTION_CANCELLED_MESSAGE.to_string());
                    }

                    match load_result_rx.recv_timeout(FILE_TRANSCRIPTION_MODEL_LOAD_POLL_INTERVAL) {
                        Ok(result) => break result,
                        Err(mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(mpsc::RecvTimeoutError::Disconnected) => {
                            break Err("Override model loader stopped before reporting a result"
                                .to_string());
                        }
                    }
                };

                load_result?;
                if tm.is_file_transcription_cancel_requested() {
                    let _ = load_decision_tx.send(FileTranscriptionOverrideLoadDecision::Restore);
                    return Err(FILE_TRANSCRIPTION_CANCELLED_MESSAGE.to_string());
                }
                load_decision_tx
                    .send(FileTranscriptionOverrideLoadDecision::Keep)
                    .map_err(|_| {
                        "Override model loader stopped before accepting the loaded model"
                            .to_string()
                    })?;
            }
        } else {
            // Ensure default model is loaded before transcription
            tm.initiate_model_load();
        }

        let transcription_model_id = model_override
            .as_deref()
            .unwrap_or(&settings.selected_model);
        let result = if needs_segments {
            if let Some(p) = &profile {
                tm.transcribe_file_with_segments(
                    samples,
                    Some(&p.language),
                    Some(p.translate_to_english),
                    crate::settings::resolve_stt_prompt(
                        Some(p),
                        &settings.transcription_prompts,
                        transcription_model_id,
                    ),
                    apply_custom_words_enabled,
                )
                .map_err(|e| format!("Local transcription failed: {}", e))
            } else {
                tm.transcribe_file_with_segments(
                    samples,
                    None,
                    None,
                    None,
                    apply_custom_words_enabled,
                )
                .map_err(|e| format!("Local transcription failed: {}", e))
            }
        } else {
            let text_result = if let Some(p) = &profile {
                tm.transcribe_file_text(
                    samples,
                    Some(&p.language),
                    Some(p.translate_to_english),
                    crate::settings::resolve_stt_prompt(
                        Some(p),
                        &settings.transcription_prompts,
                        transcription_model_id,
                    ),
                    apply_custom_words_enabled,
                )
                .map_err(|e| format!("Local transcription failed: {}", e))
            } else {
                tm.transcribe_file_text(samples, None, None, None, apply_custom_words_enabled)
                    .map_err(|e| format!("Local transcription failed: {}", e))
            };
            text_result.map(|(text, meta)| (text, None, meta))
        };

        restore_loaded_model()?;

        let (text, segs, meta) = result?;
        local_execution_meta = Some(meta);
        (text, segs)
    };

    if let Some(meta) = local_execution_meta
        .as_ref()
        .filter(|meta| meta.used_vad_chunking)
    {
        append_info_message(
            &mut info_message,
            format!(
                "Smart chunking used for local file transcription: {} chunks (max {:.2} min per chunk).",
                meta.chunk_count,
                settings.file_transcription_chunking_max_minutes.max(0.25)
            ),
        );
    }

    // Format the output based on requested format
    let output_text = match format {
        OutputFormat::Text => {
            apply_output_whitespace_policy_for_settings(&transcription_text, &settings)
        }
        OutputFormat::Srt => {
            let segs = segments.as_ref().ok_or_else(|| {
                "Transcription completed without timestamps required for SRT output.".to_string()
            })?;
            if segs.is_empty() {
                return Err(
                    "Transcription completed without valid timestamps required for SRT output. No empty subtitle file was created."
                        .to_string(),
                );
            }
            segments_to_srt(segs)
        }
        OutputFormat::Vtt => {
            let segs = segments.as_ref().ok_or_else(|| {
                "Transcription completed without timestamps required for VTT output.".to_string()
            })?;
            if segs.is_empty() {
                return Err(
                    "Transcription completed without valid timestamps required for VTT output. No empty subtitle file was created."
                        .to_string(),
                );
            }
            segments_to_vtt(segs)
        }
    };

    info!(
        "Transcription completed: {} characters (format: {:?}) in {}",
        output_text.len(),
        format,
        format_elapsed(transcription_started_at.elapsed())
    );

    append_info_message(
        &mut info_message,
        format!(
            "Benchmark: file transcription completed in {}.",
            format_elapsed(transcription_started_at.elapsed())
        ),
    );

    // Save to file if requested
    let saved_file_path = if save_to_file {
        let preferred_output_path = get_output_file_path(&path, format)?;
        let output_path =
            save_transcription_without_overwrite(&preferred_output_path, output_text.as_bytes())?;
        info!("Saved transcription to: {}", output_path.display());
        Some(output_path.to_string_lossy().to_string())
    } else {
        None
    };
    let chunking_trace = local_execution_meta
        .as_ref()
        .and_then(|meta| (!meta.chunking_trace.is_empty()).then(|| meta.chunking_trace.clone()));

    Ok(FileTranscriptionResult {
        text: output_text,
        saved_file_path,
        segments,
        info_message,
        chunking_trace,
        speaker_session,
    })
}

fn append_info_message(info_message: &mut Option<String>, next_message: String) {
    match info_message {
        Some(existing) if !existing.is_empty() => {
            existing.push_str("\n");
            existing.push_str(&next_message);
        }
        _ => {
            *info_message = Some(next_message);
        }
    }
}

fn format_elapsed(elapsed: std::time::Duration) -> String {
    let total_ms = elapsed.as_millis();
    if total_ms < 1_000 {
        return format!("{} ms", total_ms);
    }

    let seconds = elapsed.as_secs_f64();
    format!("{seconds:.2} s")
}

fn apply_transcription_post_processing(
    text: String,
    settings: &AppSettings,
    should_apply_custom_words: bool,
    output_language: &OutputLanguageEvidence,
) -> String {
    let corrected = if should_apply_custom_words {
        apply_custom_words(
            &text,
            &settings.custom_words,
            settings.word_correction_threshold,
            settings.custom_words_ngram_enabled,
        )
    } else {
        text
    };

    clean_transcription_output(
        &corrected,
        output_language,
        &[],
        &settings.custom_filler_words,
        settings.filler_word_filter_enabled,
    )
}

fn resolved_output_language_for_text(
    settings: &AppSettings,
    text: &str,
    evidence: OutputLanguageEvidence,
) -> OutputLanguageEvidence {
    if evidence == OutputLanguageEvidence::Unknown
        && settings.filler_word_filter_enabled
        && settings.custom_filler_words.is_none()
    {
        if let Some(language) = detect_output_language(text, &[]) {
            return OutputLanguageEvidence::TextDetected(language);
        }
    }
    evidence
}

fn apply_transcription_post_processing_to_blocks(
    blocks: Vec<DiarizedTranscriptBlock>,
    settings: &AppSettings,
    should_apply_custom_words: bool,
    output_language: &OutputLanguageEvidence,
) -> Vec<DiarizedTranscriptBlock> {
    let mut processed_blocks: Vec<DiarizedTranscriptBlock> = Vec::new();

    for block in blocks {
        let text = apply_transcription_post_processing(
            block.text,
            settings,
            should_apply_custom_words,
            output_language,
        );
        let trimmed = text.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some(last_block) = processed_blocks.last_mut() {
            if last_block.speaker_id == block.speaker_id {
                if !last_block.text.is_empty() {
                    last_block.text.push(' ');
                }
                last_block.text.push_str(trimmed);
                continue;
            }
        }

        processed_blocks.push(DiarizedTranscriptBlock {
            speaker_id: block.speaker_id,
            default_name: block.default_name,
            text: trimmed.to_string(),
        });
    }

    processed_blocks
}

fn apply_transcription_post_processing_to_diarized_segments(
    segments: Vec<DiarizedSubtitleSegment>,
    settings: &AppSettings,
    should_apply_custom_words: bool,
    output_language: &OutputLanguageEvidence,
) -> Vec<DiarizedSubtitleSegment> {
    segments
        .into_iter()
        .filter_map(|mut segment| {
            segment.text = apply_transcription_post_processing(
                segment.text,
                settings,
                should_apply_custom_words,
                output_language,
            );
            (!segment.text.trim().is_empty()).then_some(segment)
        })
        .collect()
}

fn build_gemini_diarized_output(
    raw_words: Vec<RawDiarizedTranscriptWord>,
    format: &OutputFormat,
    save_to_file: bool,
    settings: &AppSettings,
    should_apply_custom_words: bool,
    output_language: &OutputLanguageEvidence,
) -> Result<
    Option<(
        String,
        Option<Vec<SubtitleSegment>>,
        Option<FileTranscriptionSpeakerSession>,
    )>,
    String,
> {
    let (normalized_blocks, subtitle_segments) = normalize_raw_diarized_words(raw_words);
    if normalized_blocks.is_empty() {
        return Ok(None);
    }

    let combined_text = normalized_blocks
        .iter()
        .map(|block| block.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let output_language =
        resolved_output_language_for_text(settings, &combined_text, output_language.clone());
    let processed_blocks = apply_transcription_post_processing_to_blocks(
        normalized_blocks,
        settings,
        should_apply_custom_words,
        &output_language,
    );
    let processed_subtitle_segments = apply_transcription_post_processing_to_diarized_segments(
        subtitle_segments,
        settings,
        should_apply_custom_words,
        &output_language,
    );
    if processed_blocks.is_empty() {
        return Ok(None);
    }

    let labelled_segments = matches!(format, OutputFormat::Srt | OutputFormat::Vtt)
        .then(|| render_diarized_subtitle_segments(&processed_subtitle_segments, &[]));
    let rendered_text = match format {
        OutputFormat::Text => render_diarized_transcript(&processed_blocks, &[]),
        OutputFormat::Srt => segments_to_srt(labelled_segments.as_deref().unwrap_or_default()),
        OutputFormat::Vtt => segments_to_vtt(labelled_segments.as_deref().unwrap_or_default()),
    };
    let session = if save_to_file {
        None
    } else {
        create_diarized_transcript_session(
            DiarizedTranscriptProvider::Gemini,
            processed_blocks,
            *format,
            processed_subtitle_segments,
        )?
        .map(|(session, _)| session)
    };

    Ok(Some((rendered_text, labelled_segments, session)))
}

fn build_diarized_text_output(
    provider: DiarizedTranscriptProvider,
    raw_blocks: Vec<RawSpeakerBlock>,
    format: &OutputFormat,
    save_to_file: bool,
    settings: &AppSettings,
    should_apply_custom_words: bool,
    output_language: &OutputLanguageEvidence,
) -> Result<Option<(String, Option<FileTranscriptionSpeakerSession>)>, String> {
    if !matches!(format, OutputFormat::Text) {
        return Ok(None);
    }

    let normalized_blocks = normalize_raw_speaker_blocks(raw_blocks);
    if normalized_blocks.is_empty() {
        return Ok(None);
    }

    let combined_text = normalized_blocks
        .iter()
        .map(|block| block.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let output_language =
        resolved_output_language_for_text(settings, &combined_text, output_language.clone());

    let processed_blocks = apply_transcription_post_processing_to_blocks(
        normalized_blocks,
        settings,
        should_apply_custom_words,
        &output_language,
    );
    if processed_blocks.is_empty() {
        return Ok(None);
    }

    let rendered_text = render_diarized_transcript(&processed_blocks, &[]);
    let session = if save_to_file {
        None
    } else {
        create_diarized_transcript_session(
            provider,
            processed_blocks,
            OutputFormat::Text,
            Vec::new(),
        )?
        .map(|(session, _)| session)
    };

    Ok(Some((rendered_text, session)))
}

fn require_remote_segments(
    segments: Vec<SubtitleSegment>,
    transcript_text: &str,
    provider: &str,
) -> Result<Vec<SubtitleSegment>, String> {
    if !transcript_text.trim().is_empty() && segments.is_empty() {
        return Err(format!(
            "{provider} returned transcript text without timestamps. Select Text output instead of SRT/VTT."
        ));
    }
    Ok(segments)
}

fn normalized_coverage_text(text: &str) -> String {
    let normalized = text
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    if normalized.is_empty() {
        text.split_whitespace().collect::<String>()
    } else {
        normalized
    }
}

fn transcript_coverage_matches(transcript_text: &str, annotated_text: &str) -> bool {
    normalized_coverage_text(transcript_text) == normalized_coverage_text(annotated_text)
}

fn require_complete_gemini_timestamps(
    transcript: &RemoteFileTranscription,
) -> Result<(), String> {
    if transcript.text.trim().is_empty() {
        return Err(
            "Gemini returned no transcript text for subtitle output. Select Text output or try the transcription again."
                .to_string(),
        );
    }
    let timed_text = transcript
        .segments
        .iter()
        .map(|segment| segment.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    if transcript.segments.is_empty()
        || !transcript_coverage_matches(&transcript.text, &timed_text)
    {
        return Err(
            "Gemini returned incomplete or invalid word timestamps. No SRT/VTT file was created; select Text output or try the transcription again."
                .to_string(),
        );
    }
    Ok(())
}

fn require_complete_gemini_diarization(
    transcript: &RemoteFileTranscription,
) -> Result<(), String> {
    if transcript.text.trim().is_empty() {
        return Err("Gemini returned no transcript text for speaker diarization.".to_string());
    }
    let diarized_text = transcript
        .speaker_blocks
        .iter()
        .map(|block| block.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    if transcript.speaker_blocks.is_empty()
        || !transcript_coverage_matches(&transcript.text, &diarized_text)
    {
        return Err(
            "Gemini returned incomplete speaker annotations. The full transcript was preserved by rejecting the partial diarized result; disable diarization or try again."
                .to_string(),
        );
    }
    Ok(())
}

fn post_process_remote_segments(
    segments: Vec<SubtitleSegment>,
    settings: &AppSettings,
    should_apply_custom_words: bool,
    output_language: &OutputLanguageEvidence,
) -> Vec<SubtitleSegment> {
    segments
        .into_iter()
        .filter_map(|mut segment| {
            if should_apply_custom_words {
                segment.text = apply_custom_words(
                    &segment.text,
                    &settings.custom_words,
                    settings.word_correction_threshold,
                    settings.custom_words_ngram_enabled,
                );
            }
            segment.text = clean_transcription_output(
                &segment.text,
                output_language,
                &[],
                &settings.custom_filler_words,
                settings.filler_word_filter_enabled,
            );
            (!segment.text.trim().is_empty()).then_some(segment)
        })
        .collect()
}

fn normalize_soniox_language_hints(hints: Option<Vec<String>>) -> Option<Vec<String>> {
    let Some(hints) = hints else {
        return None;
    };

    let mut deduped = Vec::new();
    for hint in hints {
        let normalized = hint.trim().to_lowercase().replace('_', "-");
        if normalized.is_empty() || normalized == "auto" || normalized == "os_input" {
            continue;
        }
        let normalized = if normalized == "zh-hans" || normalized == "zh-hant" {
            "zh".to_string()
        } else {
            normalized
                .split('-')
                .next()
                .unwrap_or_default()
                .trim()
                .to_string()
        };
        if normalized.is_empty() || deduped.iter().any(|value| value == &normalized) {
            continue;
        }
        deduped.push(normalized);
    }

    if deduped.is_empty() {
        None
    } else {
        Some(deduped)
    }
}

/// Decode an audio file to f32 PCM samples at 16kHz
fn probe_audio_duration(path: &PathBuf) -> Result<f64, String> {
    use rodio::Source;
    use std::fs::File;
    use std::io::BufReader;

    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_lowercase())
        .unwrap_or_default();

    if extension == "wav" {
        let reader = hound::WavReader::open(path)
            .map_err(|error| format!("Failed to inspect WAV file: {error}"))?;
        let sample_rate = reader.spec().sample_rate;
        validate_audio_sample_rate(sample_rate)?;
        return Ok(reader.duration() as f64 / sample_rate as f64);
    }

    let file = File::open(path).map_err(|error| format!("Failed to open file: {error}"))?;
    let byte_len = file
        .metadata()
        .map_err(|error| format!("Failed to read file metadata: {error}"))?
        .len();
    let reader = BufReader::new(file);
    let mut decoder_builder = rodio::Decoder::builder()
        .with_data(reader)
        .with_byte_len(byte_len)
        .with_seekable(true);
    if !extension.is_empty() {
        decoder_builder = decoder_builder.with_hint(&extension);
    }
    if let Some(mime_type) = audio_mime_type_for_extension(&extension) {
        decoder_builder = decoder_builder.with_mime_type(mime_type);
    }
    let source = decoder_builder
        .build()
        .map_err(|error| format!("Failed to inspect audio file: {error}"))?;
    validate_audio_sample_rate(source.sample_rate())?;
    source.total_duration().map(|duration| duration.as_secs_f64()).ok_or_else(|| {
        "The audio duration could not be determined safely before decoding. Convert the file to WAV and try again."
            .to_string()
    })
}

fn decode_audio_file(path: &PathBuf) -> Result<Vec<f32>, String> {
    use rodio::Source;
    use std::fs::File;
    use std::io::BufReader; // Import trait for sample_rate() and channels()

    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    // For WAV files, use hound for direct reading
    if extension == "wav" {
        return decode_wav_file(path);
    }

    // For other formats, use rodio's Symphonia-backed decoder. MP4/M4A files
    // often keep the metadata atom after the media data, so the decoder needs
    // a seekable source with a known byte length.
    let file = File::open(path).map_err(|e| format!("Failed to open file: {}", e))?;
    let byte_len = file
        .metadata()
        .map_err(|e| format!("Failed to read file metadata: {}", e))?
        .len();
    let reader = BufReader::new(file);

    let mut decoder_builder = rodio::Decoder::builder()
        .with_data(reader)
        .with_byte_len(byte_len)
        .with_seekable(true);
    if !extension.is_empty() {
        decoder_builder = decoder_builder.with_hint(&extension);
    }
    if let Some(mime_type) = audio_mime_type_for_extension(&extension) {
        decoder_builder = decoder_builder.with_mime_type(mime_type);
    }

    let source = decoder_builder
        .build()
        .map_err(|e| format!("Failed to decode audio: {}", e))?;

    // Get source sample rate and channels
    let sample_rate = source.sample_rate();
    validate_audio_sample_rate(sample_rate)?;
    let channels = source.channels() as usize;

    debug!("Audio file: {} Hz, {} channels", sample_rate, channels);

    // Collect all samples as f32 (rodio decoder outputs f32)
    let samples: Vec<f32> = source.collect();

    // Convert to mono if stereo
    let mono_samples: Vec<f32> = if channels > 1 {
        samples
            .chunks(channels)
            .map(|chunk| chunk.iter().sum::<f32>() / channels as f32)
            .collect()
    } else {
        samples
    };

    // Resample to 16kHz if necessary
    let target_sample_rate = 16000;
    let resampled = if sample_rate != target_sample_rate {
        resample_audio(&mono_samples, sample_rate, target_sample_rate)?
    } else {
        mono_samples
    };

    Ok(resampled)
}

fn audio_mime_type_for_extension(extension: &str) -> Option<&'static str> {
    match extension {
        "m4a" => Some("audio/mp4"),
        "mp3" => Some("audio/mpeg"),
        "ogg" => Some("audio/ogg"),
        "flac" => Some("audio/flac"),
        "webm" => Some("audio/webm"),
        _ => None,
    }
}

/// Decode a WAV file directly using hound
fn decode_wav_file(path: &PathBuf) -> Result<Vec<f32>, String> {
    let reader =
        hound::WavReader::open(path).map_err(|e| format!("Failed to open WAV file: {}", e))?;

    let spec = reader.spec();
    let sample_rate = spec.sample_rate;
    validate_audio_sample_rate(sample_rate)?;
    let channels = spec.channels as usize;

    debug!(
        "WAV file: {} Hz, {} channels, {} bits",
        sample_rate, channels, spec.bits_per_sample
    );

    // Read samples based on format
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            // Use i64 for the shift to avoid overflow with 32-bit samples
            let max_val = (1_i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .into_samples::<i32>()
                .enumerate()
                .map(|(sample_index, sample)| {
                    sample.map(|value| value as f32 / max_val).map_err(|error| {
                        format!("Failed to decode WAV sample {sample_index}: {error}")
                    })
                })
                .collect::<Result<Vec<_>, String>>()?
        }
        hound::SampleFormat::Float => reader
            .into_samples::<f32>()
            .enumerate()
            .map(|(sample_index, sample)| {
                sample
                    .map_err(|error| format!("Failed to decode WAV sample {sample_index}: {error}"))
            })
            .collect::<Result<Vec<_>, String>>()?,
    };

    // Convert to mono if stereo
    let mono_samples: Vec<f32> = if channels > 1 {
        samples
            .chunks(channels)
            .map(|chunk| chunk.iter().sum::<f32>() / channels as f32)
            .collect()
    } else {
        samples
    };

    // Resample to 16kHz if necessary
    let target_sample_rate = 16000;
    let resampled = if sample_rate != target_sample_rate {
        resample_audio(&mono_samples, sample_rate, target_sample_rate)?
    } else {
        mono_samples
    };

    Ok(resampled)
}

const MIN_SUPPORTED_AUDIO_SAMPLE_RATE: u32 = 1_000;
const MAX_SUPPORTED_AUDIO_SAMPLE_RATE: u32 = 384_000;

fn validate_audio_sample_rate(sample_rate: u32) -> Result<(), String> {
    if (MIN_SUPPORTED_AUDIO_SAMPLE_RATE..=MAX_SUPPORTED_AUDIO_SAMPLE_RATE).contains(&sample_rate) {
        Ok(())
    } else {
        Err(format!(
            "Unsupported audio sample rate {sample_rate} Hz; expected {MIN_SUPPORTED_AUDIO_SAMPLE_RATE}–{MAX_SUPPORTED_AUDIO_SAMPLE_RATE} Hz"
        ))
    }
}

/// Resample audio from one sample rate to another
fn resample_audio(samples: &[f32], from_rate: u32, to_rate: u32) -> Result<Vec<f32>, String> {
    use rubato::{FftFixedIn, Resampler};

    // Use a reasonable chunk size
    let chunk_size = 1024.min(samples.len());
    if chunk_size == 0 {
        return Ok(Vec::new());
    }

    let mut resampler = FftFixedIn::<f32>::new(
        from_rate as usize,
        to_rate as usize,
        chunk_size,
        1, // sub_chunks
        1, // channels
    )
    .map_err(|e| format!("Failed to create resampler: {}", e))?;

    let mut output = Vec::new();

    // Process in chunks
    for chunk in samples.chunks(chunk_size) {
        // Pad last chunk if needed
        let mut input_chunk = chunk.to_vec();
        if input_chunk.len() < chunk_size {
            input_chunk.resize(chunk_size, 0.0);
        }

        let result = resampler
            .process(&[input_chunk], None)
            .map_err(|e| format!("Failed to resample audio: {}", e))?;

        if let Some(out_chunk) = result.first() {
            output.extend_from_slice(out_chunk);
        }
    }

    // FftFixedIn buffers input internally when its FFT block does not align
    // with our caller-side chunks. Push a final zero-padded block so delayed
    // audio frames are emitted instead of being dropped at end of file.
    let tail = resampler
        .process_partial::<Vec<f32>>(None, None)
        .map_err(|e| format!("Failed to flush resampled audio: {e}"))?;
    if let Some(out_chunk) = tail.first() {
        output.extend_from_slice(out_chunk);
    }

    Ok(output)
}

/// Get the output file path for saving transcription
/// Saves to Documents folder with same name as audio file but appropriate extension
fn get_output_file_path(audio_path: &PathBuf, format: OutputFormat) -> Result<PathBuf, String> {
    // Get Documents folder
    let documents_dir =
        dirs::document_dir().ok_or_else(|| "Could not find Documents folder".to_string())?;

    // Create output filename from audio filename
    let stem = audio_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("transcription");

    let ext = get_format_extension(format);
    let output_path = documents_dir.join(format!("{}.{}", stem, ext));

    Ok(output_path)
}

fn save_transcription_without_overwrite(
    preferred_path: &Path,
    contents: &[u8],
) -> Result<PathBuf, String> {
    let parent = preferred_path
        .parent()
        .ok_or_else(|| "Transcription output path has no parent directory".to_string())?;
    let stem = preferred_path
        .file_stem()
        .ok_or_else(|| "Transcription output path has no file name".to_string())?;
    let extension = preferred_path.extension();

    for index in 1..=10_000 {
        let candidate = if index == 1 {
            preferred_path.to_path_buf()
        } else {
            let mut file_name = stem.to_os_string();
            file_name.push(format!("-{index}"));
            if let Some(extension) = extension {
                file_name.push(".");
                file_name.push(extension);
            }
            parent.join(file_name)
        };

        let mut file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(format!(
                    "Failed to create transcription file {}: {error}",
                    candidate.display()
                ));
            }
        };

        if let Err(error) = file.write_all(contents).and_then(|_| file.flush()) {
            drop(file);
            let _ = std::fs::remove_file(&candidate);
            return Err(format!(
                "Failed to save transcription to {}: {error}",
                candidate.display()
            ));
        }

        return Ok(candidate);
    }

    Err(format!(
        "Could not allocate a collision-safe transcription name for {}",
        preferred_path.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        require_remote_segments, resample_audio, save_transcription_without_overwrite,
        session_blocks_local_file_transcription, validate_audio_sample_rate,
    };
    use crate::session_manager::SessionState;
    use crate::subtitle::SubtitleSegment;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn audio_sample_rate_validation_rejects_resource_exhaustion_inputs() {
        assert!(validate_audio_sample_rate(u32::MAX).is_err());
        assert!(validate_audio_sample_rate(0).is_err());
    }

    #[test]
    fn audio_sample_rate_validation_accepts_normal_audio_rates() {
        for sample_rate in [8_000, 16_000, 44_100, 48_000, 192_000, 384_000] {
            assert!(validate_audio_sample_rate(sample_rate).is_ok());
        }
    }

    #[test]
    fn resampling_flushes_the_final_buffered_frames() {
        let input = vec![0.0; 512_000];
        let output = resample_audio(&input, 44_100, 16_000).unwrap();
        let duration_preserving_minimum = input.len() * 16_000 / 44_100;

        assert!(output.len() >= duration_preserving_minimum);
    }

    #[test]
    fn saved_transcription_does_not_overwrite_an_existing_file() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "aivorelay-transcription-collision-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).unwrap();
        let preferred = directory.join("meeting.txt");
        fs::write(&preferred, "existing transcript").unwrap();

        let saved = save_transcription_without_overwrite(&preferred, b"new transcript").unwrap();

        assert_eq!(saved, directory.join("meeting-2.txt"));
        assert_eq!(
            fs::read_to_string(&preferred).unwrap(),
            "existing transcript"
        );
        assert_eq!(fs::read_to_string(saved).unwrap(), "new transcript");
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn processing_session_blocks_local_file_transcription() {
        let processing = SessionState::Processing {
            binding_id: "transcribe".to_string(),
            operation_id: 7,
        };

        assert!(session_blocks_local_file_transcription(&processing));
        assert!(!session_blocks_local_file_transcription(
            &SessionState::Idle
        ));
    }

    #[test]
    fn remote_subtitle_export_rejects_text_without_timestamps() {
        let error = require_remote_segments(Vec::new(), "Recognized speech", "Provider")
            .expect_err("non-empty text without timing must not produce synthetic subtitles");
        assert!(error.contains("without timestamps"));

        assert!(require_remote_segments(
            vec![SubtitleSegment {
                start: 0.2,
                end: 1.0,
                text: "Recognized speech".to_string(),
            }],
            "Recognized speech",
            "Provider",
        )
        .is_ok());
    }
}
