use crate::managers::tts::{FileConversionResult, TtsManager};
use crate::managers::tts_history::{
    apply_provider_synthesis_config, llm_cleanup_config_from_settings,
    provider_synthesis_config_from_settings, NewTtsHistoryEntry, TtsHistoryDeleteOutcome,
    TtsHistoryEntry, TtsHistoryManager, TtsHistoryScope, TtsHistorySourceKind,
};
use crate::settings::{
    LLMPrompt, TtsKeySource, TtsLlmScope, TtsOutputFormat, TtsProvider, TtsSettings,
    DEFAULT_TTS_MURF_GEN2_VOICE, DEFAULT_TTS_MURF_VOICE,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};

static REGENERATION_FILE_ID: AtomicU64 = AtomicU64::new(0);
const ALLOWED_MP3_BITRATES: &[u16] = &[64, 96, 128, 192, 256, 320];

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RegenerateTtsHistoryRequest {
    pub id: i64,
    /// Optional external destination. When omitted (the normal UI path), a
    /// collision-safe cache output is used only long enough to append the
    /// managed history copy and is then removed.
    pub output_path: Option<String>,
    pub provider: Option<TtsProvider>,
    pub model: Option<String>,
    pub voice: Option<String>,
    pub prompt_preset_id: Option<String>,
    pub prompt_preset_name: Option<String>,
    /// Literal resolved instructions. Callers must read any instructions file
    /// themselves; this string is never evaluated as code.
    pub instructions: Option<String>,
    /// Optional LLM text-cleanup overrides. These are separate from TTS voice
    /// instructions and never mutate saved settings.
    pub llm_preprocessing: Option<bool>,
    pub llm_prompt_id: Option<String>,
    pub llm_prompt_name: Option<String>,
    pub llm_instructions: Option<String>,
    pub llm_provider_id: Option<String>,
    pub llm_model: Option<String>,
    pub llm_key_source: Option<TtsKeySource>,
    pub llm_custom_base_url: Option<String>,
    pub llm_custom_allow_insecure_http: Option<bool>,
    pub llm_reasoning_enabled: Option<bool>,
    pub llm_reasoning_budget: Option<u32>,
    pub llm_chunk_target_chars: Option<u32>,
    pub llm_retry_count: Option<u8>,
    pub llm_retry_base_delay_ms: Option<u32>,
    pub llm_request_timeout_seconds: Option<u32>,
    pub output_format: Option<TtsOutputFormat>,
    pub mp3_bitrate_kbps: Option<u16>,
    /// Must be true only after showing the API-credit warning.
    pub confirmed_api_charge: bool,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RegenerateTtsHistoryResponse {
    pub source_entry_id: i64,
    pub new_entry: TtsHistoryEntry,
    pub output_path: Option<String>,
    pub operation_id: u64,
    pub chunk_count: usize,
    pub resumed_chunks: usize,
    pub processed_character_count: usize,
}

#[derive(Clone, Debug)]
struct ResolvedPrompt {
    preset_id: Option<String>,
    preset_name: Option<String>,
    instructions: Option<String>,
}

struct TemporarySourceFile(PathBuf);

impl Drop for TemporarySourceFile {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_file(&self.0) {
            if error.kind() != std::io::ErrorKind::NotFound {
                log::warn!(
                    "Failed to remove temporary TTS history source {}: {}",
                    self.0.display(),
                    error
                );
            }
        }
    }
}

struct TemporaryOutputFile(PathBuf);

impl Drop for TemporaryOutputFile {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_file(&self.0) {
            if error.kind() != std::io::ErrorKind::NotFound {
                log::warn!(
                    "Failed to remove temporary TTS history output {}: {}",
                    self.0.display(),
                    error
                );
            }
        }
    }
}

