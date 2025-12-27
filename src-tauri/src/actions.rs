#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
use crate::apple_intelligence;
use crate::audio_feedback::{play_feedback_sound, play_feedback_sound_blocking, SoundType};
use crate::audio_toolkit::apply_custom_words;
use crate::managers::audio::AudioRecordingManager;
use crate::managers::connector::ConnectorManager;
use crate::managers::history::HistoryManager;
use crate::managers::remote_stt::RemoteSttManager;
use crate::managers::transcription::TranscriptionManager;
use crate::settings::{
    get_settings, AppSettings, TranscriptionProvider, APPLE_INTELLIGENCE_PROVIDER_ID,
};
use crate::shortcut;
use crate::tray::{change_tray_icon, TrayIconState};
use crate::utils::{
    self, show_recording_overlay, show_sending_overlay, show_thinking_overlay,
    show_transcribing_overlay,
};
use crate::ManagedToggleState;
use async_openai::types::{
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
    ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs,
};
use ferrous_opencc::{config::BuiltinConfig, OpenCC};
use log::{debug, error, info};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager};

// Shortcut Action Trait
pub trait ShortcutAction: Send + Sync {
    fn start(&self, app: &AppHandle, binding_id: &str, shortcut_str: &str);
    fn stop(&self, app: &AppHandle, binding_id: &str, shortcut_str: &str);
}

// Transcribe Action
struct TranscribeAction;

struct AiReplaceSelectionAction;

struct SendToExtensionAction;
struct SendToExtensionWithSelectionAction;
struct SendScreenshotToExtensionAction;

async fn maybe_post_process_transcription(
    settings: &AppSettings,
    transcription: &str,
) -> Option<String> {
    if !settings.post_process_enabled {
        return None;
    }

    let provider = match settings.active_post_process_provider().cloned() {
        Some(provider) => provider,
        None => {
            debug!("Post-processing enabled but no provider is selected");
            return None;
        }
    };

    let model = settings
        .post_process_models
        .get(&provider.id)
        .cloned()
        .unwrap_or_default();

    if model.trim().is_empty() {
        debug!(
            "Post-processing skipped because provider '{}' has no model configured",
            provider.id
        );
        return None;
    }

    let selected_prompt_id = match &settings.post_process_selected_prompt_id {
        Some(id) => id.clone(),
        None => {
            debug!("Post-processing skipped because no prompt is selected");
            return None;
        }
    };

    let prompt = match settings
        .post_process_prompts
        .iter()
        .find(|prompt| prompt.id == selected_prompt_id)
    {
        Some(prompt) => prompt.prompt.clone(),
        None => {
            debug!(
                "Post-processing skipped because prompt '{}' was not found",
                selected_prompt_id
            );
            return None;
        }
    };

    if prompt.trim().is_empty() {
        debug!("Post-processing skipped because the selected prompt is empty");
        return None;
    }

    debug!(
        "Starting LLM post-processing with provider '{}' (model: {})",
        provider.id, model
    );

    // Replace ${output} variable in the prompt with the actual text
    let processed_prompt = prompt.replace("${output}", transcription);
    debug!("Processed prompt length: {} chars", processed_prompt.len());

    if provider.id == APPLE_INTELLIGENCE_PROVIDER_ID {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            if !apple_intelligence::check_apple_intelligence_availability() {
                debug!("Apple Intelligence selected but not currently available on this device");
                return None;
            }

            let token_limit = model.trim().parse::<i32>().unwrap_or(0);
            return match apple_intelligence::process_text(&processed_prompt, token_limit) {
                Ok(result) => {
                    if result.trim().is_empty() {
                        debug!("Apple Intelligence returned an empty response");
                        None
                    } else {
                        debug!(
                            "Apple Intelligence post-processing succeeded. Output length: {} chars",
                            result.len()
                        );
                        Some(result)
                    }
                }
                Err(err) => {
                    error!("Apple Intelligence post-processing failed: {}", err);
                    None
                }
            };
        }

        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            debug!("Apple Intelligence provider selected on unsupported platform");
            return None;
        }
    }

    let api_key = settings
        .post_process_api_keys
        .get(&provider.id)
        .cloned()
        .unwrap_or_default();

    // Create OpenAI-compatible client
    let client = match crate::llm_client::create_client(&provider, api_key) {
        Ok(client) => client,
        Err(e) => {
            error!("Failed to create LLM client: {}", e);
            return None;
        }
    };

    // Build the chat completion request
    let message = match ChatCompletionRequestUserMessageArgs::default()
        .content(processed_prompt)
        .build()
    {
        Ok(msg) => ChatCompletionRequestMessage::User(msg),
        Err(e) => {
            error!("Failed to build chat message: {}", e);
            return None;
        }
    };

    let request = match CreateChatCompletionRequestArgs::default()
        .model(&model)
        .messages(vec![message])
        .build()
    {
        Ok(req) => req,
        Err(e) => {
            error!("Failed to build chat completion request: {}", e);
            return None;
        }
    };

    // Send the request
    match client.chat().create(request).await {
        Ok(response) => {
            if let Some(choice) = response.choices.first() {
                if let Some(content) = &choice.message.content {
                    debug!(
                        "LLM post-processing succeeded for provider '{}'. Output length: {} chars",
                        provider.id,
                        content.len()
                    );
                    return Some(content.clone());
                }
            }
            error!("LLM API response has no content");
            None
        }
        Err(e) => {
            error!(
                "LLM post-processing failed for provider '{}': {}. Falling back to original transcription.",
                provider.id,
                e
            );
            None
        }
    }
}

