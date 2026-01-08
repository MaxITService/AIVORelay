#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
use crate::apple_intelligence;
use crate::audio_feedback::{play_feedback_sound, play_feedback_sound_blocking, SoundType};
use crate::audio_toolkit::apply_custom_words;
use crate::managers::audio::AudioRecordingManager;
use crate::managers::connector::ConnectorManager;
use crate::managers::history::HistoryManager;
use crate::managers::llm_operation::LlmOperationTracker;
use crate::managers::remote_stt::RemoteSttManager;
use crate::managers::transcription::TranscriptionManager;
use crate::session_manager::{self, ManagedSessionState};
use crate::settings::{
    get_settings, AppSettings, TranscriptionProvider, APPLE_INTELLIGENCE_PROVIDER_ID,
};
use crate::tray::{change_tray_icon, TrayIconState};
use crate::utils::{
    self, show_recording_overlay, show_sending_overlay, show_thinking_overlay,
    show_transcribing_overlay,
};
use crate::ManagedToggleState;
use ferrous_opencc::{config::BuiltinConfig, OpenCC};
use log::{debug, error};
use once_cell::sync::Lazy;
use std::collections::{HashMap, HashSet};
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

struct RepastLastAction;

enum PostProcessTranscriptionOutcome {
    Skipped,
    Cancelled,
    Processed {
        text: String,
        prompt_template: String,
    },
}