#[tauri::command]
#[specta::specta]
pub fn get_tts_history_entries(
    history: State<'_, Arc<TtsHistoryManager>>,
    scope: TtsHistoryScope,
) -> Result<Vec<TtsHistoryEntry>, String> {
    history
        .list_entries(scope)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn get_tts_history_entry(
    history: State<'_, Arc<TtsHistoryManager>>,
    id: i64,
) -> Result<Option<TtsHistoryEntry>, String> {
    history
        .get_entry_by_id(id)
        .map_err(|error| error.to_string())
}

/// Returns a retained path resolved exclusively from the database row.
/// Missing records return `None`; a missing retained file is an explicit error.
#[tauri::command]
#[specta::specta]
pub fn get_tts_history_audio_path(
    history: State<'_, Arc<TtsHistoryManager>>,
    id: i64,
) -> Result<Option<String>, String> {
    let Some(path) = history
        .retained_audio_path(id)
        .map_err(|error| error.to_string())?
    else {
        return Ok(None);
    };
    if !path.is_file() {
        return Err(format!(
            "Retained audio file for TTS history entry {id} is missing: {}",
            path.display()
        ));
    }
    Ok(Some(path.to_string_lossy().into_owned()))
}

#[tauri::command]
#[specta::specta]
pub fn delete_tts_history_entry(
    history: State<'_, Arc<TtsHistoryManager>>,
    id: i64,
) -> Result<bool, String> {
    history.delete_entry(id).map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn delete_tts_history_entry_detailed(
    history: State<'_, Arc<TtsHistoryManager>>,
    id: i64,
) -> Result<Option<TtsHistoryDeleteOutcome>, String> {
    history
        .delete_entry_detailed(id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn delete_all_tts_history_entries(
    history: State<'_, Arc<TtsHistoryManager>>,
    scope: TtsHistoryScope,
) -> Result<usize, String> {
    history
        .delete_all_entries(scope)
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn export_tts_history_audio(
    history: State<'_, Arc<TtsHistoryManager>>,
    id: i64,
    destination: String,
) -> Result<String, String> {
    let path = history
        .export_audio(id, destination)
        .map_err(|error| error.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}

/// Re-synthesizes retained raw text through the full file TTS pipeline and
/// appends a new history variant under the source entry's group.
///
/// This is also the shared implementation used by the headless CLI.
pub async fn regenerate_tts_history_entry_core(
    app: &AppHandle,
    history: &Arc<TtsHistoryManager>,
    tts: &Arc<TtsManager>,
    request: RegenerateTtsHistoryRequest,
) -> Result<RegenerateTtsHistoryResponse, String> {
    let source_entry = history
        .get_entry_by_id(request.id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("TTS history entry {} not found", request.id))?;
    let effective_provider = request.provider.unwrap_or(source_entry.provider);
    let saved_tts = crate::settings::get_settings(app).tts;
    let saved_llm_enabled = match source_entry.scope {
        TtsHistoryScope::Interactive => saved_tts.llm_preprocessing.interactive_enabled,
        TtsHistoryScope::File => saved_tts.llm_preprocessing.file_enabled,
    };
    let has_llm_override = request.llm_prompt_id.is_some()
        || request.llm_prompt_name.is_some()
        || request.llm_instructions.is_some()
        || request.llm_provider_id.is_some()
        || request.llm_model.is_some()
        || request.llm_key_source.is_some()
        || request.llm_custom_base_url.is_some()
        || request.llm_custom_allow_insecure_http.is_some()
        || request.llm_reasoning_enabled.is_some()
        || request.llm_reasoning_budget.is_some()
        || request.llm_chunk_target_chars.is_some()
        || request.llm_retry_count.is_some()
        || request.llm_retry_base_delay_ms.is_some()
        || request.llm_request_timeout_seconds.is_some();
    let llm_cleanup_enabled = request
        .llm_preprocessing
        .unwrap_or(saved_llm_enabled || has_llm_override);
    if (effective_provider.requires_paid_confirmation() || llm_cleanup_enabled)
        && !request.confirmed_api_charge
    {
        return Err(
            "Regeneration requires explicit confirmation because it can make a paid TTS or AI-cleanup API request"
                .to_string(),
        );
    }
    let (output_path, output_format, temporary_output) = prepare_regeneration_output(
        app,
        request.output_path.as_deref(),
        request.output_format,
        source_entry.output_format,
    )?;

    let scope = match source_entry.scope {
        TtsHistoryScope::Interactive => crate::settings::TtsOperationScope::Interactive,
        TtsHistoryScope::File => crate::settings::TtsOperationScope::File,
    };
    let mut settings = crate::settings::get_settings(app)
        .tts
        .effective_for_scope(scope);
    let provider_was_overridden = request.provider.is_some();
    settings.provider = request.provider.unwrap_or(source_entry.provider);
    if !provider_was_overridden {
        let source_provider = settings.provider;
        set_model_and_voice(
            &mut settings,
            source_provider,
            Some(&source_entry.model),
            Some(&source_entry.voice),
        )?;
        restore_source_language(&mut settings, &source_entry.language);
        if let Some(provider_controls) = source_entry.provider_synthesis_config.as_deref() {
            apply_provider_synthesis_config(&mut settings, provider_controls)
                .map_err(|error| error.to_string())?;
        }
    }
    let selected_provider = settings.provider;
    set_model_and_voice(
        &mut settings,
        selected_provider,
        request.model.as_deref(),
        request.voice.as_deref(),
    )?;
    let llm_scope = match source_entry.scope {
        TtsHistoryScope::Interactive => TtsLlmScope::Interactive,
        TtsHistoryScope::File => TtsLlmScope::File,
    };
    apply_regeneration_llm_overrides(&mut settings, llm_scope, &request)?;
    settings.output_format = output_format;
    if let Some(bitrate) = request.mp3_bitrate_kbps {
        if output_format != TtsOutputFormat::Mp3 {
            return Err("--bitrate is valid only for MP3 output".to_string());
        }
        if !ALLOWED_MP3_BITRATES.contains(&bitrate) {
            return Err(format!(
                "Unsupported MP3 bitrate {bitrate}; use {} kb/s",
                ALLOWED_MP3_BITRATES
                    .iter()
                    .map(u16::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        settings.mp3_bitrate_kbps = bitrate;
    }
    if output_format == TtsOutputFormat::Mp3
        && !ALLOWED_MP3_BITRATES.contains(&settings.mp3_bitrate_kbps)
    {
        return Err(format!(
            "Saved MP3 bitrate {} is invalid; select one of {}",
            settings.mp3_bitrate_kbps,
            ALLOWED_MP3_BITRATES
                .iter()
                .map(u16::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    let explicit_prompt_override = request.instructions.is_some()
        || request.prompt_preset_id.is_some()
        || request.prompt_preset_name.is_some();
    let prompt = enforce_prompt_model_compatibility(
        &settings,
        explicit_prompt_override,
        resolve_regeneration_prompt(&settings, &source_entry, &request)?,
    )?;
    if let Some(instructions) = prompt.instructions.as_deref() {
        TtsManager::validate_openai_instructions(instructions)
            .map_err(|error| error.to_string())?;
    }
    settings.openai_instructions = prompt.instructions.clone().unwrap_or_default();

    let temporary_source =
        write_temporary_source(app, &source_entry.source_text, source_entry.source_kind)?;
    let resume_namespace = request
        .output_path
        .is_none()
        .then(|| format!("history-regeneration-entry-{}", source_entry.id));
    let resolved = tts
        .convert_text_file_for_history_resolved(
            &temporary_source.0,
            &output_path,
            &settings,
            llm_scope,
            resume_namespace.as_deref(),
        )
        .await
        .map_err(|error| error.to_string())?;
    let conversion = resolved.value;
    settings = resolved.settings;
    let (model, voice) = current_model_and_voice(&settings);
    let language = current_language(&settings);
    let new_entry = history
        .append_confirmed_regeneration_success(
            NewTtsHistoryEntry {
                scope: source_entry.scope,
                group_id: source_entry.group_id.clone(),
                source_text: source_entry.source_text.clone(),
                source_kind: source_entry.source_kind,
                provider: settings.provider,
                model,
                voice,
                language,
                output_format,
                external_output_path: request.output_path.as_ref().map(|_| output_path.clone()),
                prompt_preset_id: prompt.preset_id,
                prompt_preset_name: prompt.preset_name,
                resolved_instructions: prompt.instructions,
                llm_cleanup_config: llm_cleanup_config_from_settings(&settings, source_entry.scope),
                provider_synthesis_config: provider_synthesis_config_from_settings(&settings),
            },
            &output_path,
        )
        .map_err(|error| {
            format!(
                "Audio was created at {}, but the new history variant could not be retained: {}",
                output_path.display(),
                error
            )
        })?;
    if let Some(resume_namespace) = resume_namespace.as_deref() {
        if let Err(error) = tts.discard_managed_resume_namespace(resume_namespace) {
            log::warn!(
                "TTS History result {} was saved, but its completed resume checkpoint could not be cleared: {}",
                new_entry.id,
                error
            );
        }
    }
    drop(temporary_output);

    Ok(regeneration_response(
        source_entry.id,
        new_entry,
        request.output_path.as_ref().map(|_| output_path.as_path()),
        &conversion,
    ))
}

#[tauri::command]
#[specta::specta]
pub async fn regenerate_tts_history_entry(
    app: AppHandle,
    history: State<'_, Arc<TtsHistoryManager>>,
    tts: State<'_, Arc<TtsManager>>,
    request: RegenerateTtsHistoryRequest,
) -> Result<RegenerateTtsHistoryResponse, String> {
    regenerate_tts_history_entry_core(&app, history.inner(), tts.inner(), request).await
}

fn regeneration_response(
    source_entry_id: i64,
    new_entry: TtsHistoryEntry,
    output_path: Option<&Path>,
    conversion: &FileConversionResult,
) -> RegenerateTtsHistoryResponse {
    RegenerateTtsHistoryResponse {
        source_entry_id,
        new_entry,
        output_path: output_path.map(|path| path.to_string_lossy().into_owned()),
        operation_id: conversion.operation_id,
        chunk_count: conversion.chunk_count,
        resumed_chunks: conversion.resumed_chunks,
        processed_character_count: conversion.processed_character_count,
    }
}

fn apply_regeneration_llm_overrides(
    settings: &mut TtsSettings,
    scope: TtsLlmScope,
    request: &RegenerateTtsHistoryRequest,
) -> Result<(), String> {
    let has_override = request.llm_prompt_id.is_some()
        || request.llm_prompt_name.is_some()
        || request.llm_instructions.is_some()
        || request.llm_provider_id.is_some()
        || request.llm_model.is_some()
        || request.llm_key_source.is_some()
        || request.llm_custom_base_url.is_some()
        || request.llm_custom_allow_insecure_http.is_some()
        || request.llm_reasoning_enabled.is_some()
        || request.llm_reasoning_budget.is_some()
        || request.llm_chunk_target_chars.is_some()
        || request.llm_retry_count.is_some()
        || request.llm_retry_base_delay_ms.is_some()
        || request.llm_request_timeout_seconds.is_some();
    if request.llm_preprocessing == Some(false) && has_override {
        return Err(
            "Disabling TTS AI cleanup conflicts with the supplied LLM cleanup overrides"
                .to_string(),
        );
    }

    let llm = &mut settings.llm_preprocessing;
    let enabled = request.llm_preprocessing.unwrap_or(has_override);
    if request.llm_preprocessing.is_some() || has_override {
        match scope {
            TtsLlmScope::Interactive => llm.interactive_enabled = enabled,
            TtsLlmScope::File => llm.file_enabled = enabled,
        }
    }
    if let Some(provider_id) = request.llm_provider_id.as_deref() {
        let provider_id = provider_id.trim();
        if provider_id.is_empty() {
            return Err("TTS AI cleanup provider ID cannot be empty".to_string());
        }
        llm.provider_id = provider_id.to_string();
    }
    if let Some(model) = request.llm_model.as_deref() {
        let model = model.trim();
        if model.is_empty() {
            return Err("TTS AI cleanup model cannot be empty".to_string());
        }
        llm.model = model.to_string();
    }
    if let Some(key_source) = request.llm_key_source {
        llm.key_source = key_source;
    }
    if let Some(base_url) = request.llm_custom_base_url.as_deref() {
        if llm.provider_id != "custom" {
            return Err(
                "A custom LLM base URL is supported only by the custom provider".to_string(),
            );
        }
        let base_url = base_url.trim();
        if base_url.is_empty() {
            return Err("Custom TTS AI cleanup base URL cannot be empty".to_string());
        }
        llm.custom_base_url = base_url.trim_end_matches('/').to_string();
    }
    if let Some(allow) = request.llm_custom_allow_insecure_http {
        if llm.provider_id != "custom" {
            return Err(
                "Insecure HTTP is supported only by the custom TTS AI cleanup provider".to_string(),
            );
        }
        llm.custom_allow_insecure_http = allow;
    }
    if let Some(reasoning) = request.llm_reasoning_enabled {
        llm.reasoning_enabled = reasoning;
    }
    if let Some(budget) = request.llm_reasoning_budget {
        if !(1_024..=1_000_000).contains(&budget) {
            return Err("TTS AI cleanup reasoning budget must be 1024–1000000".to_string());
        }
        if request.llm_reasoning_enabled == Some(false)
            || (request.llm_reasoning_enabled.is_none() && !llm.reasoning_enabled)
        {
            return Err(
                "A TTS AI cleanup reasoning budget requires reasoning to be enabled".to_string(),
            );
        }
        llm.reasoning_budget = budget;
    }
    if let Some(chars) = request.llm_chunk_target_chars {
        if !(1_000..=50_000).contains(&chars) {
            return Err("TTS AI cleanup chunk size must be 1000–50000 characters".to_string());
        }
        llm.chunk_target_chars = chars;
    }
    if let Some(retries) = request.llm_retry_count {
        if retries > 10 {
            return Err("TTS AI cleanup retries must be 0–10".to_string());
        }
        llm.retry_count = retries;
    }
    if let Some(delay) = request.llm_retry_base_delay_ms {
        if !(100..=30_000).contains(&delay) {
            return Err("TTS AI cleanup retry delay must be 100–30000 ms".to_string());
        }
        llm.retry_base_delay_ms = delay;
    }
    if let Some(timeout) = request.llm_request_timeout_seconds {
        if !(10..=600).contains(&timeout) {
            return Err("TTS AI cleanup timeout must be 10–600 seconds".to_string());
        }
        llm.request_timeout_seconds = timeout;
    }

    if request.llm_prompt_id.is_some() && request.llm_prompt_name.is_some() {
        return Err("Choose a TTS AI cleanup prompt by ID or name, not both".to_string());
    }
    let (prompts, selected_id) = match scope {
        TtsLlmScope::Interactive => (
            &mut llm.interactive_prompts,
            &mut llm.interactive_selected_prompt_id,
        ),
        TtsLlmScope::File => (&mut llm.file_prompts, &mut llm.file_selected_prompt_id),
    };
    if let Some(instructions) = request.llm_instructions.as_deref() {
        let instructions = instructions.trim();
        if instructions.is_empty() {
            return Err("TTS AI cleanup instructions cannot be empty".to_string());
        }
        if instructions.chars().count() > 32_768 {
            return Err("TTS AI cleanup instructions must not exceed 32768 characters".to_string());
        }
        let id = "history_regeneration_llm_instructions".to_string();
        prompts.push(LLMPrompt {
            id: id.clone(),
            name: "History regeneration instructions".to_string(),
            prompt: instructions.to_string(),
        });
        *selected_id = id;
    } else if let Some(id) = request.llm_prompt_id.as_deref() {
        let prompt = prompts
            .iter()
            .find(|prompt| prompt.id == id.trim())
            .ok_or_else(|| format!("Unknown TTS AI cleanup prompt ID '{}'", id.trim()))?;
        *selected_id = prompt.id.clone();
    } else if let Some(name) = request.llm_prompt_name.as_deref() {
        let matches = prompts
            .iter()
            .filter(|prompt| prompt.name.eq_ignore_ascii_case(name.trim()))
            .collect::<Vec<_>>();
        let prompt = match matches.as_slice() {
            [] => {
                return Err(format!(
                    "Unknown TTS AI cleanup prompt name '{}'",
                    name.trim()
                ))
            }
            [prompt] => *prompt,
            _ => {
                return Err(format!(
                    "More than one TTS AI cleanup prompt is named '{}'",
                    name.trim()
                ))
            }
        };
        *selected_id = prompt.id.clone();
    }
    Ok(())
}

fn prepare_regeneration_output(
    app: &AppHandle,
    external_output: Option<&str>,
    requested_format: Option<TtsOutputFormat>,
    source_format: TtsOutputFormat,
) -> Result<(PathBuf, TtsOutputFormat, Option<TemporaryOutputFile>), String> {
    if let Some(external_output) = external_output {
        let output_path = absolute_path(Path::new(external_output.trim()))?;
        if output_path.exists() {
            return Err(format!(
                "Output file already exists: {}",
                output_path.display()
            ));
        }
        let output_format = resolve_output_format(&output_path, requested_format)?;
        return Ok((output_path, output_format, None));
    }

    let output_format = requested_format.unwrap_or(source_format);
    let extension = match output_format {
        TtsOutputFormat::Mp3 => "mp3",
        TtsOutputFormat::Wav => "wav",
    };
    let directory = app
        .path()
        .app_cache_dir()
        .map_err(|error| format!("Failed to resolve app cache directory: {error}"))?
        .join("tts-history-regeneration");
    fs::create_dir_all(&directory).map_err(|error| {
        format!(
            "Failed to create TTS history regeneration cache {}: {error}",
            directory.display()
        )
    })?;
    let sequence = REGENERATION_FILE_ID.fetch_add(1, Ordering::Relaxed);
    let output_path = directory.join(format!(
        ".output-{}-{sequence}.{extension}",
        std::process::id()
    ));
    if output_path.exists() {
        return Err(format!(
            "Temporary regeneration output collision: {}",
            output_path.display()
        ));
    }
    Ok((
        output_path.clone(),
        output_format,
        Some(TemporaryOutputFile(output_path)),
    ))
}

fn resolve_output_format(
    output_path: &Path,
    requested: Option<TtsOutputFormat>,
) -> Result<TtsOutputFormat, String> {
    let from_extension = match output_path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("mp3") => TtsOutputFormat::Mp3,
        Some("wav") => TtsOutputFormat::Wav,
        _ => return Err("Regeneration output must end in .mp3 or .wav".to_string()),
    };
    if let Some(requested) = requested {
        if requested != from_extension {
            return Err(format!(
                "Requested output format does not match the {} extension",
                output_path.display()
            ));
        }
    }
    Ok(from_extension)
}

fn set_model_and_voice(
    settings: &mut TtsSettings,
    provider: TtsProvider,
    model: Option<&str>,
    voice: Option<&str>,
) -> Result<(), String> {
    if provider == TtsProvider::Deepgram {
        if let (Some(model), Some(voice)) = (model, voice) {
            if !model.trim().eq_ignore_ascii_case(voice.trim()) {
                return Err(
                    "Deepgram uses one model identifier as its voice; --model and --voice must match when both are supplied"
                        .to_string(),
                );
            }
        }
    }
    if let Some(model) = model {
        let model = nonempty_override("--model", model)?;
        match provider {
            TtsProvider::Soniox => settings.soniox_model = model,
            TtsProvider::Deepgram => settings.deepgram_model = model,
            TtsProvider::OpenAi => settings.openai_model = model,
            TtsProvider::Murf => {
                if !matches!(model.as_str(), "falcon-2" | "gen2") {
                    return Err("Murf model must be falcon-2 or gen2".to_string());
                }
                if settings.murf_model != model && voice.is_none() {
                    settings.murf_voice = if model == "gen2" {
                        DEFAULT_TTS_MURF_GEN2_VOICE
                    } else {
                        DEFAULT_TTS_MURF_VOICE
                    }
                    .to_string();
                }
                settings.murf_model = model;
            }
            TtsProvider::ElevenLabs => {
                if !matches!(
                    model.as_str(),
                    "eleven_v3" | "eleven_multilingual_v2"
                ) {
                    return Err(
                        "ElevenLabs model must be eleven_v3 or eleven_multilingual_v2"
                            .to_string(),
                    );
                }
                settings.elevenlabs_model = model;
                if settings.elevenlabs_model == "eleven_v3" {
                    settings.speed = 1.0;
                }
            }
            TtsProvider::Cartesia => {
                if model != "sonic-3.5" {
                    return Err("Cartesia uses the fixed model sonic-3.5".to_string());
                }
                settings.cartesia_model = model;
            }
            TtsProvider::OpenAiCompatible => settings.openai_compatible_model = model,
            TtsProvider::Edge => {
                if model != crate::managers::edge_tts::EDGE_TTS_MODEL {
                    return Err(format!(
                        "Edge-TTS uses the fixed model {}",
                        crate::managers::edge_tts::EDGE_TTS_MODEL
                    ));
                }
            }
            TtsProvider::LocalQwen => {
                let expected_repo = crate::managers::local_tts::LOCAL_TTS_MODEL_REPO;
                let expected_revision = crate::managers::local_tts::LOCAL_TTS_MODEL_REVISION;
                if model != expected_repo
                    && model != expected_revision
                    && model != format!("{expected_repo}@{expected_revision}")
                {
                    return Err(format!(
                        "Local Qwen3-TTS uses the pinned model {expected_repo}@{expected_revision}"
                    ));
                }
            }
            TtsProvider::LocalKokoro => {
                let expected_repo = crate::managers::local_kokoro::KOKORO_MODEL_REPOSITORY;
                let expected_revision = crate::managers::local_kokoro::KOKORO_MODEL_REVISION;
                if model != expected_repo
                    && model != expected_revision
                    && model != format!("{expected_repo}@{expected_revision}")
                {
                    return Err(format!(
                        "Local Kokoro uses the pinned model {expected_repo}@{expected_revision}"
                    ));
                }
            }
            TtsProvider::Windows => {
                if model != "windows.media.speechsynthesis" {
                    return Err(
                        "Windows voices use the fixed model windows.media.speechsynthesis"
                            .to_string(),
                    );
                }
            }
        }
    }
    if let Some(voice) = voice {
        // An empty Windows voice ID is a deliberate, persisted selection of
        // the current OS default. Other providers still require a concrete
        // non-empty voice/model identifier.
        let voice = if provider == TtsProvider::Windows {
            voice.trim().to_string()
        } else {
            nonempty_override("--voice", voice)?
        };
        match provider {
            TtsProvider::Soniox => settings.soniox_voice = voice,
            // Deepgram represents its voice as the speak endpoint's model.
            TtsProvider::Deepgram => settings.deepgram_model = voice,
            TtsProvider::OpenAi => settings.openai_voice = voice,
            TtsProvider::Murf => settings.murf_voice = voice,
            TtsProvider::ElevenLabs => settings.elevenlabs_voice = voice,
            TtsProvider::Cartesia => settings.cartesia_voice = voice,
            TtsProvider::OpenAiCompatible => settings.openai_compatible_voice = voice,
            TtsProvider::Edge => {
                settings.edge_voice_language = crate::managers::edge_tts::voice_language(&voice);
                settings.edge_voice = voice;
            }
            TtsProvider::LocalQwen => settings.local_qwen_voice = voice,
            TtsProvider::LocalKokoro => settings.local_kokoro_voice = voice,
            TtsProvider::Windows => settings.windows_voice_id = voice,
        }
    }
    Ok(())
}

fn nonempty_override(flag: &str, value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        Err(format!("{flag} must not be empty"))
    } else {
        Ok(value.to_string())
    }
}

fn current_model_and_voice(settings: &TtsSettings) -> (String, String) {
    match settings.provider {
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
        TtsProvider::Edge => (
            crate::managers::edge_tts::EDGE_TTS_MODEL.to_string(),
            settings.edge_voice.clone(),
        ),
        TtsProvider::LocalQwen => (
            format!(
                "{}@{}",
                crate::managers::local_tts::LOCAL_TTS_MODEL_REPO,
                crate::managers::local_tts::LOCAL_TTS_MODEL_REVISION
            ),
            settings.local_qwen_voice.clone(),
        ),
        TtsProvider::LocalKokoro => (
            format!(
                "{}@{}",
                crate::managers::local_kokoro::KOKORO_MODEL_REPOSITORY,
                crate::managers::local_kokoro::KOKORO_MODEL_REVISION
            ),
            settings.local_kokoro_voice.clone(),
        ),
        TtsProvider::Windows => (
            "windows.media.speechsynthesis".to_string(),
            settings.windows_voice_id.clone(),
        ),
    }
}

fn current_language(settings: &TtsSettings) -> String {
    match settings.provider {
        TtsProvider::Soniox => settings.soniox_language.clone(),
        TtsProvider::LocalQwen => settings.local_qwen_language.clone(),
        TtsProvider::LocalKokoro => settings.local_kokoro_language.clone(),
        TtsProvider::Windows => settings.windows_voice_language.clone(),
        TtsProvider::Edge => settings.edge_voice_language.clone(),
        TtsProvider::Murf => settings.murf_language.clone(),
        TtsProvider::ElevenLabs => settings.elevenlabs_language.clone(),
        TtsProvider::Cartesia => settings.cartesia_language.clone(),
        TtsProvider::Deepgram | TtsProvider::OpenAi | TtsProvider::OpenAiCompatible => String::new(),
    }
}

fn restore_source_language(settings: &mut TtsSettings, source_language: &str) {
    let source_language = source_language.trim();
    if source_language.is_empty() {
        return;
    }
    match settings.provider {
        TtsProvider::Soniox => settings.soniox_language = source_language.to_string(),
        TtsProvider::LocalQwen => settings.local_qwen_language = source_language.to_string(),
        TtsProvider::LocalKokoro => settings.local_kokoro_language = source_language.to_string(),
        TtsProvider::Windows => settings.windows_voice_language = source_language.to_string(),
        TtsProvider::Edge => settings.edge_voice_language = source_language.to_string(),
        TtsProvider::Murf => settings.murf_language = source_language.to_string(),
        TtsProvider::ElevenLabs => settings.elevenlabs_language = source_language.to_string(),
        TtsProvider::Cartesia => settings.cartesia_language = source_language.to_string(),
        TtsProvider::Deepgram | TtsProvider::OpenAi | TtsProvider::OpenAiCompatible => {}
    }
}

fn resolve_regeneration_prompt(
    settings: &TtsSettings,
    source_entry: &TtsHistoryEntry,
    request: &RegenerateTtsHistoryRequest,
) -> Result<ResolvedPrompt, String> {
    if settings.provider != TtsProvider::OpenAi {
        if request.instructions.is_some()
            || request.prompt_preset_id.is_some()
            || request.prompt_preset_name.is_some()
        {
            return Err(
                "TTS instruction prompts require the OpenAI provider for regeneration".to_string(),
            );
        }
        return Ok(ResolvedPrompt {
            preset_id: None,
            preset_name: None,
            instructions: None,
        });
    }

    if let Some(instructions) = request.instructions.as_ref() {
        return Ok(ResolvedPrompt {
            preset_id: None,
            preset_name: None,
            instructions: nonempty_optional(instructions),
        });
    }

    if request.prompt_preset_id.is_some() && request.prompt_preset_name.is_some() {
        return Err("Specify a TTS prompt preset by either ID or name, not both".to_string());
    }
    if let Some(id) = request.prompt_preset_id.as_deref() {
        let preset = settings
            .prompt_presets
            .iter()
            .find(|preset| preset.id == id)
            .ok_or_else(|| format!("Unknown TTS prompt preset ID '{id}'"))?;
        return Ok(ResolvedPrompt {
            preset_id: Some(preset.id.clone()),
            preset_name: Some(preset.name.clone()),
            instructions: nonempty_optional(&preset.instructions),
        });
    }
    if let Some(name) = request.prompt_preset_name.as_deref() {
        let matches = settings
            .prompt_presets
            .iter()
            .filter(|preset| preset.name.eq_ignore_ascii_case(name.trim()))
            .collect::<Vec<_>>();
        let preset = match matches.as_slice() {
            [] => return Err(format!("Unknown TTS prompt preset '{}'", name.trim())),
            [preset] => *preset,
            _ => {
                return Err(format!(
                    "More than one TTS prompt preset is named '{}'",
                    name.trim()
                ))
            }
        };
        return Ok(ResolvedPrompt {
            preset_id: Some(preset.id.clone()),
            preset_name: Some(preset.name.clone()),
            instructions: nonempty_optional(&preset.instructions),
        });
    }

    if settings.provider == source_entry.provider {
        if source_entry.prompt_preset_id.is_some()
            || source_entry.prompt_preset_name.is_some()
            || source_entry.resolved_instructions.is_some()
        {
            return Ok(ResolvedPrompt {
                preset_id: source_entry.prompt_preset_id.clone(),
                preset_name: source_entry.prompt_preset_name.clone(),
                instructions: source_entry.resolved_instructions.clone(),
            });
        }
    }

    if !settings.selected_prompt_id.trim().is_empty() {
        if let Some(preset) = settings
            .prompt_presets
            .iter()
            .find(|preset| preset.id == settings.selected_prompt_id)
        {
            return Ok(ResolvedPrompt {
                preset_id: Some(preset.id.clone()),
                preset_name: Some(preset.name.clone()),
                instructions: nonempty_optional(&preset.instructions),
            });
        }
    }
    Ok(ResolvedPrompt {
        preset_id: None,
        preset_name: None,
        instructions: nonempty_optional(&settings.openai_instructions),
    })
}

fn enforce_prompt_model_compatibility(
    settings: &TtsSettings,
    explicit_prompt_override: bool,
    prompt: ResolvedPrompt,
) -> Result<ResolvedPrompt, String> {
    if settings.provider != TtsProvider::OpenAi
        || prompt.instructions.is_none()
        || TtsManager::openai_model_supports_instructions(&settings.openai_model)
    {
        return Ok(prompt);
    }
    if explicit_prompt_override {
        return Err(format!(
            "OpenAI voice instructions require a gpt-4o-mini-tts model; selected model is '{}'",
            settings.openai_model.trim()
        ));
    }
    Ok(ResolvedPrompt {
        preset_id: None,
        preset_name: None,
        instructions: None,
    })
}

fn nonempty_optional(value: &str) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn write_temporary_source(
    app: &AppHandle,
    source_text: &str,
    source_kind: TtsHistorySourceKind,
) -> Result<TemporarySourceFile, String> {
    let directory = app
        .path()
        .app_cache_dir()
        .map_err(|error| format!("Failed to resolve app cache directory: {error}"))?
        .join("tts-history-regeneration");
    fs::create_dir_all(&directory).map_err(|error| {
        format!(
            "Failed to create TTS history regeneration cache {}: {error}",
            directory.display()
        )
    })?;
    let sequence = REGENERATION_FILE_ID.fetch_add(1, Ordering::Relaxed);
    let extension = source_kind_extension(source_kind);
    let path = directory.join(format!(
        ".source-{}-{sequence}.{extension}",
        std::process::id()
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|error| {
            format!(
                "Failed to create temporary TTS history source {}: {error}",
                path.display()
            )
        })?;
    if let Err(error) = file
        .write_all(source_text.as_bytes())
        .and_then(|_| file.flush())
        .and_then(|_| file.sync_all())
    {
        let _ = fs::remove_file(&path);
        return Err(format!(
            "Failed to write temporary TTS history source {}: {error}",
            path.display()
        ));
    }
    Ok(TemporarySourceFile(path))
}

fn source_kind_extension(source_kind: TtsHistorySourceKind) -> &'static str {
    match source_kind {
        TtsHistorySourceKind::Text => "txt",
        TtsHistorySourceKind::Markdown => "md",
    }
}

fn absolute_path(path: &Path) -> Result<PathBuf, String> {
    if path.as_os_str().is_empty() {
        return Err("Regeneration output path must not be empty".to_string());
    }
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        std::env::current_dir()
            .map(|directory| directory.join(path))
            .map_err(|error| format!("Failed to resolve current directory: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_format_must_match_extension() {
        assert_eq!(
            resolve_output_format(Path::new("voice.mp3"), None).expect("infer MP3"),
            TtsOutputFormat::Mp3
        );
        assert!(resolve_output_format(Path::new("voice.wav"), Some(TtsOutputFormat::Mp3)).is_err());
        assert!(resolve_output_format(Path::new("voice.flac"), None).is_err());
    }

    #[test]
    fn mp3_bitrate_allowlist_is_stable() {
        assert_eq!(ALLOWED_MP3_BITRATES, &[64, 96, 128, 192, 256, 320]);
    }

    #[test]
    fn markdown_history_regeneration_uses_markdown_normalization_path() {
        assert_eq!(source_kind_extension(TtsHistorySourceKind::Markdown), "md");
        assert_eq!(source_kind_extension(TtsHistorySourceKind::Text), "txt");
    }

    #[test]
    fn incompatible_openai_model_ignores_saved_prompt_but_rejects_override() {
        let mut settings = TtsSettings::default();
        settings.provider = TtsProvider::OpenAi;
        settings.openai_model = "tts-1".to_string();
        let prompt = || ResolvedPrompt {
            preset_id: Some("saved".to_string()),
            preset_name: Some("Saved".to_string()),
            instructions: Some("Speak calmly.".to_string()),
        };

        let inactive = enforce_prompt_model_compatibility(&settings, false, prompt())
            .expect("saved prompt should become inactive");
        assert!(inactive.preset_id.is_none());
        assert!(inactive.preset_name.is_none());
        assert!(inactive.instructions.is_none());
        assert!(enforce_prompt_model_compatibility(&settings, true, prompt()).is_err());
    }

    #[test]
    fn legacy_empty_history_language_keeps_current_provider_language() {
        let mut qwen = TtsSettings {
            provider: TtsProvider::LocalQwen,
            local_qwen_language: "Russian".to_string(),
            ..TtsSettings::default()
        };
        restore_source_language(&mut qwen, "");
        assert_eq!(qwen.local_qwen_language, "Russian");
        restore_source_language(&mut qwen, "English");
        assert_eq!(qwen.local_qwen_language, "English");

        let mut soniox = TtsSettings {
            provider: TtsProvider::Soniox,
            soniox_language: "fi".to_string(),
            ..TtsSettings::default()
        };
        restore_source_language(&mut soniox, "  ");
        assert_eq!(soniox.soniox_language, "fi");
    }

    #[test]
    fn windows_history_accepts_default_voice_and_fixed_model_only() {
        let mut settings = TtsSettings {
            provider: TtsProvider::Windows,
            windows_voice_id: "old-voice".to_string(),
            ..TtsSettings::default()
        };
        set_model_and_voice(
            &mut settings,
            TtsProvider::Windows,
            Some("windows.media.speechsynthesis"),
            Some(""),
        )
        .unwrap();
        assert!(settings.windows_voice_id.is_empty());
        assert!(set_model_and_voice(
            &mut settings,
            TtsProvider::Windows,
            Some("wrong-model"),
            None,
        )
        .is_err());
    }

    #[test]
    fn murf_history_model_override_selects_a_compatible_default_voice() {
        let mut settings = TtsSettings {
            provider: TtsProvider::Murf,
            ..TtsSettings::default()
        };
        set_model_and_voice(
            &mut settings,
            TtsProvider::Murf,
            Some("gen2"),
            None,
        )
        .unwrap();
        assert_eq!(settings.murf_voice, DEFAULT_TTS_MURF_GEN2_VOICE);

        set_model_and_voice(
            &mut settings,
            TtsProvider::Murf,
            Some("falcon-2"),
            None,
        )
        .unwrap();
        assert_eq!(settings.murf_voice, DEFAULT_TTS_MURF_VOICE);
    }
}
