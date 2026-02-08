#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
use crate::apple_intelligence;
use crate::audio_feedback::{play_feedback_sound, play_feedback_sound_blocking, SoundType};
use crate::audio_toolkit::apply_custom_words;
use crate::managers::audio::AudioRecordingManager;
use crate::managers::connector::ConnectorManager;
use crate::managers::history::HistoryManager;
use crate::managers::llm_operation::LlmOperationTracker;
use crate::managers::model::{EngineType, ModelManager};
use crate::managers::remote_stt::RemoteSttManager;
use crate::managers::soniox_realtime::{FinalChunkCallback, SonioxRealtimeManager, SonioxRealtimeOptions};
use crate::managers::soniox_stt::SonioxSttManager;
use crate::managers::transcription::TranscriptionManager;
use crate::session_manager::{self, ManagedSessionState};
use crate::settings::{
    get_settings, AppSettings, TranscriptionProvider, APPLE_INTELLIGENCE_PROVIDER_ID,
};
use crate::tray::{change_tray_icon, TrayIconState};
use crate::utils::{
    self, show_finalizing_overlay, show_recording_overlay, show_sending_overlay, show_thinking_overlay,
    show_transcribing_overlay,
};
use crate::ManagedToggleState;
use ferrous_opencc::{config::BuiltinConfig, OpenCC};
use log::{debug, error, warn};
use once_cell::sync::Lazy;
use natural::phonetics::soundex;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use strsim::normalized_levenshtein;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;

// Shortcut Action Trait
pub trait ShortcutAction: Send + Sync {
    fn start(&self, app: &AppHandle, binding_id: &str, shortcut_str: &str);
    fn stop(&self, app: &AppHandle, binding_id: &str, shortcut_str: &str);

    /// Returns true if this action is instant (fires on every keypress).
    /// Instant actions bypass toggle state management entirely - each press
    /// triggers `start()` without tracking start/stop state.
    ///
    /// Examples: profile cycling, repaste, cancel - these are one-shot
    /// operations that make no sense as "toggle on/off" actions.
    fn is_instant(&self) -> bool {
        false
    }
}

// Transcribe Action
struct TranscribeAction;

struct AiReplaceSelectionAction;

struct SendToExtensionAction;
struct SendToExtensionWithSelectionAction;
struct SendScreenshotToExtensionAction;

struct RepastLastAction;

struct CycleProfileAction;
#[cfg(target_os = "windows")]
struct SpawnVoiceButtonAction;

use crate::settings::TranscriptionProfile;

enum PostProcessTranscriptionOutcome {
    Skipped,
    Cancelled,
    Processed {
        text: String,
        prompt_template: String,
    },
}

#[derive(Clone, Debug, Default)]
struct StopRecordingContext {
    captured_profile_id: Option<String>,
    current_app: String,
}

#[derive(Clone, Debug, Default)]
struct LlmTemplateContext {
    output: String,
    instruction: String,
    selection: String,
    current_app: String,
    short_prev_transcript: String,
    language: String,
    profile_name: String,
    time_local: String,
    date_iso: String,
    translate_to_english: String,
}

/// Tracks the frontmost app captured at recording start, keyed by binding_id.
static RECORDING_APP_CONTEXT: Lazy<Mutex<HashMap<String, String>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

fn capture_recording_app_context(binding_id: &str) {
    #[cfg(target_os = "windows")]
    let app_name = crate::active_app::get_frontmost_app_name().unwrap_or_default();

    #[cfg(not(target_os = "windows"))]
    let app_name = String::new();

    if let Ok(mut context) = RECORDING_APP_CONTEXT.lock() {
        context.insert(binding_id.to_string(), app_name);
    }
}

fn take_recording_app_context(binding_id: &str) -> String {
    if let Ok(mut context) = RECORDING_APP_CONTEXT.lock() {
        return context.remove(binding_id).unwrap_or_default();
    }
    String::new()
}

fn clamp_prev_transcript_words(settings: &AppSettings) -> usize {
    settings
        .llm_context_prev_transcript_max_words
        .clamp(1, 2000)
}

fn clamp_prev_transcript_expiry(settings: &AppSettings) -> Duration {
    Duration::from_secs(
        settings
            .llm_context_prev_transcript_expiry_seconds
            .clamp(10, 86_400),
    )
}

fn resolve_effective_language(
    app: &AppHandle,
    settings: &AppSettings,
    profile: Option<&TranscriptionProfile>,
) -> String {
    let requested = profile
        .map(|p| p.language.clone())
        .unwrap_or_else(|| settings.selected_language.clone());

    if settings.transcription_provider != TranscriptionProvider::Local {
        return requested;
    }

    let mm = app.state::<Arc<ModelManager>>();
    let is_whisper = mm
        .get_model_info(&settings.selected_model)
        .map(|m| matches!(m.engine_type, EngineType::Whisper))
        .unwrap_or(false);

    if is_whisper && !requested.trim().is_empty() {
        requested
    } else {
        "auto".to_string()
    }
}

fn resolve_effective_translate_to_english(
    settings: &AppSettings,
    profile: Option<&TranscriptionProfile>,
) -> bool {
    profile
        .map(|p| p.translate_to_english)
        .unwrap_or(settings.translate_to_english)
}

fn resolve_profile_name(profile: Option<&TranscriptionProfile>) -> String {
    profile
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "Default".to_string())
}

fn resolve_short_prev_transcript(settings: &AppSettings, current_app: &str) -> String {
    if !settings.llm_context_prev_transcript_enabled || current_app.trim().is_empty() {
        return String::new();
    }

    crate::transcript_context::get_short_prev_transcript(
        current_app,
        clamp_prev_transcript_words(settings),
        clamp_prev_transcript_expiry(settings),
    )
}

fn update_short_prev_transcript(settings: &AppSettings, current_app: &str, transcription: &str) {
    if !settings.llm_context_prev_transcript_enabled
        || current_app.trim().is_empty()
        || transcription.trim().is_empty()
    {
        return;
    }

    crate::transcript_context::update_transcript_context(
        current_app,
        transcription,
        clamp_prev_transcript_words(settings),
        clamp_prev_transcript_expiry(settings),
    );
}

fn build_llm_template_context(
    app: &AppHandle,
    settings: &AppSettings,
    profile: Option<&TranscriptionProfile>,
    current_app: &str,
    output: &str,
    instruction: &str,
    selection: &str,
) -> LlmTemplateContext {
    let now = chrono::Local::now();
    let translate_to_english = resolve_effective_translate_to_english(settings, profile);

    LlmTemplateContext {
        output: output.to_string(),
        instruction: instruction.to_string(),
        selection: selection.to_string(),
        current_app: current_app.to_string(),
        short_prev_transcript: resolve_short_prev_transcript(settings, current_app),
        language: resolve_effective_language(app, settings, profile),
        profile_name: resolve_profile_name(profile),
        time_local: now.format("%A, %B %-d, %Y %-I:%M:%S %p").to_string(),
        date_iso: now.to_rfc3339(),
        translate_to_english: translate_to_english.to_string(),
    }
}

fn apply_llm_template_vars(template: &str, context: &LlmTemplateContext) -> String {
    template
        .replace("${output}", &context.output)
        .replace("${instruction}", &context.instruction)
        .replace("${selection}", &context.selection)
        .replace("${current_app}", &context.current_app)
        .replace("${short_prev_transcript}", &context.short_prev_transcript)
        .replace("${language}", &context.language)
        .replace("${profile_name}", &context.profile_name)
        .replace("${time_local}", &context.time_local)
        .replace("${date_iso}", &context.date_iso)
        .replace("${translate_to_english}", &context.translate_to_english)
}