async fn maybe_convert_chinese_variant(
    settings: &AppSettings,
    transcription: &str,
) -> Option<String> {
    // Check if language is set to Simplified or Traditional Chinese
    let is_simplified = settings.selected_language == "zh-Hans";
    let is_traditional = settings.selected_language == "zh-Hant";

    if !is_simplified && !is_traditional {
        debug!("selected_language is not Simplified or Traditional Chinese; skipping translation");
        return None;
    }

    debug!(
        "Starting Chinese translation using OpenCC for language: {}",
        settings.selected_language
    );

    // Use OpenCC to convert based on selected language
    let config = if is_simplified {
        // Convert Traditional Chinese to Simplified Chinese
        BuiltinConfig::Tw2sp
    } else {
        // Convert Simplified Chinese to Traditional Chinese
        BuiltinConfig::S2twp
    };

    match OpenCC::from_config(config) {
        Ok(converter) => {
            let converted = converter.convert(transcription);
            debug!(
                "OpenCC translation completed. Input length: {}, Output length: {}",
                transcription.len(),
                converted.len()
            );
            Some(converted)
        }
        Err(e) => {
            error!("Failed to initialize OpenCC converter: {}. Falling back to original transcription.", e);
            None
        }
    }
}

fn reset_toggle_state(app: &AppHandle, binding_id: &str) {
    if let Ok(mut states) = app.state::<ManagedToggleState>().lock() {
        if let Some(state) = states.active_toggles.get_mut(binding_id) {
            *state = false;
        }
    }
}

fn emit_ai_replace_error(app: &AppHandle, message: impl Into<String>) {
    let _ = app.emit("ai-replace-error", message.into());
}

// ============================================================================
// Shared Recording Helpers - Reduces duplication across action implementations
// ============================================================================

/// Starts recording with proper audio feedback handling.
/// Handles both always-on and on-demand microphone modes.
/// Returns true if recording was successfully started.
fn start_recording_with_feedback(app: &AppHandle, binding_id: &str) -> bool {
    let settings = get_settings(app);

    // Load model in the background if using local transcription
    let tm = app.state::<Arc<TranscriptionManager>>();
    if settings.transcription_provider == TranscriptionProvider::Local {
        tm.initiate_model_load();
    }

    change_tray_icon(app, TrayIconState::Recording);
    show_recording_overlay(app);

    let rm = app.state::<Arc<AudioRecordingManager>>();
    let is_always_on = settings.always_on_microphone;
    debug!("Microphone mode - always_on: {}", is_always_on);

    let mut recording_started = false;
    if is_always_on {
        // Always-on mode: Play audio feedback immediately, then apply mute after sound finishes
        debug!("Always-on mode: Playing audio feedback immediately");
        let rm_clone = Arc::clone(&rm);
        let app_clone = app.clone();
        std::thread::spawn(move || {
            play_feedback_sound_blocking(&app_clone, SoundType::Start);
            rm_clone.apply_mute();
        });

        recording_started = rm.try_start_recording(binding_id);
        debug!("Recording started: {}", recording_started);
    } else {
        // On-demand mode: Start recording first, then play audio feedback, then apply mute
        debug!("On-demand mode: Starting recording first, then audio feedback");
        let recording_start_time = Instant::now();
        if rm.try_start_recording(binding_id) {
            recording_started = true;
            debug!("Recording started in {:?}", recording_start_time.elapsed());
            let app_clone = app.clone();
            let rm_clone = Arc::clone(&rm);
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(100));
                debug!("Handling delayed audio feedback/mute sequence");
                play_feedback_sound_blocking(&app_clone, SoundType::Start);
                rm_clone.apply_mute();
            });
        } else {
            debug!("Failed to start recording");
        }
    }

    if recording_started {
        shortcut::register_cancel_shortcut(app);
    }

    recording_started
}