async fn maybe_post_process_transcription(
    app: &AppHandle,
    settings: &AppSettings,
    transcription: &str,
) -> PostProcessTranscriptionOutcome {
    if !settings.post_process_enabled {
        return PostProcessTranscriptionOutcome::Skipped;
    }

    let provider = match settings.active_post_process_provider().cloned() {
        Some(provider) => provider,
        None => {
            debug!("Post-processing enabled but no provider is selected");
            return PostProcessTranscriptionOutcome::Skipped;
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
        return PostProcessTranscriptionOutcome::Skipped;
    }

    let selected_prompt_id = match &settings.post_process_selected_prompt_id {
        Some(id) => id.clone(),
        None => {
            debug!("Post-processing skipped because no prompt is selected");
            return PostProcessTranscriptionOutcome::Skipped;
        }
    };

    let prompt_template = match settings
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
            return PostProcessTranscriptionOutcome::Skipped;
        }
    };

    if prompt_template.trim().is_empty() {
        debug!("Post-processing skipped because the selected prompt is empty");
        return PostProcessTranscriptionOutcome::Skipped;
    }

    debug!(
        "Starting LLM post-processing with provider '{}' (model: {})",
        provider.id, model
    );

    // Replace ${output} variable in the prompt with the actual text
    let processed_prompt = prompt_template.replace("${output}", transcription);
    debug!("Processed prompt length: {} chars", processed_prompt.len());

    if provider.id == APPLE_INTELLIGENCE_PROVIDER_ID {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            if !apple_intelligence::check_apple_intelligence_availability() {
                debug!("Apple Intelligence selected but not currently available on this device");
                return PostProcessTranscriptionOutcome::Skipped;
            }

            let llm_tracker = app.state::<Arc<LlmOperationTracker>>();
            let operation_id = llm_tracker.start_operation();
            show_thinking_overlay(app);

            let token_limit = model.trim().parse::<i32>().unwrap_or(0);
            return match apple_intelligence::process_text(&processed_prompt, token_limit) {
                Ok(result) => {
                    if llm_tracker.is_cancelled(operation_id) {
                        debug!(
                            "LLM post-processing operation {} was cancelled, discarding result",
                            operation_id
                        );
                        return PostProcessTranscriptionOutcome::Cancelled;
                    }

                    if result.trim().is_empty() {
                        debug!("Apple Intelligence returned an empty response");
                        PostProcessTranscriptionOutcome::Skipped
                    } else {
                        debug!(
                            "Apple Intelligence post-processing succeeded. Output length: {} chars",
                            result.len()
                        );
                        PostProcessTranscriptionOutcome::Processed {
                            text: result,
                            prompt_template,
                        }
                    }
                }
                Err(err) => {
                    if llm_tracker.is_cancelled(operation_id) {
                        debug!(
                            "LLM post-processing operation {} was cancelled, skipping error handling",
                            operation_id
                        );
                        return PostProcessTranscriptionOutcome::Cancelled;
                    }

                    error!("Apple Intelligence post-processing failed: {}", err);
                    PostProcessTranscriptionOutcome::Skipped
                }
            };
        }

        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            debug!("Apple Intelligence provider selected on unsupported platform");
            return PostProcessTranscriptionOutcome::Skipped;
        }
    }

    let llm_tracker = app.state::<Arc<LlmOperationTracker>>();
    let operation_id = llm_tracker.start_operation();
    show_thinking_overlay(app);

    // On Windows, use secure key storage
    #[cfg(target_os = "windows")]
    let api_key = crate::secure_keys::get_post_process_api_key(&provider.id);

    // On non-Windows, use JSON settings
    #[cfg(not(target_os = "windows"))]
    let api_key = settings
        .post_process_api_keys
        .get(&provider.id)
        .cloned()
        .unwrap_or_default();

    // Send the chat completion request
    match crate::llm_client::send_chat_completion(&provider, api_key, &model, processed_prompt)
        .await
    {
        Ok(Some(content)) => {
            if llm_tracker.is_cancelled(operation_id) {
                debug!(
                    "LLM post-processing operation {} was cancelled, discarding result",
                    operation_id
                );
                return PostProcessTranscriptionOutcome::Cancelled;
            }

            debug!(
                "LLM post-processing succeeded for provider '{}'. Output length: {} chars",
                provider.id,
                content.len()
            );
            PostProcessTranscriptionOutcome::Processed {
                text: content,
                prompt_template,
            }
        }
        Ok(None) => {
            if llm_tracker.is_cancelled(operation_id) {
                debug!(
                    "LLM post-processing operation {} was cancelled, skipping error handling",
                    operation_id
                );
                return PostProcessTranscriptionOutcome::Cancelled;
            }

            error!("LLM API response has no content");
            PostProcessTranscriptionOutcome::Skipped
        }
        Err(e) => {
            if llm_tracker.is_cancelled(operation_id) {
                debug!(
                    "LLM post-processing operation {} was cancelled, skipping error handling",
                    operation_id
                );
                return PostProcessTranscriptionOutcome::Cancelled;
            }

            error!(
                "LLM post-processing failed for provider '{}': {}. Falling back to original transcription.",
                provider.id,
                e
            );
            PostProcessTranscriptionOutcome::Skipped
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
///
/// This function creates a recording session that will be stored in managed state.
/// The session's Drop ensures cleanup (cancel shortcut, mute, overlay) happens exactly once.
///
/// IMPORTANT: We hold the session state lock throughout the entire operation to prevent
/// race conditions when the user rapidly presses the shortcut key.
fn start_recording_with_feedback(app: &AppHandle, binding_id: &str) -> bool {
    let settings = get_settings(app);

    // Load model in the background if using local transcription
    let tm = app.state::<Arc<TranscriptionManager>>();
    if settings.transcription_provider == TranscriptionProvider::Local {
        tm.initiate_model_load();
    }

    // Hold the lock for the entire operation to prevent race conditions
    let state = app.state::<ManagedSessionState>();
    let mut state_guard = state.lock().expect("Failed to lock session state");

    // Check if we're already recording or processing
    // During processing, we block new recordings to prevent overlapping operations
    if !matches!(*state_guard, session_manager::SessionState::Idle) {
        debug!("start_recording_with_feedback: System busy (recording or processing), ignoring");
        return false;
    }

    // Mark as recording immediately to prevent concurrent starts
    // We'll update with the real session once recording actually starts
    // For now, create a placeholder session
    let session = Arc::new(session_manager::RecordingSession::new_with_resources(
        app, true, // cancel shortcut will be registered
        true, // mute may be applied (session tracks this for cleanup)
    ));

    *state_guard = session_manager::SessionState::Recording {
        session: Arc::clone(&session),
        binding_id: binding_id.to_string(),
    };

    // Now release the lock before doing I/O operations
    drop(state_guard);

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
        // Register cancel shortcut now that recording is confirmed
        session.register_cancel_shortcut();
    } else {
        // Recording failed - clean up
        // Take the session back and let it drop (which will clean up)
        let state = app.state::<ManagedSessionState>();
        let mut state_guard = state.lock().expect("Failed to lock session state");
        *state_guard = session_manager::SessionState::Idle;
        drop(state_guard);

        // Session's Drop will handle cleanup, but we also explicitly reset UI
        utils::hide_recording_overlay(app);
        change_tray_icon(app, TrayIconState::Idle);
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
        /// Kept for debugging and future logging; currently only shown_in_overlay is checked
        #[allow(dead_code)]
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
/// Performs transcription with optional profile overrides.
/// When binding_id is provided and matches a transcription profile,
/// uses that profile's language and translation settings.
async fn perform_transcription_for_profile(
    app: &AppHandle,
    samples: Vec<f32>,
    binding_id: Option<&str>,
) -> TranscriptionOutcome {
    let settings = get_settings(app);

    // Check if this binding corresponds to a transcription profile
    let profile = binding_id.and_then(|id| settings.transcription_profile_by_binding(id));

    if settings.transcription_provider == TranscriptionProvider::RemoteOpenAiCompatible {
        // Remote STT doesn't currently support per-profile language/translate overrides
        // Log the profile info if present
        if let Some(p) = &profile {
            log::info!(
                "Transcription using Remote STT with profile '{}' (lang={}, translate={}): base_url={}, model={}",
                p.name,
                p.language,
                p.translate_to_english,
                settings.remote_stt.base_url,
                settings.remote_stt.model_id
            );
        } else {
            log::info!(
                "Transcription using Remote STT: base_url={}, model={}",
                settings.remote_stt.base_url,
                settings.remote_stt.model_id
            );
        }
        let remote_manager = app.state::<Arc<RemoteSttManager>>();
        let operation_id = remote_manager.start_operation();

        let result = remote_manager
            .transcribe(
                &settings.remote_stt,
                &samples,
                // Use profile's system_prompt if available, otherwise fall back to per-model prompt
                profile
                    .as_ref()
                    .map(|p| p.system_prompt.clone())
                    .filter(|p| !p.trim().is_empty())
                    .or_else(|| {
                        settings
                            .transcription_prompts
                            .get(&settings.remote_stt.model_id)
                            .filter(|p| !p.trim().is_empty())
                            .cloned()
                    }),
            )
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

        // Use profile overrides for local transcription if available
        let result = if let Some(p) = &profile {
            log::info!(
                "Transcription using Local model '{}' with profile '{}' (lang={}, translate={})",
                settings.selected_model,
                p.name,
                p.language,
                p.translate_to_english
            );
            tm.transcribe_with_overrides(samples, Some(&p.language), Some(p.translate_to_english))
        } else {
            log::info!(
                "Transcription using Local model: {}",
                settings.selected_model
            );
            tm.transcribe(samples)
        };

        match result {
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
///
/// This function transitions from Recording to Processing state.
/// The session's finish() method handles cleanup (unregistering cancel shortcut).
/// Pass the binding_id to ensure we only stop our own recording.
///
/// IMPORTANT: After calling this, the caller MUST call exit_processing() when
/// the async work is complete (success or error).
fn prepare_stop_recording(app: &AppHandle, binding_id: &str) -> bool {
    // Take the session and transition to Processing state
    let state = app.state::<ManagedSessionState>();
    let mut state_guard = state.lock().expect("Failed to lock session state");

    let session = match &*state_guard {
        session_manager::SessionState::Recording {
            binding_id: current_binding_id,
            session,
        } if current_binding_id == binding_id => {
            let session = Arc::clone(session);
            // Transition to Processing state
            *state_guard = session_manager::SessionState::Processing {
                binding_id: binding_id.to_string(),
            };
            Some(session)
        }
        session_manager::SessionState::Recording {
            binding_id: current_binding_id,
            ..
        } => {
            debug!(
                "prepare_stop_recording: Binding mismatch (expected {}, got {})",
                binding_id, current_binding_id
            );
            None
        }
        session_manager::SessionState::Processing { .. } => {
            debug!("prepare_stop_recording: Already in Processing state");
            None
        }
        session_manager::SessionState::Idle => {
            debug!(
                "prepare_stop_recording: No active session for binding {}",
                binding_id
            );
            None
        }
    };

    // Release lock before doing I/O
    drop(state_guard);

    if let Some(session) = session {
        // Explicitly finish the session to trigger cleanup
        // This unregisters the cancel shortcut exactly once
        session.finish();

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
        true
    } else {
        false
    }
}

/// Asynchronously stops recording and performs transcription.
/// Handles errors by cleaning up the UI and returning None.
async fn get_transcription_or_cleanup(
    app: &AppHandle,
    binding_id: &str,
) -> Option<(String, Vec<f32>)> {
    let rm = Arc::clone(&app.state::<Arc<AudioRecordingManager>>());

    if let Some(samples) = rm.stop_recording(binding_id) {
        // Quick Tap Optimization: If audio is less than threshold, skip STT
        // 16000 Hz * (threshold_ms / 1000)
        let settings = get_settings(app);
        let threshold_samples =
            (settings.ai_replace_quick_tap_threshold_ms as f32 / 1000.0 * 16000.0) as usize;

        if samples.len() < threshold_samples {
            debug!(
                "Quick tap detected ({} samples < {}), skipping transcription",
                samples.len(),
                threshold_samples
            );
            return Some((String::new(), samples));
        }

        match perform_transcription_for_profile(app, samples.clone(), Some(binding_id)).await {
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
) -> Option<String> {
    let settings = get_settings(app);
    let mut final_text = transcription.clone();
    let mut post_processed_text: Option<String> = None;
    let mut post_process_prompt: Option<String> = None;

    if let Some(converted_text) = maybe_convert_chinese_variant(&settings, &transcription).await {
        final_text = converted_text.clone();
        post_processed_text = Some(converted_text);
    } else {
        match maybe_post_process_transcription(app, &settings, &transcription).await {
            PostProcessTranscriptionOutcome::Skipped => {}
            PostProcessTranscriptionOutcome::Cancelled => {
                return None;
            }
            PostProcessTranscriptionOutcome::Processed {
                text,
                prompt_template,
            } => {
                final_text = text.clone();
                post_processed_text = Some(text);
                post_process_prompt = Some(prompt_template);
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

    Some(final_text)
}

// ============================================================================

fn build_extension_message(settings: &AppSettings, instruction: &str, selection: &str) -> String {
    let instruction_trimmed = instruction.trim();
    let selection_trimmed = selection.trim();

    if instruction_trimmed.is_empty() {
        if settings.send_to_extension_with_selection_allow_no_voice {
            let system_prompt = settings
                .send_to_extension_with_selection_no_voice_system_prompt
                .trim();
            if system_prompt.is_empty() {
                return selection_trimmed.to_string();
            } else {
                return format!("SYSTEM:\n{}\n\n{}", system_prompt, selection_trimmed);
            }
        } else {
            return String::new();
        }
    }

    if selection_trimmed.is_empty() {
        return instruction_trimmed.to_string();
    }

    let user_template = settings.send_to_extension_with_selection_user_prompt.trim();
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

    let system_prompt = settings
        .send_to_extension_with_selection_system_prompt
        .trim();
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
        .active_ai_replace_provider()
        .cloned()
        .ok_or_else(|| "No LLM provider configured".to_string())?;

    let model = settings.ai_replace_model(&provider.id);

    if model.trim().is_empty() {
        return Err(format!(
            "No model configured for provider '{}'",
            provider.label
        ));
    }

    let system_prompt = if instruction.trim().is_empty() && settings.ai_replace_allow_quick_tap {
        settings.ai_replace_quick_tap_system_prompt.clone()
    } else if selected_text.trim().is_empty() && settings.ai_replace_allow_no_selection {
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

    let api_key = settings.ai_replace_api_key(&provider.id);

    // Use the new HTTP-based LLM client
    match crate::llm_client::send_chat_completion_with_system(
        &provider,
        api_key,
        &model,
        system_prompt,
        user_prompt,
    )
    .await
    {
        Ok(Some(content)) => {
            debug!("AI replace LLM response length: {} chars", content.len());
            Ok(content)
        }
        Ok(None) => Err("LLM API response has no content".to_string()),
        Err(e) => Err(format!("LLM request failed: {}", e)),
    }
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
        if !prepare_stop_recording(app, binding_id) {
            return; // No active session - nothing to do
        }

        let ah = app.clone();
        let binding_id = binding_id.to_string();

        tauri::async_runtime::spawn(async move {
            let (transcription, samples) =
                match get_transcription_or_cleanup(&ah, &binding_id).await {
                    Some(res) => res,
                    None => {
                        session_manager::exit_processing(&ah);
                        return;
                    }
                };

            if transcription.is_empty() {
                utils::hide_recording_overlay(&ah);
                change_tray_icon(&ah, TrayIconState::Idle);
                session_manager::exit_processing(&ah);
                return;
            }

            let final_text =
                match apply_post_processing_and_history(&ah, transcription, samples).await {
                    Some(text) => text,
                    None => {
                        session_manager::exit_processing(&ah);
                        return;
                    }
                };

            let ah_clone = ah.clone();
            ah.run_on_main_thread(move || {
                let _ = utils::paste(final_text, ah_clone.clone());
                utils::hide_recording_overlay(&ah_clone);
                change_tray_icon(&ah_clone, TrayIconState::Idle);
            })
            .ok();

            session_manager::exit_processing(&ah);
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
            // Extension went offline - take session to trigger cleanup via Drop
            let _ = session_manager::take_session_if_matches(app, binding_id);
            return;
        }

        if !prepare_stop_recording(app, binding_id) {
            return; // No active session - nothing to do
        }

        let ah = app.clone();
        let cm = Arc::clone(&app.state::<Arc<ConnectorManager>>());
        let binding_id = binding_id.to_string();

        tauri::async_runtime::spawn(async move {
            let (transcription, samples) =
                match get_transcription_or_cleanup(&ah, &binding_id).await {
                    Some(res) => res,
                    None => {
                        session_manager::exit_processing(&ah);
                        return;
                    }
                };

            if transcription.is_empty() {
                utils::hide_recording_overlay(&ah);
                change_tray_icon(&ah, TrayIconState::Idle);
                session_manager::exit_processing(&ah);
                return;
            }

            let final_text =
                match apply_post_processing_and_history(&ah, transcription, samples).await {
                    Some(text) => text,
                    None => {
                        session_manager::exit_processing(&ah);
                        return;
                    }
                };

            match cm.queue_message(&final_text) {
                Ok(id) => debug!("Connector message queued with id: {}", id),
                Err(e) => error!("Failed to queue connector message: {}", e),
            }

            let ah_clone = ah.clone();
            ah.run_on_main_thread(move || {
                utils::hide_recording_overlay(&ah_clone);
                change_tray_icon(&ah_clone, TrayIconState::Idle);
            })
            .ok();

            session_manager::exit_processing(&ah);
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
            // Extension went offline - take session to trigger cleanup via Drop
            let _ = session_manager::take_session_if_matches(app, binding_id);
            return;
        }

        if !prepare_stop_recording(app, binding_id) {
            return; // No active session - nothing to do
        }

        let ah = app.clone();
        let cm = Arc::clone(&app.state::<Arc<ConnectorManager>>());
        let binding_id = binding_id.to_string();

        tauri::async_runtime::spawn(async move {
            let (transcription, samples) =
                match get_transcription_or_cleanup(&ah, &binding_id).await {
                    Some(res) => res,
                    None => {
                        session_manager::exit_processing(&ah);
                        return;
                    }
                };

            let settings = get_settings(&ah);
            let final_transcription = if transcription.trim().is_empty() {
                if !settings.send_to_extension_with_selection_allow_no_voice {
                    utils::hide_recording_overlay(&ah);
                    change_tray_icon(&ah, TrayIconState::Idle);
                    session_manager::exit_processing(&ah);
                    return;
                }
                String::new()
            } else {
                match apply_post_processing_and_history(&ah, transcription, samples).await {
                    Some(text) => text,
                    None => {
                        session_manager::exit_processing(&ah);
                        return;
                    }
                }
            };

            let selected_text = utils::capture_selection_text_copy(&ah).unwrap_or_default();
            let message = build_extension_message(&settings, &final_transcription, &selected_text);

            if !message.trim().is_empty() {
                let _ = cm.queue_message(&message);
            }

            let ah_clone = ah.clone();
            ah.run_on_main_thread(move || {
                utils::hide_recording_overlay(&ah_clone);
                change_tray_icon(&ah_clone, TrayIconState::Idle);
            })
            .ok();

            session_manager::exit_processing(&ah);
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

/// Collects all image files in a folder into a HashSet for quick existence checks.
fn collect_existing_images(folder: &std::path::Path, recursive: bool) -> HashSet<PathBuf> {
    let mut images = HashSet::new();

    fn scan(dir: &std::path::Path, recursive: bool, images: &mut HashSet<PathBuf>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && recursive {
                    scan(&path, recursive, images);
                    continue;
                }
                if !path.is_file() {
                    continue;
                }
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_lowercase());
                if matches!(
                    ext.as_deref(),
                    Some("png")
                        | Some("jpg")
                        | Some("jpeg")
                        | Some("gif")
                        | Some("webp")
                        | Some("bmp")
                ) {
                    images.insert(path);
                }
            }
        }
    }
    scan(folder, recursive, &mut images);
    images
}

/// Finds the newest image in a folder, optionally recursive.
fn find_newest_image(folder: &std::path::Path, recursive: bool) -> Option<PathBuf> {
    let mut newest: Option<(PathBuf, std::time::SystemTime)> = None;

    fn scan(
        dir: &std::path::Path,
        recursive: bool,
        newest: &mut Option<(PathBuf, std::time::SystemTime)>,
    ) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && recursive {
                    scan(&path, recursive, newest);
                    continue;
                }
                if !path.is_file() {
                    continue;
                }
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_lowercase());
                if matches!(
                    ext.as_deref(),
                    Some("png")
                        | Some("jpg")
                        | Some("jpeg")
                        | Some("gif")
                        | Some("webp")
                        | Some("bmp")
                ) {
                    if let Ok(metadata) = path.metadata() {
                        if let Ok(modified) = metadata.modified() {
                            if newest.is_none() || modified > newest.as_ref().unwrap().1 {
                                *newest = Some((path, modified));
                            }
                        }
                    }
                }
            }
        }
    }
    scan(folder, recursive, &mut newest);
    newest.map(|(p, _)| p)
}

/// Watches for a NEW image file (created after start_time and not in existing_files).
async fn watch_for_new_image(
    folder: PathBuf,
    timeout_secs: u64,
    recursive: bool,
    existing_files: HashSet<PathBuf>,
    start_time: std::time::SystemTime,
    allow_fallback_to_old: bool,
) -> Result<PathBuf, String> {
    use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
    use std::sync::mpsc;
    use std::time::Duration;

    debug!(
        "watch_for_new_image: folder={}, timeout={}s, existing_files_count={}, recursive={}",
        folder.display(),
        timeout_secs,
        existing_files.len(),
        recursive
    );

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
            // Timeout - check for recent files if fallback is allowed (e.g. strict mode disabled)
            if allow_fallback_to_old {
                if let Some(recent) = find_newest_image(&folder, recursive) {
                    return Ok(recent);
                }
            }
            return Err("Screenshot timeout: no new image detected".to_string());
        }

        // Helper check for "is this a new file"
        let is_new_file = |path: &PathBuf| -> bool {
            let is_known_old = existing_files.contains(path);
            let is_fresh = if let Ok(meta) = path.metadata() {
                if let Ok(modified) = meta.modified() {
                    modified > start_time
                } else {
                    false
                }
            } else {
                false
            };
            // It's new if it wasn't there before, OR it was there but modified recently (overwrite)
            !is_known_old || is_fresh
        };

        match rx.recv_timeout(remaining.min(Duration::from_millis(500))) {
            Ok(path) => {
                debug!("watch_for_new_image: watcher event for {:?}", path);
                // Give the file system a moment to finish writing
                tokio::time::sleep(Duration::from_millis(100)).await;
                let is_new = is_new_file(&path);
                debug!(
                    "watch_for_new_image: path exists={}, is_new={}",
                    path.exists(),
                    is_new
                );
                if path.exists() && is_new {
                    return Ok(path);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Polling fallback: check if any file in folder is new
                // This covers cases where watcher might miss an event
                if let Some(path) = find_newest_image(&folder, recursive) {
                    let is_new = is_new_file(&path);
                    debug!(
                        "watch_for_new_image: polling found {:?}, is_new={}",
                        path, is_new
                    );
                    if is_new {
                        return Ok(path);
                    }
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
            // Extension went offline - take session to trigger cleanup via Drop
            let _ = session_manager::take_session_if_matches(app, binding_id);
            return;
        }

        if !prepare_stop_recording(app, binding_id) {
            return; // No active session - nothing to do
        }

        let ah = app.clone();
        let cm = Arc::clone(&app.state::<Arc<ConnectorManager>>());
        let binding_id = binding_id.to_string();

        tauri::async_runtime::spawn(async move {
            let (voice_text, _) = match get_transcription_or_cleanup(&ah, &binding_id).await {
                Some(res) => res,
                None => {
                    session_manager::exit_processing(&ah);
                    return;
                }
            };

            let settings = get_settings(&ah);
            let final_voice_text =
                if voice_text.trim().is_empty() && settings.screenshot_allow_no_voice {
                    settings.screenshot_no_voice_default_prompt.clone()
                } else {
                    voice_text
                };

            // Hide overlay immediately after transcription (avoid capturing it in screenshots)
            utils::hide_recording_overlay_immediately(&ah);
            change_tray_icon(&ah, TrayIconState::Idle);

            if settings.screenshot_capture_method
                == crate::settings::ScreenshotCaptureMethod::Native
            {
                // Native region capture (Windows only)
                #[cfg(target_os = "windows")]
                {
                    use crate::region_capture::{open_region_picker, RegionCaptureResult};

                    match open_region_picker(&ah, settings.native_region_capture_mode).await {
                        RegionCaptureResult::Selected { region, image_data } => {
                            debug!("Screenshot captured for region: {:?}", region);
                            // Send screenshot bytes directly to connector
                            let _ = cm.queue_bundle_message_bytes(
                                &final_voice_text,
                                image_data,
                                "image/png",
                            );
                        }
                        RegionCaptureResult::Cancelled => {
                            debug!("Screenshot capture cancelled by user");
                            // Just return, no error - user intentionally cancelled
                        }
                        RegionCaptureResult::Error(e) => {
                            emit_screenshot_error(&ah, &e);
                        }
                    }
                }

                #[cfg(not(target_os = "windows"))]
                {
                    emit_screenshot_error(
                        &ah,
                        "Native screenshot capture is only supported on Windows.",
                    );
                }
                session_manager::exit_processing(&ah);
                return;
            }

            // Validate screenshot folder before launching capture tool
            let screenshot_folder = PathBuf::from(expand_env_vars(&settings.screenshot_folder));
            if !screenshot_folder.exists() {
                emit_screenshot_error(
                    &ah,
                    &format!(
                        "Screenshot folder not found: {}",
                        screenshot_folder.display()
                    ),
                );
                session_manager::exit_processing(&ah);
                return;
            }
            if !screenshot_folder.is_dir() {
                emit_screenshot_error(
                    &ah,
                    &format!(
                        "Screenshot path is not a folder: {}",
                        screenshot_folder.display()
                    ),
                );
                session_manager::exit_processing(&ah);
                return;
            }

            // Snapshot existing files to prevent picking up old ones
            let existing_files =
                collect_existing_images(&screenshot_folder, settings.screenshot_include_subfolders);
            let start_time = std::time::SystemTime::now();

            // Launch screenshot tool
            let capture_command = settings.screenshot_capture_command.clone();
            if !capture_command.trim().is_empty() {
                #[cfg(target_os = "windows")]
                let _ = std::process::Command::new("powershell")
                    .args(["-NoProfile", "-Command", &capture_command])
                    .spawn();
            }

            // Wait for screenshot
            let timeout = settings.screenshot_timeout_seconds as u64;
            match watch_for_new_image(
                screenshot_folder,
                timeout,
                settings.screenshot_include_subfolders,
                existing_files,
                start_time,
                !settings.screenshot_require_recent, // Fallback if requirement is disabled
            )
            .await
            {
                Ok(path) => {
                    let _ = cm.queue_bundle_message(&final_voice_text, &path);
                }
                Err(e) => {
                    emit_screenshot_error(&ah, &e);
                }
            }

            session_manager::exit_processing(&ah);
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
        if !prepare_stop_recording(app, binding_id) {
            return; // No active session - nothing to do
        }

        let ah = app.clone();
        let binding_id = binding_id.to_string();

        tauri::async_runtime::spawn(async move {
            let (transcription, _) = match get_transcription_or_cleanup(&ah, &binding_id).await {
                Some(res) => res,
                None => {
                    session_manager::exit_processing(&ah);
                    return;
                }
            };

            let settings = get_settings(&ah);

            if transcription.trim().is_empty() {
                if !settings.ai_replace_allow_quick_tap {
                    emit_ai_replace_error(&ah, "No instruction captured.");
                    utils::hide_recording_overlay(&ah);
                    change_tray_icon(&ah, TrayIconState::Idle);
                    session_manager::exit_processing(&ah);
                    return;
                }
                // proceeding with empty transcription
            }

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
                session_manager::exit_processing(&ah);
                return;
            }

            show_thinking_overlay(&ah);

            // Start LLM operation tracking for cancellation support
            let llm_tracker = ah.state::<Arc<LlmOperationTracker>>();
            let operation_id = llm_tracker.start_operation();

            let hm = Arc::clone(&ah.state::<Arc<HistoryManager>>());
            let instruction_for_history = transcription.clone();
            let selection_for_history = selected_text.clone();

            match ai_replace_with_llm(&settings, &selected_text, &transcription).await {
                Ok(output) => {
                    // Check if operation was cancelled while we were waiting
                    if llm_tracker.is_cancelled(operation_id) {
                        debug!(
                            "LLM operation {} was cancelled, discarding result",
                            operation_id
                        );
                        // Overlay already hidden by cancel_current_operation
                        // exit_processing already called by cancel
                        return;
                    }

                    // Save to history with AI response
                    let hm_clone = Arc::clone(&hm);
                    let instruction_clone = instruction_for_history.clone();
                    let selection_clone = selection_for_history.clone();
                    let output_for_history = output.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Err(e) = hm_clone
                            .save_ai_replace_entry(
                                instruction_clone,
                                selection_clone,
                                Some(output_for_history),
                            )
                            .await
                        {
                            error!("Failed to save AI Replace entry to history: {}", e);
                        }
                    });

                    let ah_clone = ah.clone();
                    ah.run_on_main_thread(move || {
                        let _ = utils::paste(output, ah_clone.clone());
                        utils::hide_recording_overlay(&ah_clone);
                        change_tray_icon(&ah_clone, TrayIconState::Idle);
                    })
                    .ok();
                }
                Err(_) => {
                    // Check if cancelled - if so, skip error reporting
                    if llm_tracker.is_cancelled(operation_id) {
                        debug!(
                            "LLM operation {} was cancelled, skipping error handling",
                            operation_id
                        );
                        // exit_processing already called by cancel
                        return;
                    }

                    // Save to history with no AI response (indicates failure)
                    tauri::async_runtime::spawn(async move {
                        if let Err(e) = hm
                            .save_ai_replace_entry(
                                instruction_for_history,
                                selection_for_history,
                                None, // Response never received
                            )
                            .await
                        {
                            error!("Failed to save AI Replace entry to history: {}", e);
                        }
                    });

                    emit_ai_replace_error(&ah, "AI replace failed.");
                    utils::hide_recording_overlay(&ah);
                    change_tray_icon(&ah, TrayIconState::Idle);
                }
            }

            session_manager::exit_processing(&ah);
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

// Repaste Last Action
impl ShortcutAction for RepastLastAction {
    fn start(&self, app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {
        debug!("RepastLastAction::start called");

        let ah = app.clone();

        tauri::async_runtime::spawn(async move {
            let hm = Arc::clone(&ah.state::<Arc<HistoryManager>>());

            match hm.get_latest_entry().await {
                Ok(Some(entry)) => {
                    // Determine what text to paste based on action type
                    let text_to_paste = match entry.action_type.as_str() {
                        "ai_replace" => {
                            // For AI Replace, use the AI response if available
                            match entry.ai_response {
                                Some(response) => response,
                                None => {
                                    // AI response never received
                                    let _ = ah.emit(
                                        "repaste-error",
                                        "AI response was never received for this entry.",
                                    );
                                    return;
                                }
                            }
                        }
                        _ => {
                            // For regular transcription, prefer post-processed text, fall back to transcription
                            entry
                                .post_processed_text
                                .unwrap_or(entry.transcription_text)
                        }
                    };

                    if text_to_paste.trim().is_empty() {
                        let _ = ah.emit("repaste-error", "No text available to repaste.");
                        return;
                    }

                    let ah_clone = ah.clone();
                    ah.run_on_main_thread(move || {
                        let _ = utils::paste(text_to_paste, ah_clone);
                    })
                    .ok();
                }
                Ok(None) => {
                    let _ = ah.emit("repaste-error", "No history entries available.");
                }
                Err(e) => {
                    error!("Failed to get latest history entry: {}", e);
                    let _ = ah.emit("repaste-error", "Failed to retrieve history.");
                }
            }
        });
    }

    fn stop(&self, _app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {
        // Repaste is instant, nothing to do on stop
    }
}

// ============================================================================
// Voice Command Action (Windows only)
// ============================================================================

#[cfg(target_os = "windows")]
struct VoiceCommandAction;

/// Event payload for showing the command confirmation overlay
#[derive(Clone, serde::Serialize, specta::Type)]
pub struct CommandConfirmPayload {
    /// The suggested command to execute
    pub command: String,
    /// What the user said (for context)
    pub spoken_text: String,
    /// Whether this came from LLM (true) or predefined match (false)
    pub from_llm: bool,
    /// PowerShell arguments for execution
    pub ps_args: String,
    /// Whether to open PowerShell window and keep it open
    pub keep_window_open: bool,
    /// Whether to use Windows Terminal instead of classic PowerShell
    pub use_windows_terminal: bool,
}

/// Computes a similarity score between two strings using a simple word-based approach.
/// Returns a value between 0.0 and 1.0.
fn compute_similarity(a: &str, b: &str) -> f64 {
    let a_lower = a.to_lowercase();
    let b_lower = b.to_lowercase();

    // Exact match
    if a_lower == b_lower {
        return 1.0;
    }

    // Word-based Jaccard similarity
    let a_words: HashSet<&str> = a_lower.split_whitespace().collect();
    let b_words: HashSet<&str> = b_lower.split_whitespace().collect();

    if a_words.is_empty() || b_words.is_empty() {
        return 0.0;
    }

    let intersection = a_words.intersection(&b_words).count();
    let union = a_words.union(&b_words).count();

    if union == 0 {
        return 0.0;
    }

    intersection as f64 / union as f64
}

/// Finds the best matching predefined command for the given transcription.
/// Returns (command, similarity_score) if a match above threshold is found.
pub fn find_matching_command(
    transcription: &str,
    commands: &[crate::settings::VoiceCommand],
    default_threshold: f64,
) -> Option<(crate::settings::VoiceCommand, f64)> {
    let mut best_match: Option<(crate::settings::VoiceCommand, f64)> = None;

    for cmd in commands.iter().filter(|c| c.enabled) {
        let threshold = if cmd.similarity_threshold > 0.0 {
            cmd.similarity_threshold
        } else {
            default_threshold
        };

        let score = compute_similarity(transcription, &cmd.trigger_phrase);

        if score >= threshold {
            match &best_match {
                Some((_, best_score)) if score > *best_score => {
                    best_match = Some((cmd.clone(), score));
                }
                None => {
                    best_match = Some((cmd.clone(), score));
                }
                _ => {}
            }
        }
    }

    best_match
}

/// Generates a PowerShell command using LLM based on user's spoken request
#[cfg(target_os = "windows")]
pub async fn generate_command_with_llm(
    app: &AppHandle,
    spoken_text: &str,
) -> Result<String, String> {
    let settings = get_settings(app);

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

    let system_prompt = settings.voice_command_system_prompt.clone();
    let user_prompt = spoken_text.to_string();

    #[cfg(target_os = "windows")]
    let api_key = crate::secure_keys::get_post_process_api_key(&provider.id);

    #[cfg(not(target_os = "windows"))]
    let api_key = settings
        .post_process_api_keys
        .get(&provider.id)
        .cloned()
        .unwrap_or_default();

    match crate::llm_client::send_chat_completion_with_system(
        &provider,
        api_key,
        &model,
        system_prompt,
        user_prompt,
    )
    .await
    {
        Ok(Some(content)) => {
            let trimmed = content.trim();
            if trimmed == "UNSAFE_REQUEST" {
                Err("Request was deemed unsafe by the LLM".to_string())
            } else {
                Ok(trimmed.to_string())
            }
        }
        Ok(None) => Err("LLM returned empty response".to_string()),
        Err(e) => Err(format!("LLM request failed: {}", e)),
    }
}

fn emit_voice_command_error(app: &AppHandle, message: impl Into<String>) {
    let _ = app.emit("voice-command-error", message.into());
}

#[cfg(target_os = "windows")]
impl ShortcutAction for VoiceCommandAction {
    fn start(&self, app: &AppHandle, binding_id: &str, _shortcut_str: &str) {
        let start_time = Instant::now();
        debug!(
            "VoiceCommandAction::start called for binding: {}",
            binding_id
        );

        start_recording_with_feedback(app, binding_id);

        debug!(
            "VoiceCommandAction::start completed in {:?}",
            start_time.elapsed()
        );
    }

    fn stop(&self, app: &AppHandle, binding_id: &str, _shortcut_str: &str) {
        if !prepare_stop_recording(app, binding_id) {
            return;
        }

        let ah = app.clone();
        let binding_id = binding_id.to_string();

        tauri::async_runtime::spawn(async move {
            let (transcription, _) = match get_transcription_or_cleanup(&ah, &binding_id).await {
                Some(res) => res,
                None => {
                    session_manager::exit_processing(&ah);
                    return;
                }
            };

            if transcription.trim().is_empty() {
                emit_voice_command_error(&ah, "No command detected");
                utils::hide_recording_overlay(&ah);
                change_tray_icon(&ah, TrayIconState::Idle);
                session_manager::exit_processing(&ah);
                return;
            }

            let settings = get_settings(&ah);

            // Step 1: Try to match against predefined commands
            if let Some((matched_cmd, score)) = find_matching_command(
                &transcription,
                &settings.voice_commands,
                settings.voice_command_default_threshold,
            ) {
                debug!(
                    "Voice command matched: '{}' -> '{}' (score: {:.2})",
                    matched_cmd.trigger_phrase, matched_cmd.script, score
                );

                // Show confirmation overlay
                crate::overlay::show_command_confirm_overlay(
                    &ah,
                    CommandConfirmPayload {
                        command: matched_cmd.script.clone(),
                        spoken_text: transcription.clone(),
                        from_llm: false,
                        ps_args: settings.voice_command_ps_args.clone(),
                        keep_window_open: settings.voice_command_keep_window_open,
                        use_windows_terminal: settings.voice_command_use_windows_terminal,
                    },
                );

                utils::hide_recording_overlay(&ah);
                change_tray_icon(&ah, TrayIconState::Idle);
                session_manager::exit_processing(&ah);
                return;
            }

            // Step 2: No predefined match - try LLM fallback if enabled
            if settings.voice_command_llm_fallback {
                debug!(
                    "No predefined match, using LLM fallback for: '{}'",
                    transcription
                );

                show_thinking_overlay(&ah);

                match generate_command_with_llm(&ah, &transcription).await {
                    Ok(suggested_command) => {
                        debug!("LLM suggested command: '{}'", suggested_command);

                        // Show confirmation overlay
                        crate::overlay::show_command_confirm_overlay(
                            &ah,
                            CommandConfirmPayload {
                                command: suggested_command,
                                spoken_text: transcription,
                                from_llm: true,
                                ps_args: settings.voice_command_ps_args.clone(),
                                keep_window_open: settings.voice_command_keep_window_open,
                                use_windows_terminal: settings.voice_command_use_windows_terminal,
                            },
                        );
                    }
                    Err(e) => {
                        emit_voice_command_error(&ah, format!("Failed to generate command: {}", e));
                    }
                }
            } else {
                emit_voice_command_error(
                    &ah,
                    format!("No matching command found for: '{}'", transcription),
                );
            }

            utils::hide_recording_overlay(&ah);
            change_tray_icon(&ah, TrayIconState::Idle);
            session_manager::exit_processing(&ah);
        });
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
        "repaste_last".to_string(),
        Arc::new(RepastLastAction) as Arc<dyn ShortcutAction>,
    );
    map.insert(
        "test".to_string(),
        Arc::new(TestAction) as Arc<dyn ShortcutAction>,
    );
    #[cfg(target_os = "windows")]
    map.insert(
        "voice_command".to_string(),
        Arc::new(VoiceCommandAction) as Arc<dyn ShortcutAction>,
    );
    map
});