/// Post-process transcription with LLM, optionally using profile-specific settings.
///
/// If `profile` is Some, uses the profile's LLM settings:
/// - `profile.llm_post_process_enabled` determines if post-processing is enabled
/// - `profile.llm_prompt_override` overrides the global prompt (if set)
/// - `profile.llm_model_override` overrides the global model (if set and valid for current provider)
///
/// If `profile` is None (default profile), uses global settings.
async fn maybe_post_process_transcription(
    app: &AppHandle,
    settings: &AppSettings,
    profile: Option<&TranscriptionProfile>,
    template_context: &LlmTemplateContext,
) -> PostProcessTranscriptionOutcome {
    if settings.transcription_provider == TranscriptionProvider::RemoteSoniox {
        debug!("Skipping post-processing for Soniox streaming transcription");
        return PostProcessTranscriptionOutcome::Skipped;
    }

    // Determine if post-processing is enabled based on profile or global setting
    let is_enabled = match profile {
        Some(p) => p.llm_post_process_enabled,
        None => settings.post_process_enabled,
    };

    if !is_enabled {
        return PostProcessTranscriptionOutcome::Skipped;
    }

    let provider = match settings.active_post_process_provider().cloned() {
        Some(provider) => provider,
        None => {
            debug!("Post-processing enabled but no provider is selected");
            return PostProcessTranscriptionOutcome::Skipped;
        }
    };

    // Determine model: profile override > global setting
    let global_model = settings
        .post_process_models
        .get(&provider.id)
        .cloned()
        .unwrap_or_default();

    let model = match profile {
        Some(p) => {
            // Use profile override if set and non-empty, otherwise fall back to global
            p.llm_model_override
                .as_ref()
                .filter(|m| !m.trim().is_empty())
                .cloned()
                .unwrap_or(global_model)
        }
        None => global_model,
    };

    if model.trim().is_empty() {
        debug!(
            "Post-processing skipped because provider '{}' has no model configured",
            provider.id
        );
        return PostProcessTranscriptionOutcome::Skipped;
    }

    // Determine prompt: profile override > global selected prompt
    let prompt_template = match profile {
        Some(p)
            if p.llm_prompt_override
                .as_ref()
                .map_or(false, |s| !s.trim().is_empty()) =>
        {
            // Use profile's prompt override
            p.llm_prompt_override.clone().unwrap()
        }
        _ => {
            // Use global selected prompt
            let selected_prompt_id = match &settings.post_process_selected_prompt_id {
                Some(id) => id.clone(),
                None => {
                    debug!("Post-processing skipped because no prompt is selected");
                    return PostProcessTranscriptionOutcome::Skipped;
                }
            };

            match settings
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
            }
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

    let processed_prompt = apply_llm_template_vars(&prompt_template, template_context);
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

    // Build reasoning config from settings
    let reasoning_config = crate::llm_client::ReasoningConfig::new(
        settings.post_process_reasoning_enabled,
        settings.post_process_reasoning_budget,
    );

    // Send the chat completion request with optional reasoning
    match crate::llm_client::send_chat_completion_with_reasoning(
        &provider,
        api_key,
        &model,
        processed_prompt,
        reasoning_config,
    )
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

            // Strip invisible Unicode characters that some LLMs (e.g., Qwen) may insert
            let content = if settings.zero_width_filter_enabled {
                content
                    .replace('\u{200B}', "") // Zero-Width Space
                    .replace('\u{200C}', "") // Zero-Width Non-Joiner
                    .replace('\u{200D}', "") // Zero-Width Joiner
                    .replace('\u{FEFF}', "") // Byte Order Mark / Zero-Width No-Break Space
            } else {
                content
            };
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

fn show_ai_replace_error_overlay(app: &AppHandle, message: impl Into<String>) {
    let message = message.into();
    emit_ai_replace_error(app, message.clone());
    crate::plus_overlay_state::show_error_overlay_with_message(
        app,
        crate::plus_overlay_state::OverlayErrorCategory::Unknown,
        message,
    );
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

    // Capture the effective profile ID at recording start time.
    // This ensures transcription uses the profile that was active when recording started,
    // even if the user switches profiles mid-recording.
    let captured_profile_id =
        if binding_id == "transcribe" && settings.active_profile_id != "default" {
            // Main transcribe shortcut with an active profile - capture that profile ID
            Some(settings.active_profile_id.clone())
        } else if binding_id.starts_with("transcribe_profile_") {
            // Profile-specific shortcut - extract and capture the profile ID
            binding_id
                .strip_prefix("transcribe_")
                .map(|s| s.to_string())
        } else {
            // No profile to capture (ai_replace, send_to_extension, etc.)
            None
        };

    debug!(
        "start_recording_with_feedback: captured_profile_id={:?} for binding={}",
        captured_profile_id, binding_id
    );

    *state_guard = session_manager::SessionState::Recording {
        session: Arc::clone(&session),
        binding_id: binding_id.to_string(),
        captured_profile_id,
    };

    // Capture the active app context at recording start for prompt variables.
    capture_recording_app_context(binding_id);

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
        // Drop captured app context for failed recordings.
        let _ = take_recording_app_context(binding_id);

        // Recording failed - clean up
        // Take the session back and let it drop (which will clean up)
        let state = app.state::<ManagedSessionState>();
        let mut state_guard = state.lock().expect("Failed to lock session state");
        *state_guard = session_manager::SessionState::Idle;
        drop(state_guard);

        // Show microphone error overlay instead of just hiding
        crate::plus_overlay_state::show_mic_error_overlay(app);
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
///
/// The captured_profile_id parameter is the profile that was active when recording started.
/// This ensures transcription uses the correct profile even if the user switches profiles
/// mid-recording. If None, no profile is used (global settings apply).
async fn perform_transcription_for_profile(
    app: &AppHandle,
    samples: Vec<f32>,
    binding_id: Option<&str>,
    captured_profile_id: Option<String>,
) -> TranscriptionOutcome {
    let settings = get_settings(app);

    // Use the captured profile ID from recording start, not the current active_profile_id.
    // This ensures that if the user switches profiles mid-recording, we still use
    // the profile that was active when recording started.
    let profile = if let Some(profile_id) = &captured_profile_id {
        settings.transcription_profile(profile_id)
    } else {
        None
    };

    debug!(
        "perform_transcription_for_profile: binding_id={:?}, captured_profile_id={:?}, resolved_profile={:?}",
        binding_id,
        captured_profile_id,
        profile.as_ref().map(|p| &p.name)
    );

    if settings.transcription_provider == TranscriptionProvider::RemoteOpenAiCompatible {
        // Determine translate_to_english: use profile setting if available, otherwise global setting
        let translate_to_english = profile
            .as_ref()
            .map(|p| p.translate_to_english)
            .unwrap_or(settings.translate_to_english);

        // Determine language: use profile setting if available, otherwise global setting
        let language = profile
            .as_ref()
            .map(|p| p.language.clone())
            .unwrap_or_else(|| settings.selected_language.clone());

        // Log the request details
        if let Some(p) = &profile {
            log::info!(
                "Transcription using Remote STT with profile '{}' (lang={}, translate={}): base_url={}, model={}",
                p.name,
                language,
                translate_to_english,
                settings.remote_stt.base_url,
                settings.remote_stt.model_id
            );
        } else {
            log::info!(
                "Transcription using Remote STT: base_url={}, model={}, lang={}, translate={}",
                settings.remote_stt.base_url,
                settings.remote_stt.model_id,
                language,
                translate_to_english
            );
        }
        let remote_manager = app.state::<Arc<RemoteSttManager>>();
        let operation_id = remote_manager.start_operation();

        let prompt = crate::settings::resolve_stt_prompt(
            profile,
            &settings.transcription_prompts,
            &settings.remote_stt.model_id,
        );

        let result = remote_manager
            .transcribe(
                &settings.remote_stt,
                &samples,
                prompt,
                Some(language),
                translate_to_english,
            )
            .await
            .map(|text| {
                // Apply custom word corrections
                let corrected =
                    if settings.custom_words_enabled && !settings.custom_words.is_empty() {
                        apply_custom_words(
                            &text,
                            &settings.custom_words,
                            settings.word_correction_threshold,
                            settings.custom_words_ngram_enabled,
                        )
                    } else {
                        text
                    };
                // Apply filler word filter (if enabled)
                if settings.filler_word_filter_enabled {
                    crate::audio_toolkit::filter_transcription_output(&corrected)
                } else {
                    corrected
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
    } else if settings.transcription_provider == TranscriptionProvider::RemoteSoniox {
        // Determine language: use profile setting if available, otherwise global setting
        let language = profile
            .as_ref()
            .map(|p| p.language.clone())
            .unwrap_or_else(|| settings.selected_language.clone());

        #[cfg(target_os = "windows")]
        let api_key = crate::secure_keys::get_soniox_api_key();

        #[cfg(not(target_os = "windows"))]
        let api_key = String::new();

        if let Some(p) = &profile {
            log::info!(
                "Transcription using Soniox with profile '{}' (lang={}): model={}",
                p.name,
                language,
                settings.soniox_model
            );
        } else {
            log::info!(
                "Transcription using Soniox: model={}, lang={}",
                settings.soniox_model,
                language
            );
        }

        let soniox_manager = app.state::<Arc<SonioxSttManager>>();
        let operation_id = soniox_manager.start_operation();
        let should_stream_insert = binding_id
            .map(|id| id == "transcribe" || id.starts_with("transcribe_profile_"))
            .unwrap_or(false);

        let result = if should_stream_insert {
            let app_handle = app.clone();
            soniox_manager
                .transcribe_with_streaming_callback(
                    Some(operation_id),
                    &api_key,
                    &settings.soniox_model,
                    settings.soniox_timeout_seconds,
                    &samples,
                    Some(language.as_str()),
                    move |chunk| {
                        if chunk.is_empty() {
                            return Ok(());
                        }
                        let chunk = chunk.to_string();
                        let ah_for_call = app_handle.clone();
                        let ah_for_closure = ah_for_call.clone();
                        ah_for_call.run_on_main_thread(move || {
                            let _ =
                                crate::clipboard::paste_stream_chunk(chunk, ah_for_closure.clone());
                        })
                        .map_err(|e| anyhow::anyhow!("Failed to queue stream chunk paste: {}", e))
                    },
                )
                .await
        } else {
            soniox_manager
                .transcribe(
                    Some(operation_id),
                    &api_key,
                    &settings.soniox_model,
                    settings.soniox_timeout_seconds,
                    &samples,
                    Some(language.as_str()),
                )
                .await
        };

        let result = result.map(|text| {
            let corrected = if settings.custom_words_enabled && !settings.custom_words.is_empty() {
                apply_custom_words(
                    &text,
                    &settings.custom_words,
                    settings.word_correction_threshold,
                    settings.custom_words_ngram_enabled,
                )
            } else {
                text
            };
            if settings.filler_word_filter_enabled {
                crate::audio_toolkit::filter_transcription_output(&corrected)
            } else {
                corrected
            }
        });

        if soniox_manager.is_cancelled(operation_id) {
            debug!(
                "Soniox transcription operation {} was cancelled, discarding result",
                operation_id
            );
            return TranscriptionOutcome::Cancelled;
        }

        match result {
            Ok(text) => TranscriptionOutcome::Success(text),
            Err(err) => {
                let err_str = format!("{}", err);
                if soniox_manager.is_cancelled(operation_id)
                    || err_str.to_lowercase().contains("cancelled")
                {
                    return TranscriptionOutcome::Cancelled;
                }
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
            tm.transcribe_with_overrides(
                samples,
                Some(&p.language),
                Some(p.translate_to_english),
                // Use resolve_stt_prompt to respect stt_prompt_override_enabled flag
                crate::settings::resolve_stt_prompt(
                    Some(p),
                    &settings.transcription_prompts,
                    &settings.selected_model,
                ),
                settings.custom_words_enabled,
            )
        } else {
            log::info!(
                "Transcription using Local model: {}",
                settings.selected_model
            );
            tm.transcribe(samples, settings.custom_words_enabled)
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
/// Returns context captured at recording start on success, None if no active session.
///
/// IMPORTANT: After calling this, the caller MUST call exit_processing() when
/// the async work is complete (success or error).
fn prepare_stop_recording_with_options(
    app: &AppHandle,
    binding_id: &str,
    show_processing_overlay: bool,
) -> Option<StopRecordingContext> {
    // Take the session and transition to Processing state
    let state = app.state::<ManagedSessionState>();
    let mut state_guard = state.lock().expect("Failed to lock session state");

    let result = match &*state_guard {
        session_manager::SessionState::Recording {
            binding_id: current_binding_id,
            session,
            captured_profile_id,
        } if current_binding_id == binding_id => {
            let session = Arc::clone(session);
            let captured = captured_profile_id.clone();
            // Transition to Processing state
            *state_guard = session_manager::SessionState::Processing {
                binding_id: binding_id.to_string(),
            };
            Some((session, captured))
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

    if let Some((session, captured_profile_id)) = result {
        let current_app = take_recording_app_context(binding_id);

        // Explicitly finish the session to trigger cleanup
        // This unregisters the cancel shortcut exactly once
        session.finish();

        let settings = get_settings(app);

        change_tray_icon(app, TrayIconState::Transcribing);
        if show_processing_overlay {
            if settings.transcription_provider != TranscriptionProvider::Local {
                show_sending_overlay(app);
            } else {
                show_transcribing_overlay(app);
            }
        }

        let rm = app.state::<Arc<AudioRecordingManager>>();
        rm.remove_mute();

        play_feedback_sound(app, SoundType::Stop);
        Some(StopRecordingContext {
            captured_profile_id,
            current_app,
        })
    } else {
        None
    }
}

fn prepare_stop_recording(app: &AppHandle, binding_id: &str) -> Option<StopRecordingContext> {
    prepare_stop_recording_with_options(app, binding_id, true)
}

/// Asynchronously stops recording and performs transcription.
/// Handles errors by cleaning up the UI and returning None.
///
/// The captured_profile_id is the profile that was active when recording started,
/// ensuring transcription uses the correct profile even if the user switches mid-recording.
async fn get_transcription_or_cleanup(
    app: &AppHandle,
    binding_id: &str,
    captured_profile_id: Option<String>,
) -> Option<(String, Vec<f32>)> {
    let rm = Arc::clone(&app.state::<Arc<AudioRecordingManager>>());

    if let Some(samples) = rm.stop_recording(binding_id) {
        // Quick Tap Optimization: Only apply to AI Replace action
        let settings = get_settings(app);
        let is_ai_replace = binding_id.starts_with("ai_replace");
        let should_skip = is_ai_replace && {
            let threshold_samples =
                (settings.ai_replace_quick_tap_threshold_ms as f32 / 1000.0 * 16000.0) as usize;
            samples.len() < threshold_samples
        };

        if should_skip {
            debug!(
                "Quick tap detected for AI Replace ({} samples < {}), skipping transcription",
                samples.len(),
                (settings.ai_replace_quick_tap_threshold_ms as f32 / 1000.0 * 16000.0) as usize
            );
            return Some((String::new(), samples));
        }

        match perform_transcription_for_profile(
            app,
            samples.clone(),
            Some(binding_id),
            captured_profile_id,
        )
        .await
        {
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

fn resolve_profile_for_binding<'a>(
    settings: &'a AppSettings,
    binding_id: &str,
) -> Option<&'a TranscriptionProfile> {
    if binding_id == "transcribe" && settings.active_profile_id != "default" {
        return settings.transcription_profile(&settings.active_profile_id);
    }

    if binding_id.starts_with("transcribe_profile_") {
        if let Some(profile_id) = binding_id.strip_prefix("transcribe_") {
            return settings.transcription_profile(profile_id);
        }
    }

    None
}

fn should_use_soniox_live_streaming(settings: &AppSettings) -> bool {
    settings.transcription_provider == TranscriptionProvider::RemoteSoniox
        && settings.soniox_live_enabled
        && SonioxRealtimeManager::is_realtime_model(&settings.soniox_model)
}

fn resolve_soniox_hint_from_language(language: &str) -> Option<String> {
    let resolution = crate::language_resolver::resolve_requested_language_for_soniox(Some(language));

    match resolution.status {
        crate::language_resolver::SonioxLanguageResolutionStatus::Supported => {
            if let Some(normalized) = &resolution.normalized {
                debug!(
                    "Soniox language resolved: '{}' -> '{}'",
                    resolution.original.as_deref().unwrap_or(""),
                    normalized
                );
            }
        }
        crate::language_resolver::SonioxLanguageResolutionStatus::AutoOrEmpty => {
            debug!("Soniox language set to auto-detect (no language hint)");
        }
        crate::language_resolver::SonioxLanguageResolutionStatus::OsInputUnavailable => {
            warn!(
                "Soniox language fallback: OS input language could not be resolved, using auto-detect"
            );
        }
        crate::language_resolver::SonioxLanguageResolutionStatus::Unsupported => {
            warn!(
                "Soniox language fallback: unsupported language '{}' (normalized='{}'), using auto-detect",
                resolution.original.as_deref().unwrap_or(""),
                resolution.normalized.as_deref().unwrap_or("")
            );
        }
    }

    resolution.hint
}

fn build_soniox_realtime_options(settings: &AppSettings, language: &str) -> SonioxRealtimeOptions {
    let mut language_hints = if settings.soniox_use_profile_language_hint_only {
        Vec::new()
    } else {
        let normalized_hints =
            crate::language_resolver::normalize_soniox_hint_list(settings.soniox_language_hints.clone());
        if !normalized_hints.rejected.is_empty() {
            warn!(
                "Ignoring unsupported Soniox language hints: {}",
                normalized_hints.rejected.join(", ")
            );
        }
        normalized_hints.normalized
    };

    if settings.soniox_use_profile_language_hint_only || language_hints.is_empty() {
        if let Some(profile_hint) = resolve_soniox_hint_from_language(language) {
            language_hints.push(profile_hint);
        }
    }

    SonioxRealtimeOptions {
        language_hints,
        language_hints_strict: settings.soniox_language_hints_strict,
        enable_speaker_diarization: settings.soniox_enable_speaker_diarization,
        enable_language_identification: settings.soniox_enable_language_identification,
        enable_endpoint_detection: settings.soniox_enable_endpoint_detection,
        max_endpoint_delay_ms: settings.soniox_max_endpoint_delay_ms,
        keepalive_interval_seconds: settings.soniox_keepalive_interval_seconds,
    }
}

fn apply_soniox_output_filters(settings: &AppSettings, text: String) -> String {
    let corrected = if settings.custom_words_enabled && !settings.custom_words.is_empty() {
        apply_custom_words(
            &text,
            &settings.custom_words,
            settings.word_correction_threshold,
            settings.custom_words_ngram_enabled,
        )
    } else {
        text
    };

    if settings.filler_word_filter_enabled {
        crate::audio_toolkit::filter_transcription_output(&corrected)
    } else {
        corrected
    }
}

/// Applies Chinese conversion, LLM post-processing and saves to history.
///
/// `profile_id` is the ID of the active transcription profile (e.g., "default" or "profile_1234").
/// If a custom profile is used, its LLM settings will be applied for post-processing.
///
/// Text replacement order is controlled by `text_replacements_before_llm`:
/// - When true:  STT → Text Replacement → LLM → Output
/// - When false: STT → LLM → Text Replacement → Output (default)
async fn apply_post_processing_and_history(
    app: &AppHandle,
    transcription: String,
    samples: Vec<f32>,
    profile_id: Option<String>,
    current_app: &str,
) -> Option<String> {
    let settings = get_settings(app);
    let mut final_text = transcription.clone();
    let mut post_processed_text: Option<String> = None;
    let mut post_process_prompt: Option<String> = None;

    // Look up the profile if a custom profile is being used
    let profile = profile_id
        .as_ref()
        .filter(|id| *id != "default")
        .and_then(|id| settings.transcription_profile(id));

    // Helper closure for applying text replacements
    let apply_replacements = |text: &str| -> String {
        if settings.text_replacements_enabled && !settings.text_replacements.is_empty() {
            let original_len = text.len();
            let result =
                crate::settings::apply_text_replacements(text, &settings.text_replacements);
            if result.len() != original_len {
                debug!(
                    "Text replacements applied: {} chars -> {} chars",
                    original_len,
                    result.len()
                );
            }
            result
        } else {
            text.to_string()
        }
    };

    // Apply text replacements BEFORE LLM if configured
    if settings.text_replacements_before_llm {
        final_text = apply_replacements(&final_text);
    }

    if let Some(converted_text) = maybe_convert_chinese_variant(&settings, &final_text).await {
        final_text = converted_text;
    }

    let template_context = build_llm_template_context(
        app,
        &settings,
        profile,
        current_app,
        &final_text,
        "",
        "",
    );

    match maybe_post_process_transcription(app, &settings, profile, &template_context).await {
        PostProcessTranscriptionOutcome::Skipped => {
            if final_text != transcription {
                // Chinese conversion was applied but LLM post-processing was not.
                post_processed_text = Some(final_text.clone());
            }
        }
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

    // Apply text replacements AFTER LLM if NOT configured for before
    if !settings.text_replacements_before_llm {
        final_text = apply_replacements(&final_text);
    }

    // Keep recent transcript context per app for prompt variable ${short_prev_transcript}.
    // Use raw transcription (before post-processing) to avoid compounding LLM output.
    update_short_prev_transcript(&settings, current_app, &transcription);

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

fn build_extension_message(
    app: &AppHandle,
    settings: &AppSettings,
    instruction: &str,
    selection: &str,
    current_app: &str,
) -> String {
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
        let template_context = build_llm_template_context(
            app,
            settings,
            None,
            current_app,
            selection_trimmed,
            instruction_trimmed,
            selection_trimmed,
        );
        apply_llm_template_vars(user_template, &template_context)
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
    app: &AppHandle,
    settings: &AppSettings,
    selected_text: &str,
    instruction: &str,
    current_app: &str,
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

    let template_context = build_llm_template_context(
        app,
        settings,
        None,
        current_app,
        selected_text,
        instruction,
        selected_text,
    );

    let user_prompt = apply_llm_template_vars(&user_template, &template_context);

    debug!(
        "AI replace LLM request using provider '{}' (model: {})",
        provider.id, model
    );

    let api_key = settings.ai_replace_api_key(&provider.id);
    if api_key.trim().is_empty() {
        return Err(format!(
            "No API key configured for provider '{}'",
            provider.label
        ));
    }

    // Build reasoning config from settings
    let reasoning_config = crate::llm_client::ReasoningConfig::new(
        settings.ai_replace_reasoning_enabled,
        settings.ai_replace_reasoning_budget,
    );

    // Use the HTTP-based LLM client with optional reasoning
    match crate::llm_client::send_chat_completion_with_system_and_reasoning(
        &provider,
        api_key,
        &model,
        system_prompt,
        user_prompt,
        reasoning_config,
    )
    .await
    {
        Ok(Some(content)) => {
            if content.trim().is_empty() {
                return Err("LLM API response is empty".to_string());
            }
            // Strip invisible Unicode characters that some LLMs may insert
            let content = if settings.zero_width_filter_enabled {
                content
                    .replace('\u{200B}', "") // Zero-Width Space
                    .replace('\u{200C}', "") // Zero-Width Non-Joiner
                    .replace('\u{200D}', "") // Zero-Width Joiner
                    .replace('\u{FEFF}', "") // Byte Order Mark / Zero-Width No-Break Space
            } else {
                content
            };
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

        let settings = get_settings(app);
        let use_soniox_live = should_use_soniox_live_streaming(&settings);

        if use_soniox_live {
            let soniox_live_manager = Arc::clone(&app.state::<Arc<SonioxRealtimeManager>>());
            soniox_live_manager.cancel();
            let audio_manager = Arc::clone(&app.state::<Arc<AudioRecordingManager>>());
            audio_manager.set_stream_frame_callback(Arc::new(move |frame| {
                soniox_live_manager.push_audio_frame(frame);
            }));
        }

        if !start_recording_with_feedback(app, binding_id) {
            if use_soniox_live {
                let soniox_live_manager = Arc::clone(&app.state::<Arc<SonioxRealtimeManager>>());
                soniox_live_manager.cancel();
                let audio_manager = Arc::clone(&app.state::<Arc<AudioRecordingManager>>());
                audio_manager.clear_stream_frame_callback();
            }
            // Recording failed to start (e.g., system busy) - reset toggle state
            // so next press will try to start again instead of calling stop
            reset_toggle_state(app, binding_id);
            return;
        }

        if use_soniox_live {
            if let Err(e) = crate::clipboard::begin_streaming_paste_session(app) {
                warn!("Failed to begin streaming clipboard session: {}", e);
            }

            let profile = resolve_profile_for_binding(&settings, binding_id);
            let language = profile
                .as_ref()
                .map(|p| p.language.clone())
                .unwrap_or_else(|| settings.selected_language.clone());
            let options = build_soniox_realtime_options(&settings, &language);
            let model = settings.soniox_model.clone();
            let timeout_seconds = settings.soniox_timeout_seconds;
            let binding_id = binding_id.to_string();
            let app_handle = app.clone();
            let soniox_live_manager = Arc::clone(&app.state::<Arc<SonioxRealtimeManager>>());

            #[cfg(target_os = "windows")]
            let api_key = crate::secure_keys::get_soniox_api_key();
            #[cfg(not(target_os = "windows"))]
            let api_key = String::new();

            let chunk_callback: FinalChunkCallback = Arc::new({
                let ah_for_cb = app_handle.clone();
                move |chunk: String| {
                    if chunk.is_empty() {
                        return;
                    }
                    let ah_for_call = ah_for_cb.clone();
                    let ah_for_clip = ah_for_call.clone();
                    let _ = ah_for_call.run_on_main_thread(move || {
                        let _ =
                            crate::clipboard::paste_stream_chunk(chunk, ah_for_clip.clone());
                    });
                }
            });

            let start_result = soniox_live_manager.start_session(
                &binding_id,
                &api_key,
                &model,
                options,
                Some(chunk_callback),
            );

            if let Err(err) = start_result {
                let err_str = format!("{}", err);
                let _ = app_handle.emit("remote-stt-error", err_str.clone());
                crate::plus_overlay_state::handle_transcription_error(&app_handle, &err_str);
                let _ = crate::clipboard::end_streaming_paste_session(&app_handle);
                app_handle
                    .state::<Arc<AudioRecordingManager>>()
                    .clear_stream_frame_callback();
                // Cancel the recording and return to idle if session startup fails.
                crate::utils::cancel_current_operation(&app_handle);
            } else {
                debug!(
                    "Soniox live session started for binding '{}' (timeout={}s)",
                    binding_id, timeout_seconds
                );
            }
        }

        debug!(
            "TranscribeAction::start completed in {:?}",
            start_time.elapsed()
        );
    }

    fn stop(&self, app: &AppHandle, binding_id: &str, _shortcut_str: &str) {
        let settings = get_settings(app);
        let soniox_live_manager = Arc::clone(&app.state::<Arc<SonioxRealtimeManager>>());
        let use_soniox_live =
            should_use_soniox_live_streaming(&settings) && soniox_live_manager.has_active_session();

        if use_soniox_live {
            let stop_context = match prepare_stop_recording_with_options(app, binding_id, false) {
                Some(context) => context,
                None => return, // No active session - nothing to do
            };
            // Live mode already streamed text while recording.
            // On stop, show explicit finalizing state unless instant-stop is enabled.
            if settings.soniox_live_instant_stop {
                utils::hide_recording_overlay(app);
            } else {
                show_finalizing_overlay(app);
            }
            let profile_id_for_postprocess = stop_context.captured_profile_id.clone();
            let current_app = stop_context.current_app.clone();

            let ah = app.clone();
            let binding_id = binding_id.to_string();
            tauri::async_runtime::spawn(async move {
                let settings = get_settings(&ah);
                let rm = Arc::clone(&ah.state::<Arc<AudioRecordingManager>>());
                let samples = match rm.stop_recording(&binding_id) {
                    Some(samples) => samples,
                    None => {
                        if settings.soniox_live_instant_stop {
                            soniox_live_manager.cancel();
                        } else {
                            let _ = soniox_live_manager
                                .finish_session(settings.soniox_live_finalize_timeout_ms)
                                .await;
                        }
                        rm.clear_stream_frame_callback();
                        let _ = crate::clipboard::end_streaming_paste_session(&ah);
                        utils::hide_recording_overlay(&ah);
                        change_tray_icon(&ah, TrayIconState::Idle);
                        session_manager::exit_processing(&ah);
                        return;
                    }
                };
                rm.clear_stream_frame_callback();

                if settings.soniox_live_instant_stop {
                    soniox_live_manager.cancel();
                    let _ = crate::clipboard::end_streaming_paste_session(&ah);

                    let ah_clone = ah.clone();
                    let binding_id_clone = binding_id.clone();
                    ah.run_on_main_thread(move || {
                        let settings = get_settings(&ah_clone);
                        if settings.append_trailing_space {
                            let _ =
                                crate::clipboard::paste_stream_chunk(" ".to_string(), ah_clone.clone());
                        }
                        utils::hide_recording_overlay(&ah_clone);
                        change_tray_icon(&ah_clone, TrayIconState::Idle);
                        if let Ok(mut states) = ah_clone.state::<ManagedToggleState>().lock() {
                            states.active_toggles.insert(binding_id_clone, false);
                        }
                    })
                    .ok();

                    session_manager::exit_processing(&ah);
                    return;
                }

                let transcription_result = soniox_live_manager
                    .finish_session(settings.soniox_live_finalize_timeout_ms)
                    .await;
                let transcription = match transcription_result {
                    Ok(text) => apply_soniox_output_filters(&settings, text),
                    Err(err) => {
                        let err_str = format!("{}", err);
                        let _ = ah.emit("remote-stt-error", err_str.clone());
                        crate::plus_overlay_state::handle_transcription_error(&ah, &err_str);
                        let _ = crate::clipboard::end_streaming_paste_session(&ah);
                        session_manager::exit_processing(&ah);
                        return;
                    }
                };

                if transcription.is_empty() {
                    let _ = crate::clipboard::end_streaming_paste_session(&ah);
                    utils::hide_recording_overlay(&ah);
                    change_tray_icon(&ah, TrayIconState::Idle);
                    session_manager::exit_processing(&ah);
                    return;
                }

                let final_text = match apply_post_processing_and_history(
                    &ah,
                    transcription,
                    samples,
                    profile_id_for_postprocess,
                    &current_app,
                )
                .await
                {
                    Some(text) => text,
                    None => {
                        let _ = crate::clipboard::end_streaming_paste_session(&ah);
                        session_manager::exit_processing(&ah);
                        return;
                    }
                };

                let ah_clone = ah.clone();
                let binding_id_clone = binding_id.clone();
                ah.run_on_main_thread(move || {
                    // Soniox live mode already inserted text incrementally while chunks arrived.
                    // Only append final trailing space once if requested.
                    let settings = get_settings(&ah_clone);
                    if settings.append_trailing_space {
                        let _ =
                            crate::clipboard::paste_stream_chunk(" ".to_string(), ah_clone.clone());
                    }
                    if settings.clipboard_handling
                        == crate::settings::ClipboardHandling::CopyToClipboard
                    {
                        let text = if settings.append_trailing_space {
                            format!("{} ", final_text)
                        } else {
                            final_text
                        };
                        let _ = ah_clone.clipboard().write_text(text);
                    }

                    utils::hide_recording_overlay(&ah_clone);
                    change_tray_icon(&ah_clone, TrayIconState::Idle);
                    if let Ok(mut states) = ah_clone.state::<ManagedToggleState>().lock() {
                        states.active_toggles.insert(binding_id_clone, false);
                    }
                })
                .ok();

                if let Err(e) = crate::clipboard::end_streaming_paste_session(&ah) {
                    warn!("Failed to end streaming clipboard session: {}", e);
                }

                session_manager::exit_processing(&ah);
            });
            return;
        }

        let stop_context = match prepare_stop_recording(app, binding_id) {
            Some(context) => context,
            None => return, // No active session - nothing to do
        };
        let captured_profile_id = stop_context.captured_profile_id.clone();
        let current_app = stop_context.current_app.clone();

        let ah = app.clone();
        let binding_id = binding_id.to_string();

        tauri::async_runtime::spawn(async move {
            let is_soniox_provider =
                get_settings(&ah).transcription_provider == TranscriptionProvider::RemoteSoniox;
            if is_soniox_provider {
                if let Err(e) = crate::clipboard::begin_streaming_paste_session(&ah) {
                    warn!("Failed to begin streaming clipboard session: {}", e);
                }
            }
            let profile_id_for_postprocess = captured_profile_id.clone();
            let (transcription, samples) =
                match get_transcription_or_cleanup(&ah, &binding_id, captured_profile_id).await {
                    Some(res) => res,
                    None => {
                        if is_soniox_provider {
                            let _ = crate::clipboard::end_streaming_paste_session(&ah);
                        }
                        session_manager::exit_processing(&ah);
                        return;
                    }
                };

            if transcription.is_empty() {
                if is_soniox_provider {
                    let _ = crate::clipboard::end_streaming_paste_session(&ah);
                }
                utils::hide_recording_overlay(&ah);
                change_tray_icon(&ah, TrayIconState::Idle);
                session_manager::exit_processing(&ah);
                return;
            }

            let final_text = match apply_post_processing_and_history(
                &ah,
                transcription,
                samples,
                profile_id_for_postprocess,
                &current_app,
            )
            .await
            {
                Some(text) => text,
                None => {
                    if is_soniox_provider {
                        let _ = crate::clipboard::end_streaming_paste_session(&ah);
                    }
                    session_manager::exit_processing(&ah);
                    return;
                }
            };

            let ah_clone = ah.clone();
            let binding_id_clone = binding_id.clone();
            ah.run_on_main_thread(move || {
                if is_soniox_provider {
                    // Soniox live mode already inserted text incrementally while chunks arrived.
                    // Only append final trailing space once if requested.
                    let settings = get_settings(&ah_clone);
                    if settings.append_trailing_space {
                        let _ = crate::clipboard::paste_stream_chunk(" ".to_string(), ah_clone.clone());
                    }
                    if settings.clipboard_handling == crate::settings::ClipboardHandling::CopyToClipboard {
                        let text = if settings.append_trailing_space {
                            format!("{} ", final_text)
                        } else {
                            final_text
                        };
                        let _ = ah_clone.clipboard().write_text(text);
                    }
                } else {
                    let _ = utils::paste(final_text, ah_clone.clone());
                }
                utils::hide_recording_overlay(&ah_clone);
                change_tray_icon(&ah_clone, TrayIconState::Idle);
                // Clear toggle state now that transcription is complete
                if let Ok(mut states) = ah_clone.state::<ManagedToggleState>().lock() {
                    states.active_toggles.insert(binding_id_clone, false);
                }
            })
            .ok();

            if is_soniox_provider {
                if let Err(e) = crate::clipboard::end_streaming_paste_session(&ah) {
                    warn!("Failed to end streaming clipboard session: {}", e);
                }
            }

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

        if !start_recording_with_feedback(app, binding_id) {
            reset_toggle_state(app, binding_id);
        }

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
            let _ = take_recording_app_context(binding_id);
            return;
        }

        let stop_context = match prepare_stop_recording(app, binding_id) {
            Some(context) => context,
            None => return, // No active session - nothing to do
        };
        let current_app = stop_context.current_app;

        let ah = app.clone();
        let cm = Arc::clone(&app.state::<Arc<ConnectorManager>>());
        let binding_id = binding_id.to_string();

        tauri::async_runtime::spawn(async move {
            let (transcription, samples) =
                match get_transcription_or_cleanup(&ah, &binding_id, None).await {
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

            // Use default profile (None) for extension actions
            let final_text =
                match apply_post_processing_and_history(
                    &ah,
                    transcription,
                    samples,
                    None,
                    &current_app,
                )
                .await
                {
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

        if !start_recording_with_feedback(app, binding_id) {
            reset_toggle_state(app, binding_id);
        }

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
            let _ = take_recording_app_context(binding_id);
            return;
        }

        let stop_context = match prepare_stop_recording(app, binding_id) {
            Some(context) => context,
            None => return, // No active session - nothing to do
        };
        let current_app = stop_context.current_app;

        let ah = app.clone();
        let cm = Arc::clone(&app.state::<Arc<ConnectorManager>>());
        let binding_id = binding_id.to_string();

        tauri::async_runtime::spawn(async move {
            let (transcription, samples) =
                match get_transcription_or_cleanup(&ah, &binding_id, None).await {
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
                // Use default profile (None) for extension actions
                match apply_post_processing_and_history(
                    &ah,
                    transcription,
                    samples,
                    None,
                    &current_app,
                )
                .await
                {
                    Some(text) => text,
                    None => {
                        session_manager::exit_processing(&ah);
                        return;
                    }
                }
            };

            let selected_text = utils::capture_selection_text_copy(&ah).unwrap_or_default();
            let message = build_extension_message(
                &ah,
                &settings,
                &final_transcription,
                &selected_text,
                &current_app,
            );

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

        if !start_recording_with_feedback(app, binding_id) {
            reset_toggle_state(app, binding_id);
        }

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
            let _ = take_recording_app_context(binding_id);
            return;
        }

        if prepare_stop_recording(app, binding_id).is_none() {
            return; // No active session - nothing to do
        }

        let ah = app.clone();
        let cm = Arc::clone(&app.state::<Arc<ConnectorManager>>());
        let binding_id = binding_id.to_string();

        tauri::async_runtime::spawn(async move {
            let (voice_text, _) = match get_transcription_or_cleanup(&ah, &binding_id, None).await {
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

        if !start_recording_with_feedback(app, binding_id) {
            reset_toggle_state(app, binding_id);
        }

        debug!(
            "AiReplaceSelectionAction::start completed in {:?}",
            start_time.elapsed()
        );
    }

    fn stop(&self, app: &AppHandle, binding_id: &str, _shortcut_str: &str) {
        let stop_context = match prepare_stop_recording(app, binding_id) {
            Some(context) => context,
            None => return, // No active session - nothing to do
        };
        let current_app = stop_context.current_app;

        let ah = app.clone();
        let binding_id = binding_id.to_string();

        tauri::async_runtime::spawn(async move {
            let (transcription, _) =
                match get_transcription_or_cleanup(&ah, &binding_id, None).await {
                    Some(res) => res,
                    None => {
                        session_manager::exit_processing(&ah);
                        return;
                    }
                };

            let settings = get_settings(&ah);

            if transcription.trim().is_empty() {
                if !settings.ai_replace_allow_quick_tap {
                    show_ai_replace_error_overlay(&ah, "No instruction captured.");
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
                show_ai_replace_error_overlay(&ah, "Could not capture selection.");
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

            match ai_replace_with_llm(
                &ah,
                &settings,
                &selected_text,
                &transcription,
                &current_app,
            )
            .await
            {
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
                    let restore_text = selected_text.clone();
                    ah.run_on_main_thread(move || {
                        if let Err(e) = utils::paste(output, ah_clone.clone()) {
                            error!("Failed to paste AI Replace output: {}", e);
                            if !restore_text.is_empty() {
                                if let Err(restore_err) =
                                    utils::paste(restore_text.clone(), ah_clone.clone())
                                {
                                    error!(
                                        "Failed to restore original selection after paste error: {}",
                                        restore_err
                                    );
                                }
                            }
                            show_ai_replace_error_overlay(
                                &ah_clone,
                                "AI replace failed while applying result.",
                            );
                            return;
                        }
                        utils::hide_recording_overlay(&ah_clone);
                        change_tray_icon(&ah_clone, TrayIconState::Idle);
                    })
                    .ok();
                }
                Err(err_message) => {
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

                    if !selected_text.is_empty() {
                        let ah_restore = ah.clone();
                        let restore_text = selected_text.clone();
                        ah.run_on_main_thread(move || {
                            if let Err(e) = utils::paste(restore_text, ah_restore.clone()) {
                                error!("Failed to restore original selection: {}", e);
                            }
                        })
                        .ok();
                    }

                    show_ai_replace_error_overlay(&ah, err_message);
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

    fn is_instant(&self) -> bool {
        true
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

            match hm.get_latest_entry() {
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

    fn is_instant(&self) -> bool {
        true
    }
}

// ============================================================================
// Cycle Transcription Profile Action
// ============================================================================

impl ShortcutAction for CycleProfileAction {
    fn start(&self, app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {
        debug!("CycleProfileAction::start called");

        // Prevent profile switching during active recording or processing
        // to avoid overlay conflicts and user confusion
        {
            let state = app.state::<ManagedSessionState>();
            let state_guard = state.lock().expect("Failed to lock session state");

            if !matches!(*state_guard, session_manager::SessionState::Idle) {
                debug!("CycleProfileAction: System busy (recording or processing), ignoring");
                return;
            }
        }

        // Call the cycle function directly (it handles overlay and events)
        match crate::shortcut::cycle_to_next_profile(app.clone()) {
            Ok(next_id) => {
                debug!("Cycled to profile: {}", next_id);
            }
            Err(e) => {
                warn!("Failed to cycle profile: {}", e);
            }
        }
    }

    fn stop(&self, _app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {
        // Cycling is instant, nothing to do on stop
    }

    fn is_instant(&self) -> bool {
        true
    }
}

#[cfg(target_os = "windows")]
impl ShortcutAction for SpawnVoiceButtonAction {
    fn start(&self, app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {
        let ah = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) =
                crate::commands::voice_activation_button::spawn_voice_activation_button_window(ah)
                    .await
            {
                warn!("Failed to spawn voice activation button window: {}", e);
            }
        });
    }

    fn stop(&self, _app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {
        // Instant action: nothing to do on key release.
    }

    fn is_instant(&self) -> bool {
        true
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
    /// The PowerShell script/command to execute
    pub command: String,
    /// What the user said (for context)
    pub spoken_text: String,
    /// Whether this came from LLM (true) or predefined match (false)
    pub from_llm: bool,
    // ==================== Execution Options ====================
    /// Silent execution (hidden window, non-interactive)
    pub silent: bool,
    /// Skip profile loading (-NoProfile flag)
    pub no_profile: bool,
    /// Use PowerShell 7 (pwsh) instead of Windows PowerShell 5.1
    pub use_pwsh: bool,
    /// Execution policy (None = system default)
    pub execution_policy: Option<String>,
    /// Working directory (None = current directory)
    pub working_directory: Option<String>,
    // ==================== Auto-run Options ====================
    /// Whether to auto-run after countdown (only for predefined commands)
    pub auto_run: bool,
    /// Countdown seconds before auto-run
    pub auto_run_seconds: u32,
}

/// Configuration for the hybrid fuzzy matching algorithm
#[derive(Debug, Clone)]
pub struct FuzzyMatchConfig {
    /// Whether to use Levenshtein distance for character-level matching
    pub use_levenshtein: bool,
    /// Per-word Levenshtein threshold (0.0-1.0, lower = more tolerant of typos)
    pub levenshtein_threshold: f64,
    /// Whether to use phonetic (Soundex) matching
    pub use_phonetic: bool,
    /// Phonetic match boost multiplier (0.0-1.0)
    pub phonetic_boost: f64,
    /// Word similarity threshold - minimum score for a word pair to be considered matching
    pub word_similarity_threshold: f64,
}

impl Default for FuzzyMatchConfig {
    fn default() -> Self {
        Self {
            use_levenshtein: true,
            levenshtein_threshold: 0.3,
            use_phonetic: true,
            phonetic_boost: 0.5,
            word_similarity_threshold: 0.7,
        }
    }
}

impl FuzzyMatchConfig {
    /// Create config from AppSettings
    pub fn from_settings(settings: &AppSettings) -> Self {
        Self {
            use_levenshtein: settings.voice_command_use_levenshtein,
            levenshtein_threshold: settings.voice_command_levenshtein_threshold,
            use_phonetic: settings.voice_command_use_phonetic,
            phonetic_boost: settings.voice_command_phonetic_boost,
            word_similarity_threshold: settings.voice_command_word_similarity_threshold,
        }
    }
}

/// Computes word-level similarity using hybrid algorithm:
/// - Levenshtein distance for typo tolerance
/// - Soundex phonetic matching for pronunciation similarity
/// Returns a value between 0.0 and 1.0.
fn compute_word_similarity(word_a: &str, word_b: &str, config: &FuzzyMatchConfig) -> f64 {
    // Exact match
    if word_a == word_b {
        return 1.0;
    }

    let mut score: f64 = 0.0;

    // Levenshtein (character-level edit distance)
    if config.use_levenshtein {
        let lev_score = normalized_levenshtein(word_a, word_b);
        // Only accept if above threshold (1.0 - threshold gives minimum required similarity)
        if lev_score >= (1.0 - config.levenshtein_threshold) {
            score = score.max(lev_score);
        }
    }

    // Phonetic matching (Soundex)
    if config.use_phonetic && soundex(word_a, word_b) {
        // Phonetic match - boost the score
        let phonetic_score = config.word_similarity_threshold + config.phonetic_boost * (1.0 - config.word_similarity_threshold);
        score = score.max(phonetic_score.min(1.0));
    }

    score
}

/// Computes a similarity score between two strings using a hybrid word-matching approach.
/// For each word in the transcription, finds the best matching word in the trigger phrase.
/// Returns a value between 0.0 and 1.0.
fn compute_similarity(a: &str, b: &str, config: &FuzzyMatchConfig) -> f64 {
    let a_lower = a.to_lowercase();
    let b_lower = b.to_lowercase();

    // Exact match
    if a_lower == b_lower {
        return 1.0;
    }

    let a_words: Vec<&str> = a_lower.split_whitespace().collect();
    let b_words: Vec<&str> = b_lower.split_whitespace().collect();

    if a_words.is_empty() || b_words.is_empty() {
        return 0.0;
    }

    // For each word in 'a', find the best matching word in 'b'
    let mut total_score: f64 = 0.0;
    let mut matched_count = 0;

    for a_word in &a_words {
        let mut best_match_score: f64 = 0.0;

        for b_word in &b_words {
            let word_score = compute_word_similarity(a_word, b_word, config);
            if word_score >= config.word_similarity_threshold {
                best_match_score = best_match_score.max(word_score);
            }
        }

        if best_match_score >= config.word_similarity_threshold {
            total_score += best_match_score;
            matched_count += 1;
        }
    }

    // Score is based on:
    // 1. How many words from 'a' matched something in 'b' (coverage)
    // 2. How well they matched (quality)
    // 3. Length ratio to penalize very different lengths
    let coverage = matched_count as f64 / a_words.len() as f64;
    let quality = if matched_count > 0 {
        total_score / matched_count as f64
    } else {
        0.0
    };

    // Length penalty - favor similar length phrases
    let len_ratio = (a_words.len().min(b_words.len()) as f64)
        / (a_words.len().max(b_words.len()) as f64);

    // Final score combines coverage, quality, and length similarity
    // Coverage is most important (70%), quality matters (20%), length is a tiebreaker (10%)
    coverage * 0.7 + quality * coverage * 0.2 + len_ratio * 0.1
}

/// Format ExecutionPolicy for frontend display.
fn format_execution_policy(policy: crate::settings::ExecutionPolicy) -> Option<String> {
    use crate::settings::ExecutionPolicy;
    match policy {
        ExecutionPolicy::Default => None,
        ExecutionPolicy::Bypass => Some("bypass".to_string()),
        ExecutionPolicy::Unrestricted => Some("unrestricted".to_string()),
        ExecutionPolicy::RemoteSigned => Some("remote_signed".to_string()),
    }
}

/// Finds the best matching predefined command for the given transcription.
/// Returns (command, similarity_score) if a match above threshold is found.
pub fn find_matching_command(
    transcription: &str,
    commands: &[crate::settings::VoiceCommand],
    default_threshold: f64,
    config: &FuzzyMatchConfig,
) -> Option<(crate::settings::VoiceCommand, f64)> {
    let mut best_match: Option<(crate::settings::VoiceCommand, f64)> = None;

    for cmd in commands.iter().filter(|c| c.enabled) {
        let threshold = if cmd.similarity_threshold > 0.0 {
            cmd.similarity_threshold
        } else {
            default_threshold
        };

        let score = compute_similarity(transcription, &cmd.trigger_phrase, config);

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

    // Use Voice Command specific provider (falls back to post-processing if not set)
    let provider = settings
        .active_voice_command_provider()
        .cloned()
        .ok_or_else(|| "No LLM provider configured for Voice Commands".to_string())?;

    // Use Voice Command specific model, fallback to post-processing model
    let model = settings
        .voice_command_models
        .get(&provider.id)
        .cloned()
        .or_else(|| settings.post_process_models.get(&provider.id).cloned())
        .unwrap_or_default();

    if model.trim().is_empty() {
        return Err(format!(
            "No model configured for provider '{}'",
            provider.label
        ));
    }

    let current_app = crate::active_app::get_frontmost_app_name().unwrap_or_default();
    let template_context = build_llm_template_context(
        app,
        &settings,
        None,
        &current_app,
        spoken_text,
        spoken_text,
        "",
    );
    let system_prompt = apply_llm_template_vars(&settings.voice_command_system_prompt, &template_context);
    let user_prompt = spoken_text.to_string();

    // Use post-processing key only when voice command provider is set to
    // "same as post-processing" (voice_command_provider_id = None).
    let use_post_process_key =
        settings.voice_command_provider_id.as_deref() != Some(provider.id.as_str());

    #[cfg(target_os = "windows")]
    let api_key = if use_post_process_key {
        crate::secure_keys::get_post_process_api_key(&provider.id)
    } else {
        crate::secure_keys::get_voice_command_api_key(&provider.id).unwrap_or_default()
    };

    #[cfg(not(target_os = "windows"))]
    let api_key = if use_post_process_key {
        settings
            .post_process_api_keys
            .get(&provider.id)
            .cloned()
            .unwrap_or_default()
    } else {
        settings
            .voice_command_api_keys
            .get(&provider.id)
            .cloned()
            .unwrap_or_default()
    };

    // Build reasoning config from settings
    let reasoning_config = crate::llm_client::ReasoningConfig::new(
        settings.voice_command_reasoning_enabled,
        settings.voice_command_reasoning_budget,
    );

    match crate::llm_client::send_chat_completion_with_system_and_reasoning(
        &provider,
        api_key,
        &model,
        system_prompt,
        user_prompt,
        reasoning_config,
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

        if !start_recording_with_feedback(app, binding_id) {
            reset_toggle_state(app, binding_id);
        }

        debug!(
            "VoiceCommandAction::start completed in {:?}",
            start_time.elapsed()
        );
    }

    fn stop(&self, app: &AppHandle, binding_id: &str, _shortcut_str: &str) {
        if prepare_stop_recording(app, binding_id).is_none() {
            return;
        }

        let ah = app.clone();
        let binding_id = binding_id.to_string();

        tauri::async_runtime::spawn(async move {
            let (transcription, _) =
                match get_transcription_or_cleanup(&ah, &binding_id, None).await {
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
            let fuzzy_config = FuzzyMatchConfig::from_settings(&settings);

            // Step 1: Try to match against predefined commands
            if let Some((matched_cmd, score)) = find_matching_command(
                &transcription,
                &settings.voice_commands,
                settings.voice_command_default_threshold,
                &fuzzy_config,
            ) {
                debug!(
                    "Voice command matched: '{}' -> '{}' (score: {:.2})",
                    matched_cmd.trigger_phrase, matched_cmd.script, score
                );

                // Resolve execution options for this command
                let resolved = matched_cmd.resolve_execution_options(&settings.voice_command_defaults);

                // Show confirmation overlay
                crate::overlay::show_command_confirm_overlay(
                    &ah,
                    CommandConfirmPayload {
                        command: matched_cmd.script.clone(),
                        spoken_text: transcription.clone(),
                        from_llm: false,
                        silent: resolved.silent,
                        no_profile: resolved.no_profile,
                        use_pwsh: resolved.use_pwsh,
                        execution_policy: format_execution_policy(resolved.execution_policy),
                        working_directory: resolved.working_directory,
                        auto_run: settings.voice_command_auto_run,
                        auto_run_seconds: settings.voice_command_auto_run_seconds,
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

                        // LLM fallback uses global defaults
                        let resolved = settings.voice_command_defaults.to_resolved_options();

                        // Show confirmation overlay
                        crate::overlay::show_command_confirm_overlay(
                            &ah,
                            CommandConfirmPayload {
                                command: suggested_command,
                                spoken_text: transcription,
                                from_llm: true,
                                silent: resolved.silent,
                                no_profile: resolved.no_profile,
                                use_pwsh: resolved.use_pwsh,
                                execution_policy: format_execution_policy(resolved.execution_policy),
                                working_directory: resolved.working_directory,
                                auto_run: false, // Never auto-run LLM-generated commands
                                auto_run_seconds: 0,
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
    map.insert(
        "cycle_profile".to_string(),
        Arc::new(CycleProfileAction) as Arc<dyn ShortcutAction>,
    );
    #[cfg(target_os = "windows")]
    map.insert(
        "spawn_button".to_string(),
        Arc::new(SpawnVoiceButtonAction) as Arc<dyn ShortcutAction>,
    );
    #[cfg(target_os = "windows")]
    map.insert(
        "voice_command".to_string(),
        Arc::new(VoiceCommandAction) as Arc<dyn ShortcutAction>,
    );
    map
});