// ============================================================================

/// Result of a transcription operation
pub enum TranscriptionOutcome {
    /// Transcription succeeded with the given text
    Success(String),
    /// Operation was cancelled (Remote STT only)
    Cancelled,
    /// Error occurred - for Remote STT, error is already shown in overlay
    Error {
        message: String,
        shown_in_overlay: bool,
    },
}

/// Performs transcription using either local or remote STT based on settings.
///
/// This helper consolidates the common transcription logic used across multiple actions:
/// - Provider selection (local vs remote)
/// - Custom word correction (for remote)
/// - Cancellation tracking (for remote)
/// - Error display in overlay (for remote)
///
/// Returns a TranscriptionOutcome indicating success, cancellation, or error.
async fn perform_transcription(app: &AppHandle, samples: Vec<f32>) -> TranscriptionOutcome {
    let settings = get_settings(app);

    if settings.transcription_provider == TranscriptionProvider::RemoteOpenAiCompatible {
        let remote_manager = app.state::<Arc<RemoteSttManager>>();
        let operation_id = remote_manager.start_operation();

        let result = remote_manager
            .transcribe(&settings.remote_stt, &samples)
            .await
            .map(|text| {
                if settings.custom_words.is_empty() {
                    text
                } else {
                    apply_custom_words(
                        &text,
                        &settings.custom_words,
                        settings.word_correction_threshold,
                    )
                }
            });

        // Check if operation was cancelled while we were waiting
        if remote_manager.is_cancelled(operation_id) {
            debug!(
                "Transcription operation {} was cancelled, discarding result",
                operation_id
            );
            return TranscriptionOutcome::Cancelled;
        }

        match result {
            Ok(text) => TranscriptionOutcome::Success(text),
            Err(err) => {
                let err_str = format!("{}", err);
                let _ = app.emit("remote-stt-error", err_str.clone());
                crate::plus_overlay_state::handle_transcription_error(app, &err_str);
                TranscriptionOutcome::Error {
                    message: err_str,
                    shown_in_overlay: true,
                }
            }
        }
    } else {
        let tm = app.state::<Arc<TranscriptionManager>>();
        match tm.transcribe(samples) {
            Ok(text) => TranscriptionOutcome::Success(text),
            Err(err) => {
                let err_str = format!("{}", err);
                debug!("Local transcription error: {}", err_str);
                TranscriptionOutcome::Error {
                    message: err_str,
                    shown_in_overlay: false,
                }
            }
        }
    }
}

// ============================================================================

/// Prepares the application state for stopping a recording.
/// Handles tray icon, overlay selection, sound, and unmuting.
fn prepare_stop_recording(app: &AppHandle) {
    shortcut::unregister_cancel_shortcut(app);
    let settings = get_settings(app);

    change_tray_icon(app, TrayIconState::Transcribing);
    if settings.transcription_provider == TranscriptionProvider::RemoteOpenAiCompatible {
        show_sending_overlay(app);
    } else {
        show_transcribing_overlay(app);
    }

    let rm = app.state::<Arc<AudioRecordingManager>>();
    rm.remove_mute();

    play_feedback_sound(app, SoundType::Stop);
}

/// Asynchronously stops recording and performs transcription.
/// Handles errors by cleaning up the UI and returning None.
async fn get_transcription_or_cleanup(
    app: &AppHandle,
    binding_id: &str,
) -> Option<(String, Vec<f32>)> {
    let rm = Arc::clone(&app.state::<Arc<AudioRecordingManager>>());

    if let Some(samples) = rm.stop_recording(binding_id) {
        match perform_transcription(app, samples.clone()).await {
            TranscriptionOutcome::Success(text) => Some((text, samples)),
            TranscriptionOutcome::Cancelled => None,
            TranscriptionOutcome::Error {
                shown_in_overlay, ..
            } => {
                if !shown_in_overlay {
                    utils::hide_recording_overlay(app);
                    change_tray_icon(app, TrayIconState::Idle);
                }
                None
            }
        }
    } else {
        debug!("No samples retrieved from recording stop");
        utils::hide_recording_overlay(app);
        change_tray_icon(app, TrayIconState::Idle);
        None
    }
}

/// Applies Chinese conversion, LLM post-processing and saves to history.
async fn apply_post_processing_and_history(
    app: &AppHandle,
    transcription: String,
    samples: Vec<f32>,
) -> String {
    let settings = get_settings(app);
    let mut final_text = transcription.clone();
    let mut post_processed_text: Option<String> = None;
    let mut post_process_prompt: Option<String> = None;

    if let Some(converted_text) = maybe_convert_chinese_variant(&settings, &transcription).await {
        final_text = converted_text.clone();
        post_processed_text = Some(converted_text);
    } else if let Some(processed_text) =
        maybe_post_process_transcription(&settings, &transcription).await
    {
        final_text = processed_text.clone();
        post_processed_text = Some(processed_text);

        if let Some(prompt_id) = &settings.post_process_selected_prompt_id {
            if let Some(prompt) = settings
                .post_process_prompts
                .iter()
                .find(|p| &p.id == prompt_id)
            {
                post_process_prompt = Some(prompt.prompt.clone());
            }
        }
    }

    let hm = Arc::clone(&app.state::<Arc<HistoryManager>>());
    tauri::async_runtime::spawn(async move {
        if let Err(e) = hm
            .save_transcription(
                samples,
                transcription,
                post_processed_text,
                post_process_prompt,
            )
            .await
        {
            error!("Failed to save transcription to history: {}", e);
        }
    });

    final_text
}

// ============================================================================

fn build_extension_message(settings: &AppSettings, instruction: &str, selection: &str) -> String {
    let instruction_trimmed = instruction.trim();
    let selection_trimmed = selection.trim();

    if instruction_trimmed.is_empty() {
        return String::new();
    }

    if selection_trimmed.is_empty() {
        return instruction_trimmed.to_string();
    }

    let user_template = settings.ai_replace_user_prompt.trim();
    let user_message = if user_template.is_empty() {
        format!(
            "INSTRUCTION:\n{}\n\nTEXT:\n{}",
            instruction_trimmed, selection
        )
    } else {
        user_template
            .replace("${instruction}", instruction_trimmed)
            .replace("${output}", selection)
    };

    let system_prompt = settings.ai_replace_system_prompt.trim();
    if system_prompt.is_empty() {
        user_message
    } else {
        format!("SYSTEM:\n{}\n\n{}", system_prompt, user_message)
    }
}

async fn ai_replace_with_llm(
    settings: &AppSettings,
    selected_text: &str,
    instruction: &str,
) -> Result<String, String> {
    let provider = settings
        .active_post_process_provider()
        .cloned()
        .ok_or_else(|| "No LLM provider configured".to_string())?;

    let model = settings
        .post_process_models
        .get(&provider.id)
        .cloned()
        .unwrap_or_default();

    if model.trim().is_empty() {
        return Err(format!(
            "No model configured for provider '{}'",
            provider.label
        ));
    }

    let system_prompt = if selected_text.trim().is_empty() && settings.ai_replace_allow_no_selection
    {
        settings.ai_replace_no_selection_system_prompt.clone()
    } else {
        settings.ai_replace_system_prompt.clone()
    };
    let user_template = settings.ai_replace_user_prompt.clone();
    if user_template.trim().is_empty() {
        return Err("AI replace prompt template is empty".to_string());
    }

    let user_prompt = user_template
        .replace("${output}", selected_text)
        .replace("${instruction}", instruction);

    debug!(
        "AI replace LLM request using provider '{}' (model: {})",
        provider.id, model
    );

    let api_key = settings
        .post_process_api_keys
        .get(&provider.id)
        .cloned()
        .unwrap_or_default();

    let client = crate::llm_client::create_client(&provider, api_key)
        .map_err(|e| format!("Failed to create LLM client: {}", e))?;

    let system_message = ChatCompletionRequestSystemMessageArgs::default()
        .content(system_prompt)
        .build()
        .map(ChatCompletionRequestMessage::System)
        .map_err(|e| format!("Failed to build system message: {}", e))?;

    let user_message = ChatCompletionRequestUserMessageArgs::default()
        .content(user_prompt)
        .build()
        .map(ChatCompletionRequestMessage::User)
        .map_err(|e| format!("Failed to build user message: {}", e))?;

    let estimated_tokens = (selected_text.len() as f32 / 4.0).ceil() as u32;
    let max_tokens = std::cmp::min(
        8192,
        std::cmp::max(256, estimated_tokens.saturating_add(512)),
    );

    let request = CreateChatCompletionRequestArgs::default()
        .model(&model)
        .messages(vec![system_message, user_message])
        .temperature(0.2)
        .max_tokens(max_tokens)
        .build()
        .map_err(|e| format!("Failed to build chat completion request: {}", e))?;

    let response = client
        .chat()
        .create(request)
        .await
        .map_err(|e| format!("LLM request failed: {}", e))?;

    if let Some(choice) = response.choices.first() {
        if let Some(content) = &choice.message.content {
            debug!("AI replace LLM response length: {} chars", content.len());
            return Ok(content.clone());
        }
    }

    Err("LLM API response has no content".to_string())
}

impl ShortcutAction for TranscribeAction {
    fn start(&self, app: &AppHandle, binding_id: &str, _shortcut_str: &str) {
        let start_time = Instant::now();
        debug!("TranscribeAction::start called for binding: {}", binding_id);

        start_recording_with_feedback(app, binding_id);

        debug!(
            "TranscribeAction::start completed in {:?}",
            start_time.elapsed()
        );
    }

    fn stop(&self, app: &AppHandle, binding_id: &str, _shortcut_str: &str) {
        prepare_stop_recording(app);

        let ah = app.clone();
        let binding_id = binding_id.to_string();

        tauri::async_runtime::spawn(async move {
            let (transcription, samples) =
                match get_transcription_or_cleanup(&ah, &binding_id).await {
                    Some(res) => res,
                    None => return,
                };

            if transcription.is_empty() {
                utils::hide_recording_overlay(&ah);
                change_tray_icon(&ah, TrayIconState::Idle);
                return;
            }

            let final_text = apply_post_processing_and_history(&ah, transcription, samples).await;

            let ah_clone = ah.clone();
            ah.run_on_main_thread(move || {
                let _ = utils::paste(final_text, ah_clone.clone());
                utils::hide_recording_overlay(&ah_clone);
                change_tray_icon(&ah_clone, TrayIconState::Idle);
            })
            .ok();
        });
    }
}

impl ShortcutAction for SendToExtensionAction {
    fn start(&self, app: &AppHandle, binding_id: &str, _shortcut_str: &str) {
        let start_time = Instant::now();
        debug!(
            "SendToExtensionAction::start called for binding: {}",
            binding_id
        );

        // Check if extension is online before starting
        let cm = Arc::clone(&app.state::<Arc<ConnectorManager>>());
        if !cm.is_online() {
            debug!("Extension is offline, showing error overlay");
            crate::plus_overlay_state::show_error_overlay(
                app,
                crate::plus_overlay_state::OverlayErrorCategory::ExtensionOffline,
            );
            return;
        }

        start_recording_with_feedback(app, binding_id);

        debug!(
            "SendToExtensionAction::start completed in {:?}",
            start_time.elapsed()
        );
    }

    fn stop(&self, app: &AppHandle, binding_id: &str, _shortcut_str: &str) {
        let cm = Arc::clone(&app.state::<Arc<ConnectorManager>>());
        if !cm.is_online() {
            shortcut::unregister_cancel_shortcut(app);
            return;
        }

        prepare_stop_recording(app);

        let ah = app.clone();
        let cm = Arc::clone(&app.state::<Arc<ConnectorManager>>());
        let binding_id = binding_id.to_string();

        tauri::async_runtime::spawn(async move {
            let (transcription, samples) =
                match get_transcription_or_cleanup(&ah, &binding_id).await {
                    Some(res) => res,
                    None => return,
                };

            if transcription.is_empty() {
                utils::hide_recording_overlay(&ah);
                change_tray_icon(&ah, TrayIconState::Idle);
                return;
            }

            let final_text = apply_post_processing_and_history(&ah, transcription, samples).await;

            match cm.queue_message(&final_text) {
                Ok(()) => debug!("Connector message queued"),
                Err(e) => error!("Failed to queue connector message: {}", e),
            }

            let ah_clone = ah.clone();
            ah.run_on_main_thread(move || {
                utils::hide_recording_overlay(&ah_clone);
                change_tray_icon(&ah_clone, TrayIconState::Idle);
            })
            .ok();
        });
    }
}

impl ShortcutAction for SendToExtensionWithSelectionAction {
    fn start(&self, app: &AppHandle, binding_id: &str, _shortcut_str: &str) {
        let start_time = Instant::now();
        debug!(
            "SendToExtensionWithSelectionAction::start called for binding: {}",
            binding_id
        );

        // Check if extension is online before starting
        let cm = Arc::clone(&app.state::<Arc<ConnectorManager>>());
        if !cm.is_online() {
            debug!("Extension is offline, showing error overlay");
            crate::plus_overlay_state::show_error_overlay(
                app,
                crate::plus_overlay_state::OverlayErrorCategory::ExtensionOffline,
            );
            return;
        }

        start_recording_with_feedback(app, binding_id);

        debug!(
            "SendToExtensionWithSelectionAction::start completed in {:?}",
            start_time.elapsed()
        );
    }

    fn stop(&self, app: &AppHandle, binding_id: &str, _shortcut_str: &str) {
        let cm = Arc::clone(&app.state::<Arc<ConnectorManager>>());
        if !cm.is_online() {
            shortcut::unregister_cancel_shortcut(app);
            return;
        }

        prepare_stop_recording(app);

        let ah = app.clone();
        let cm = Arc::clone(&app.state::<Arc<ConnectorManager>>());
        let binding_id = binding_id.to_string();

        tauri::async_runtime::spawn(async move {
            let (transcription, _) = match get_transcription_or_cleanup(&ah, &binding_id).await {
                Some(res) => res,
                None => return,
            };

            if transcription.trim().is_empty() {
                utils::hide_recording_overlay(&ah);
                change_tray_icon(&ah, TrayIconState::Idle);
                return;
            }

            let settings = get_settings(&ah);
            let selected_text = utils::capture_selection_text_copy(&ah).unwrap_or_default();
            let message = build_extension_message(&settings, &transcription, &selected_text);

            if !message.trim().is_empty() {
                let _ = cm.queue_message(&message);
            }

            let ah_clone = ah.clone();
            ah.run_on_main_thread(move || {
                utils::hide_recording_overlay(&ah_clone);
                change_tray_icon(&ah_clone, TrayIconState::Idle);
            })
            .ok();
        });
    }
}

fn emit_screenshot_error(app: &AppHandle, message: impl Into<String>) {
    let _ = app.emit("screenshot-error", message.into());
}

/// Expands Windows-style environment variables like %USERPROFILE% in a path string.
/// On non-Windows platforms, returns the path unchanged.
#[cfg(target_os = "windows")]
fn expand_env_vars(path: &str) -> String {
    let mut result = path.to_string();
    // Find all %VAR% patterns and replace with actual env values
    while let Some(start) = result.find('%') {
        if let Some(end) = result[start + 1..].find('%') {
            let var_name = &result[start + 1..start + 1 + end];
            if let Ok(value) = std::env::var(var_name) {
                result = result.replace(&format!("%{}%", var_name), &value);
            } else {
                break; // Unknown variable, stop to avoid infinite loop
            }
        } else {
            break; // No closing %, stop
        }
    }
    result
}

#[cfg(not(target_os = "windows"))]
fn expand_env_vars(path: &str) -> String {
    // On Unix, could expand $VAR or ${VAR} if needed, but for now just return as-is
    path.to_string()
}

/// Finds the most recently created image file in a directory (optionally recursive)
fn find_recent_image(
    folder: &std::path::Path,
    max_age_secs: u64,
    recursive: bool,
) -> Option<PathBuf> {
    let cutoff = std::time::SystemTime::now()
        .checked_sub(std::time::Duration::from_secs(max_age_secs))
        .unwrap_or(std::time::UNIX_EPOCH);

    let mut newest: Option<(PathBuf, std::time::SystemTime)> = None;

    fn scan_directory(
        dir: &std::path::Path,
        cutoff: std::time::SystemTime,
        recursive: bool,
        newest: &mut Option<(PathBuf, std::time::SystemTime)>,
    ) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();

                // Recurse into subdirectories if enabled
                if path.is_dir() && recursive {
                    scan_directory(&path, cutoff, recursive, newest);
                    continue;
                }

                if !path.is_file() {
                    continue;
                }

                // Check if it's an image file
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_lowercase());
                let is_image = matches!(
                    ext.as_deref(),
                    Some("png")
                        | Some("jpg")
                        | Some("jpeg")
                        | Some("gif")
                        | Some("webp")
                        | Some("bmp")
                );
                if !is_image {
                    continue;
                }

                // Check modification time
                if let Ok(metadata) = entry.metadata() {
                    if let Ok(modified) = metadata.modified() {
                        if modified > cutoff {
                            if newest.is_none() || modified > newest.as_ref().unwrap().1 {
                                *newest = Some((path, modified));
                            }
                        }
                    }
                }
            }
        }
    }

    scan_directory(folder, cutoff, recursive, &mut newest);
    newest.map(|(path, _)| path)
}

/// Watches a folder for new image files with timeout (optionally recursive)
async fn watch_for_new_image(
    folder: PathBuf,
    timeout_secs: u64,
    require_recent: bool,
    recent_window_secs: u64,
    recursive: bool,
) -> Result<PathBuf, String> {
    use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
    use std::sync::mpsc;
    use std::time::Duration;

    let (tx, rx) = mpsc::channel();

    // Create watcher
    let mut watcher = RecommendedWatcher::new(
        move |res: Result<notify::Event, notify::Error>| {
            if let Ok(event) = res {
                // Only interested in create/modify events
                if matches!(
                    event.kind,
                    notify::EventKind::Create(_) | notify::EventKind::Modify(_)
                ) {
                    for path in event.paths {
                        let ext = path
                            .extension()
                            .and_then(|e| e.to_str())
                            .map(|e| e.to_lowercase());
                        let is_image = matches!(
                            ext.as_deref(),
                            Some("png")
                                | Some("jpg")
                                | Some("jpeg")
                                | Some("gif")
                                | Some("webp")
                                | Some("bmp")
                        );
                        if is_image && path.is_file() {
                            let _ = tx.send(path);
                        }
                    }
                }
            }
        },
        Config::default(),
    )
    .map_err(|e| format!("Failed to create file watcher: {}", e))?;

    // Start watching - use recursive mode if enabled
    let watch_mode = if recursive {
        RecursiveMode::Recursive
    } else {
        RecursiveMode::NonRecursive
    };
    watcher
        .watch(&folder, watch_mode)
        .map_err(|e| format!("Failed to watch folder: {}", e))?;

    // Wait for new file or timeout
    let deadline = std::time::Instant::now() + Duration::from_secs(timeout_secs);

    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            // Timeout - check for recent files if allowed
            if !require_recent {
                if let Some(recent) = find_recent_image(&folder, timeout_secs + 5, recursive) {
                    return Ok(recent);
                }
            }
            return Err("Screenshot timeout: no new image detected".to_string());
        }

        match rx.recv_timeout(remaining.min(Duration::from_millis(500))) {
            Ok(path) => {
                // Give the file system a moment to finish writing
                tokio::time::sleep(Duration::from_millis(100)).await;
                if path.exists() {
                    return Ok(path);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Check if a recent file appeared (polling fallback)
                if let Some(recent) = find_recent_image(&folder, recent_window_secs, recursive) {
                    return Ok(recent);
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err("File watcher disconnected".to_string());
            }
        }
    }
}

impl ShortcutAction for SendScreenshotToExtensionAction {
    fn start(&self, app: &AppHandle, binding_id: &str, _shortcut_str: &str) {
        let start_time = Instant::now();
        debug!(
            "SendScreenshotToExtensionAction::start called for binding: {}",
            binding_id
        );

        // Check if extension is online before starting
        let cm = Arc::clone(&app.state::<Arc<ConnectorManager>>());
        if !cm.is_online() {
            debug!("Extension is offline, showing error overlay");
            crate::plus_overlay_state::show_error_overlay(
                app,
                crate::plus_overlay_state::OverlayErrorCategory::ExtensionOffline,
            );
            return;
        }

        start_recording_with_feedback(app, binding_id);

        debug!(
            "SendScreenshotToExtensionAction::start completed in {:?}",
            start_time.elapsed()
        );
    }

    fn stop(&self, app: &AppHandle, binding_id: &str, _shortcut_str: &str) {
        let cm = Arc::clone(&app.state::<Arc<ConnectorManager>>());
        if !cm.is_online() {
            shortcut::unregister_cancel_shortcut(app);
            return;
        }

        prepare_stop_recording(app);

        let ah = app.clone();
        let cm = Arc::clone(&app.state::<Arc<ConnectorManager>>());
        let binding_id = binding_id.to_string();

        tauri::async_runtime::spawn(async move {
            let (voice_text, _) = match get_transcription_or_cleanup(&ah, &binding_id).await {
                Some(res) => res,
                None => return,
            };

            let settings = get_settings(&ah);
            let final_voice_text =
                if voice_text.trim().is_empty() && settings.screenshot_allow_no_voice {
                    settings.screenshot_no_voice_default_prompt.clone()
                } else {
                    voice_text
                };

            // Hide overlay immediately after transcription
            utils::hide_recording_overlay(&ah);
            change_tray_icon(&ah, TrayIconState::Idle);

            // Launch screenshot tool
            let capture_command = settings.screenshot_capture_command.clone();
            if !capture_command.trim().is_empty() {
                #[cfg(target_os = "windows")]
                let _ = std::process::Command::new("powershell")
                    .args(["-NoProfile", "-Command", &capture_command])
                    .spawn();
            }

            // Wait for screenshot
            let screenshot_folder = PathBuf::from(expand_env_vars(&settings.screenshot_folder));
            let timeout = settings.screenshot_timeout_seconds as u64;

            if let Ok(path) = watch_for_new_image(
                screenshot_folder,
                timeout,
                settings.screenshot_require_recent,
                timeout,
                settings.screenshot_include_subfolders,
            )
            .await
            {
                let _ = cm.queue_bundle_message(&final_voice_text, &path);
            } else {
                emit_screenshot_error(&ah, "Screenshot timeout");
            }
        });
    }
}

impl ShortcutAction for AiReplaceSelectionAction {
    fn start(&self, app: &AppHandle, binding_id: &str, _shortcut_str: &str) {
        let start_time = Instant::now();
        debug!(
            "AiReplaceSelectionAction::start called for binding: {}",
            binding_id
        );

        if !cfg!(target_os = "windows") {
            emit_ai_replace_error(app, "AI Replace Selection is only supported on Windows.");
            reset_toggle_state(app, binding_id);
            return;
        }

        start_recording_with_feedback(app, binding_id);

        debug!(
            "AiReplaceSelectionAction::start completed in {:?}",
            start_time.elapsed()
        );
    }

    fn stop(&self, app: &AppHandle, binding_id: &str, _shortcut_str: &str) {
        prepare_stop_recording(app);

        let ah = app.clone();
        let binding_id = binding_id.to_string();

        tauri::async_runtime::spawn(async move {
            let (transcription, _) = match get_transcription_or_cleanup(&ah, &binding_id).await {
                Some(res) => res,
                None => return,
            };

            if transcription.trim().is_empty() {
                emit_ai_replace_error(&ah, "No instruction captured.");
                utils::hide_recording_overlay(&ah);
                change_tray_icon(&ah, TrayIconState::Idle);
                return;
            }

            let settings = get_settings(&ah);
            let selected_text = utils::capture_selection_text(&ah).unwrap_or_else(|_| {
                if settings.ai_replace_allow_no_selection {
                    String::new()
                } else {
                    "ERROR_NO_SELECTION".to_string()
                }
            });

            if selected_text == "ERROR_NO_SELECTION" {
                emit_ai_replace_error(&ah, "Could not capture selection.");
                utils::hide_recording_overlay(&ah);
                change_tray_icon(&ah, TrayIconState::Idle);
                return;
            }

            show_thinking_overlay(&ah);

            match ai_replace_with_llm(&settings, &selected_text, &transcription).await {
                Ok(output) => {
                    let ah_clone = ah.clone();
                    ah.run_on_main_thread(move || {
                        let _ = utils::paste(output, ah_clone.clone());
                        utils::hide_recording_overlay(&ah_clone);
                        change_tray_icon(&ah_clone, TrayIconState::Idle);
                    })
                    .ok();
                }
                Err(_) => {
                    emit_ai_replace_error(&ah, "AI replace failed.");
                    utils::hide_recording_overlay(&ah);
                    change_tray_icon(&ah, TrayIconState::Idle);
                }
            }
        });
    }
}

// Cancel Action
struct CancelAction;

impl ShortcutAction for CancelAction {
    fn start(&self, app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {
        utils::cancel_current_operation(app);
    }

    fn stop(&self, _app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {
        // Nothing to do on stop for cancel
    }
}

// Test Action
struct TestAction;

impl ShortcutAction for TestAction {
    fn start(&self, app: &AppHandle, binding_id: &str, shortcut_str: &str) {
        log::info!(
            "Shortcut ID '{}': Started - {} (App: {})", // Changed "Pressed" to "Started" for consistency
            binding_id,
            shortcut_str,
            app.package_info().name
        );
    }

    fn stop(&self, app: &AppHandle, binding_id: &str, shortcut_str: &str) {
        log::info!(
            "Shortcut ID '{}': Stopped - {} (App: {})", // Changed "Released" to "Stopped" for consistency
            binding_id,
            shortcut_str,
            app.package_info().name
        );
    }
}

// Static Action Map
pub static ACTION_MAP: Lazy<HashMap<String, Arc<dyn ShortcutAction>>> = Lazy::new(|| {
    let mut map = HashMap::new();
    map.insert(
        "transcribe".to_string(),
        Arc::new(TranscribeAction) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        "send_to_extension".to_string(),
        Arc::new(SendToExtensionAction) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        "send_to_extension_with_selection".to_string(),
        Arc::new(SendToExtensionWithSelectionAction) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        "ai_replace_selection".to_string(),
        Arc::new(AiReplaceSelectionAction) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        "send_screenshot_to_extension".to_string(),
        Arc::new(SendScreenshotToExtensionAction) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        "cancel".to_string(),
        Arc::new(CancelAction) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        "test".to_string(),
        Arc::new(TestAction) as Arc<dyn ShortcutAction>,
    );
    map
});
