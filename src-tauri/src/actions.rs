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
use crate::soniox_stream_processor::SonioxStreamProcessor;
use crate::settings::{
    apply_output_whitespace_policy_for_settings, get_settings, AppSettings, TranscriptionProvider,
    APPLE_INTELLIGENCE_PROVIDER_ID,
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

#[derive(Clone, Debug)]
struct StopRecordingContext {
    captured_profile_id: Option<String>,
    current_app: String,
    recording_settings: AppSettings,
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
type SharedSonioxStreamProcessor = Arc<Mutex<SonioxStreamProcessor>>;
static SONIOX_STREAM_PROCESSORS: Lazy<Mutex<HashMap<String, SharedSonioxStreamProcessor>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
const RECORDING_SAMPLE_RATE_HZ: f32 = 16_000.0;

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

fn quick_tap_threshold_samples(threshold_ms: u32) -> usize {
    ((threshold_ms.max(1) as f32 / 1000.0) * RECORDING_SAMPLE_RATE_HZ) as usize
}

fn register_soniox_stream_processor(
    binding_id: &str,
    settings: &AppSettings,
) -> SharedSonioxStreamProcessor {
    let processor = Arc::new(Mutex::new(SonioxStreamProcessor::from_settings(settings)));
    if let Ok(mut processors) = SONIOX_STREAM_PROCESSORS.lock() {
        processors.insert(binding_id.to_string(), Arc::clone(&processor));
    }
    processor
}

fn take_soniox_stream_processor(binding_id: &str) -> Option<SharedSonioxStreamProcessor> {
    SONIOX_STREAM_PROCESSORS
        .lock()
        .ok()
        .and_then(|mut processors| processors.remove(binding_id))
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
    let supports_language_selection = mm
        .get_model_info(&settings.selected_model)
        .map(|m| {
            matches!(
                m.engine_type,
                EngineType::Whisper | EngineType::SenseVoice
            )
        })
        .unwrap_or(false);

    if supports_language_selection && !requested.trim().is_empty() {
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
/// If `force_manual` is true, the enable flag gates are bypassed.
async fn maybe_post_process_transcription(
    app: &AppHandle,
    settings: &AppSettings,
    profile: Option<&TranscriptionProfile>,
    template_context: &LlmTemplateContext,
    force_manual: bool,
) -> PostProcessTranscriptionOutcome {
    if !force_manual && settings.transcription_provider == TranscriptionProvider::RemoteSoniox {
        debug!("Skipping post-processing for Soniox streaming transcription");
        return PostProcessTranscriptionOutcome::Skipped;
    }

    if !force_manual {
        // Determine if post-processing is enabled based on profile or global setting.
        let is_enabled = match profile {
            Some(p) => p.llm_post_process_enabled,
            None => settings.post_process_enabled,
        };

        if !is_enabled {
            return PostProcessTranscriptionOutcome::Skipped;
        }
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
    requested_language: &str,
    transcription: &str,
) -> Option<String> {
    // Check if language is set to Simplified or Traditional Chinese
    let is_simplified = requested_language == "zh-Hans";
    let is_traditional = requested_language == "zh-Hant";

    if !is_simplified && !is_traditional {
        debug!(
            "requested language is not Simplified or Traditional Chinese; skipping translation"
        );
        return None;
    }

    debug!(
        "Starting Chinese translation using OpenCC for language: {}",
        requested_language
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
        captured_settings: settings.clone(),
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

/// Detailed result for transcription fetch + cleanup decisions.
/// Used when callers need to decide whether UI cleanup is still required.
enum TranscriptionFetchOutcome {
    Success((String, Vec<f32>)),
    Cancelled,
    ErrorOverlayShown,
    ErrorNoOverlay,
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
    settings: &AppSettings,
) -> TranscriptionOutcome {
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

    let preview_output_only_enabled = is_preview_output_only_profile(settings, profile);

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
        let soniox_context = crate::settings::resolve_soniox_context(profile, &settings);
        let should_stream_insert = !preview_output_only_enabled
            && binding_id
            .map(|id| id == "transcribe" || id.starts_with("transcribe_profile_"))
            .unwrap_or(false);

        let result = if should_stream_insert {
            let app_handle = app.clone();
            let stream_processor = Arc::new(Mutex::new(SonioxStreamProcessor::from_settings(&settings)));
            let stream_processor_for_callback = Arc::clone(&stream_processor);
            let streamed_result = soniox_manager
                .transcribe_with_streaming_callback(
                    Some(operation_id),
                    &api_key,
                    &settings.soniox_model,
                    settings.soniox_timeout_seconds,
                    &samples,
                    Some(language.as_str()),
                    soniox_context.clone(),
                    move |chunk| {
                        if chunk.is_empty() {
                            return Ok(());
                        }
                        let delta = match stream_processor_for_callback.lock() {
                            Ok(mut processor) => processor.push_chunk(chunk),
                            Err(_) => {
                                return Err(anyhow::anyhow!("Failed to lock Soniox stream processor"));
                            }
                        };
                        if delta.is_empty() {
                            return Ok(());
                        }
                        let ah_for_call = app_handle.clone();
                        let ah_for_closure = ah_for_call.clone();
                        ah_for_call.run_on_main_thread(move || {
                            let _ =
                                crate::clipboard::paste_stream_chunk(delta, ah_for_closure.clone());
                        })
                        .map_err(|e| anyhow::anyhow!("Failed to queue stream chunk paste: {}", e))
                    },
                )
                .await;

            match streamed_result {
                Ok(text) => {
                    match stream_processor.lock() {
                        Ok(mut processor) => {
                            let tail_delta = processor.flush();
                            drop(processor);

                            if tail_delta.is_empty() {
                                Ok(text)
                            } else {
                                let ah_for_call = app.clone();
                                let ah_for_closure = ah_for_call.clone();
                                match ah_for_call.run_on_main_thread(move || {
                                    let _ = crate::clipboard::paste_stream_chunk(
                                        tail_delta,
                                        ah_for_closure.clone(),
                                    );
                                }) {
                                    Ok(_) => Ok(text),
                                    Err(err) => Err(anyhow::anyhow!(
                                        "Failed to queue stream tail paste: {}",
                                        err
                                    )),
                                }
                            }
                        }
                        Err(_) => Err(anyhow::anyhow!("Failed to lock Soniox stream processor")),
                    }
                }
                Err(err) => Err(err),
            }
        } else {
            soniox_manager
                .transcribe(
                    Some(operation_id),
                    &api_key,
                    &settings.soniox_model,
                    settings.soniox_timeout_seconds,
                    &samples,
                    Some(language.as_str()),
                    soniox_context,
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
            captured_settings,
        } if current_binding_id == binding_id => {
            let session = Arc::clone(session);
            let captured = captured_profile_id.clone();
            let recording_settings = captured_settings.clone();
            // Transition to Processing state
            *state_guard = session_manager::SessionState::Processing {
                binding_id: binding_id.to_string(),
            };
            Some((session, captured, recording_settings))
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

    if let Some((session, captured_profile_id, recording_settings)) = result {
        let current_app = take_recording_app_context(binding_id);

        // Explicitly finish the session to trigger cleanup
        // This unregisters the cancel shortcut exactly once
        session.finish();

        change_tray_icon(app, TrayIconState::Transcribing);
        if show_processing_overlay {
            if recording_settings.transcription_provider != TranscriptionProvider::Local {
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
            recording_settings,
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
    recording_settings: AppSettings,
) -> Option<(String, Vec<f32>)> {
    match get_transcription_or_cleanup_detailed(
        app,
        binding_id,
        captured_profile_id,
        recording_settings,
    )
    .await
    {
        TranscriptionFetchOutcome::Success(result) => Some(result),
        TranscriptionFetchOutcome::Cancelled
        | TranscriptionFetchOutcome::ErrorOverlayShown
        | TranscriptionFetchOutcome::ErrorNoOverlay => None,
    }
}

async fn get_transcription_or_cleanup_detailed(
    app: &AppHandle,
    binding_id: &str,
    captured_profile_id: Option<String>,
    recording_settings: AppSettings,
) -> TranscriptionFetchOutcome {
    let rm = Arc::clone(&app.state::<Arc<AudioRecordingManager>>());
    let soniox_live_manager = Arc::clone(&app.state::<Arc<SonioxRealtimeManager>>());
    let has_live_session = soniox_live_manager.has_active_session();

    if let Some(samples) = rm.stop_recording(binding_id) {
        if has_live_session {
            rm.clear_stream_frame_callback();
        }

        // Quick Tap Optimization: Only apply to AI Replace action
        let is_ai_replace = binding_id.starts_with("ai_replace");
        let quick_tap_threshold_samples =
            quick_tap_threshold_samples(recording_settings.ai_replace_quick_tap_threshold_ms);
        let should_skip = is_ai_replace && {
            samples.len() < quick_tap_threshold_samples
        };

        if should_skip {
            debug!(
                "Quick tap detected for AI Replace ({} samples < {}), skipping transcription",
                samples.len(),
                quick_tap_threshold_samples
            );
            if has_live_session {
                soniox_live_manager.cancel();
            }
            return TranscriptionFetchOutcome::Success((String::new(), samples));
        }

        // Soniox live consolidation: finalize session and return accumulated text
        if has_live_session {
            match soniox_live_manager
                .finish_session(recording_settings.soniox_live_finalize_timeout_ms)
                .await
            {
                Ok(text) => {
                    let filtered = apply_soniox_output_filters(&recording_settings, text);
                    return TranscriptionFetchOutcome::Success((filtered, samples));
                }
                Err(err) => {
                    let err_str = format!("{}", err);
                    let _ = app.emit("remote-stt-error", err_str.clone());
                    crate::plus_overlay_state::handle_transcription_error(app, &err_str);
                    return TranscriptionFetchOutcome::ErrorOverlayShown;
                }
            }
        }

        if is_transcribe_binding_id(binding_id)
            && recording_settings.text_replacement_decapitalize_after_edit_key_enabled
        {
            crate::text_replacement_decapitalize::begin_standard_post_recording_monitor(
                recording_settings.text_replacement_decapitalize_standard_post_recording_monitor_ms,
            );
        }

        match perform_transcription_for_profile(
            app,
            samples.clone(),
            Some(binding_id),
            captured_profile_id,
            &recording_settings,
        )
        .await
        {
            TranscriptionOutcome::Success(text) => TranscriptionFetchOutcome::Success((text, samples)),
            TranscriptionOutcome::Cancelled => TranscriptionFetchOutcome::Cancelled,
            TranscriptionOutcome::Error {
                shown_in_overlay, ..
            } => {
                if !shown_in_overlay {
                    utils::hide_recording_overlay(app);
                    change_tray_icon(app, TrayIconState::Idle);
                    return TranscriptionFetchOutcome::ErrorNoOverlay;
                }
                TranscriptionFetchOutcome::ErrorOverlayShown
            }
        }
    } else {
        if has_live_session {
            rm.clear_stream_frame_callback();
            soniox_live_manager.cancel();
        }
        debug!("No samples retrieved from recording stop");
        utils::hide_recording_overlay(app);
        change_tray_icon(app, TrayIconState::Idle);
        TranscriptionFetchOutcome::ErrorNoOverlay
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

fn is_preview_output_only_profile(
    settings: &AppSettings,
    profile: Option<&TranscriptionProfile>,
) -> bool {
    profile
        .map(|p| p.preview_output_only_enabled)
        .unwrap_or(settings.preview_output_only_enabled)
}

fn is_preview_output_only_for_captured_profile(
    settings: &AppSettings,
    captured_profile_id: Option<&String>,
) -> bool {
    captured_profile_id
        .and_then(|profile_id| settings.transcription_profile(profile_id))
        .map(|profile| profile.preview_output_only_enabled)
        .unwrap_or(settings.preview_output_only_enabled)
}

fn update_preview_text_for_output_mode(app: &AppHandle, text: &str) {
    let mut next_final = crate::managers::preview_output_mode::recording_prefix_text();
    next_final.push_str(text);
    crate::overlay::emit_soniox_live_preview_update(app, &next_final, "");
}

fn current_preview_buffer_text() -> String {
    let state = crate::overlay::get_soniox_live_preview_state();
    format!("{}{}", state.final_text, state.interim_text)
}

fn current_preview_final_text() -> String {
    crate::overlay::get_soniox_live_preview_state().final_text
}

async fn paste_preview_buffer_to_target(app: &AppHandle, text: String) -> Result<(), String> {
    if text.trim().is_empty() {
        return Ok(());
    }

    // Avoid pasting back into the preview window itself after an action click.
    crate::overlay::hide_soniox_live_preview_window(app);
    tokio::time::sleep(Duration::from_millis(90)).await;

    let ah_for_call = app.clone();
    let ah_for_paste = app.clone();
    ah_for_call
        .run_on_main_thread(move || {
            if let Err(err) = utils::paste(text, ah_for_paste.clone()) {
                warn!("Preview paste failed: {}", err);
            }
        })
        .map_err(|err| format!("Failed to dispatch paste operation: {}", err))?;

    Ok(())
}

async fn finalize_preview_workflow_after_stop(app: &AppHandle, text: String) -> Result<(), String> {
    if !text.trim().is_empty() {
        paste_preview_buffer_to_target(app, text).await?;
    }
    close_preview_output_mode_workflow(app, true);
    Ok(())
}

fn transcribe_action_for_binding(binding_id: &str) -> Option<Arc<dyn ShortcutAction>> {
    ACTION_MAP.get(binding_id).cloned().or_else(|| {
        if binding_id.starts_with("transcribe_") {
            ACTION_MAP.get("transcribe").cloned()
        } else {
            None
        }
    })
}

fn stop_transcribe_binding_from_preview(app: &AppHandle, binding_id: &str) -> Result<(), String> {
    let action = transcribe_action_for_binding(binding_id).ok_or_else(|| {
        format!(
            "No transcription action is registered for binding '{}'",
            binding_id
        )
    })?;
    action.stop(app, binding_id, "preview_output_mode");
    Ok(())
}

fn start_transcribe_binding_from_preview(app: &AppHandle, binding_id: &str) -> Result<(), String> {
    let action = transcribe_action_for_binding(binding_id).ok_or_else(|| {
        format!(
            "No transcription action is registered for binding '{}'",
            binding_id
        )
    })?;
    action.start(app, binding_id, "preview_output_mode");
    Ok(())
}

fn is_recording_for_binding(app: &AppHandle, binding_id: &str) -> bool {
    let state = app.state::<ManagedSessionState>();
    let guard = match state.lock() {
        Ok(guard) => guard,
        Err(_) => return false,
    };
    matches!(
        &*guard,
        session_manager::SessionState::Recording {
            binding_id: current_binding_id,
            ..
        } if current_binding_id == binding_id
    )
}

fn use_push_to_talk_for_transcribe_binding(settings: &AppSettings, binding_id: &str) -> bool {
    if binding_id == "transcribe" {
        if settings.active_profile_id == "default" {
            settings.push_to_talk
        } else {
            settings
                .transcription_profile(&settings.active_profile_id)
                .map(|p| p.push_to_talk)
                .unwrap_or(settings.push_to_talk)
        }
    } else if binding_id.starts_with("transcribe_") {
        settings
            .transcription_profile_by_binding(binding_id)
            .map(|p| p.push_to_talk)
            .unwrap_or(settings.push_to_talk)
    } else {
        settings.push_to_talk
    }
}

async fn wait_for_session_idle(app: &AppHandle, timeout: Duration) -> bool {
    let start = Instant::now();
    loop {
        let is_idle = {
            let state = app.state::<ManagedSessionState>();
            let idle = match state.lock() {
                Ok(guard) => matches!(&*guard, session_manager::SessionState::Idle),
                Err(_) => false,
            };
            idle
        };

        if is_idle {
            return true;
        }

        if start.elapsed() >= timeout {
            return false;
        }

        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

fn close_preview_output_mode_workflow(app: &AppHandle, clear_text: bool) {
    crate::managers::preview_output_mode::deactivate_session(app);
    crate::overlay::end_soniox_live_preview_session();
    if clear_text {
        crate::overlay::reset_soniox_live_preview(app);
    }
    crate::overlay::hide_soniox_live_preview_window(app);
}

fn is_transcribe_binding_id(binding_id: &str) -> bool {
    binding_id == "transcribe" || binding_id.starts_with("transcribe_")
}

fn should_use_soniox_live_streaming(settings: &AppSettings) -> bool {
    settings.transcription_provider == TranscriptionProvider::RemoteSoniox
        && settings.soniox_live_enabled
        && SonioxRealtimeManager::is_realtime_model(&settings.soniox_model)
}

fn should_use_soniox_live_for_recording(app: &AppHandle, binding_id: &str) -> bool {
    let soniox_live_manager = Arc::clone(&app.state::<Arc<SonioxRealtimeManager>>());
    if !soniox_live_manager.has_active_session() {
        return false;
    }

    let state = app.state::<ManagedSessionState>();
    let state_guard = state.lock().expect("Failed to lock session state");
    match &*state_guard {
        session_manager::SessionState::Recording {
            binding_id: current_binding_id,
            captured_settings,
            ..
        } if current_binding_id == binding_id => should_use_soniox_live_streaming(captured_settings),
        _ => false,
    }
}

/// Sets up Soniox live streaming for an action: installs the audio callback,
/// cancels any previous session, and starts a new accumulation-only session.
/// Call AFTER start_recording_with_feedback() succeeds — push_audio_frame()
/// buffers into pending_audio, so no frames are lost before start_session()
/// flushes them.  Returns Err on session start failure.
fn setup_and_start_soniox_live(
    app: &AppHandle,
    settings: &AppSettings,
    binding_id: &str,
) -> Result<(), String> {
    // Install audio callback (frames buffer in pending_audio until session is ready)
    let soniox_live_manager = Arc::clone(&app.state::<Arc<SonioxRealtimeManager>>());
    soniox_live_manager.cancel();
    let audio_manager = Arc::clone(&app.state::<Arc<AudioRecordingManager>>());
    audio_manager.set_stream_frame_callback(Arc::new(move |frame| {
        soniox_live_manager.push_audio_frame(frame);
    }));

    // Start session (flushes buffered audio)
    let profile = resolve_profile_for_binding(settings, binding_id);
    let language = profile
        .as_ref()
        .map(|p| p.language.clone())
        .unwrap_or_else(|| settings.selected_language.clone());
    let options = build_soniox_realtime_options(settings, &language, profile);
    let soniox_live_manager = Arc::clone(&app.state::<Arc<SonioxRealtimeManager>>());
    #[cfg(target_os = "windows")]
    let api_key = crate::secure_keys::get_soniox_api_key();
    #[cfg(not(target_os = "windows"))]
    let api_key = String::new();
    soniox_live_manager
        .start_session(binding_id, &api_key, &settings.soniox_model, options, None)
        .map_err(|e| {
            // Clean up callback on failure
            app.state::<Arc<AudioRecordingManager>>()
                .clear_stream_frame_callback();
            format!("{}", e)
        })
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

fn build_soniox_realtime_options(
    settings: &AppSettings,
    language: &str,
    profile: Option<&TranscriptionProfile>,
) -> SonioxRealtimeOptions {
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
        context: crate::settings::resolve_soniox_context(profile, settings),
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StreamTrailingAdjustment {
    None,
    AppendSpaces(usize),
    RemoveCharacters(usize),
}

fn count_trailing_whitespace_chars(text: &str) -> usize {
    text.chars()
        .rev()
        .take_while(|ch| ch.is_whitespace())
        .count()
}

fn resolve_stream_trailing_adjustment(
    settings: &AppSettings,
    original_text: &str,
) -> StreamTrailingAdjustment {
    let adjusted = apply_output_whitespace_policy_for_settings(original_text, settings);
    let original_trailing_count = count_trailing_whitespace_chars(original_text);
    let adjusted_trailing_count = count_trailing_whitespace_chars(&adjusted);

    if adjusted_trailing_count > original_trailing_count {
        StreamTrailingAdjustment::AppendSpaces(adjusted_trailing_count - original_trailing_count)
    } else if original_trailing_count > adjusted_trailing_count {
        StreamTrailingAdjustment::RemoveCharacters(original_trailing_count - adjusted_trailing_count)
    } else {
        StreamTrailingAdjustment::None
    }
}

fn apply_stream_trailing_adjustment(app: &AppHandle, adjustment: StreamTrailingAdjustment) {
    match adjustment {
        StreamTrailingAdjustment::None => {}
        StreamTrailingAdjustment::AppendSpaces(count) => {
            if count > 0 {
                let _ = crate::clipboard::paste_stream_chunk(" ".repeat(count), app.clone());
            }
        }
        StreamTrailingAdjustment::RemoveCharacters(count) => {
            if count > 0 {
                let _ = crate::clipboard::delete_last_stream_characters(app.clone(), count);
            }
        }
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
    settings: &AppSettings,
    transcription: String,
    samples: Vec<f32>,
    profile_id: Option<String>,
    current_app: &str,
) -> Option<String> {
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

    let requested_language = profile
        .map(|p| p.language.as_str())
        .unwrap_or(settings.selected_language.as_str());
    if let Some(converted_text) = maybe_convert_chinese_variant(requested_language, &final_text).await
    {
        final_text = converted_text;
    }

    let template_context = build_llm_template_context(
        app,
        settings,
        profile,
        current_app,
        &final_text,
        "",
        "",
    );

    match maybe_post_process_transcription(app, settings, profile, &template_context, false).await {
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

    // If the user recently edited text manually (for example with Backspace),
    // lower the first alphabetic uppercase character in the next matching output.
    final_text =
        crate::text_replacement_decapitalize::maybe_decapitalize_next_chunk_standard(&final_text);

    final_text = apply_output_whitespace_policy_for_settings(&final_text, settings);

    // Keep recent transcript context per app for prompt variable ${short_prev_transcript}.
    // Use raw transcription (before post-processing) to avoid compounding LLM output.
    update_short_prev_transcript(settings, current_app, &transcription);

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
        let profile = resolve_profile_for_binding(&settings, binding_id);
        let preview_output_only_enabled = is_preview_output_only_profile(&settings, profile);

        if !start_recording_with_feedback(app, binding_id) {
            // Recording failed to start (e.g., system busy) - reset toggle state
            // so next press will try to start again instead of calling stop
            reset_toggle_state(app, binding_id);
            return;
        }

        if preview_output_only_enabled {
            let was_active_for_binding =
                crate::managers::preview_output_mode::is_active_for_binding(binding_id);
            let mut recording_prefix = crate::overlay::get_soniox_live_preview_state().final_text;
            if !was_active_for_binding {
                crate::overlay::begin_soniox_live_preview_session();
                crate::overlay::reset_soniox_live_preview(app);
                recording_prefix.clear();
            }
            crate::managers::preview_output_mode::activate_session(
                app,
                binding_id.to_string(),
                profile.map(|p| p.id.clone()),
                use_soniox_live,
                recording_prefix,
            );
            crate::overlay::show_soniox_live_preview_window(app);
        }

        if use_soniox_live {
            // Install audio callback — frames buffer in pending_audio until session is ready.
            // This is safe to do after recording starts because start_session() flushes
            // the pending buffer.
            let soniox_live_manager = Arc::clone(&app.state::<Arc<SonioxRealtimeManager>>());
            soniox_live_manager.cancel();
            let audio_manager = Arc::clone(&app.state::<Arc<AudioRecordingManager>>());
            audio_manager.set_stream_frame_callback(Arc::new(move |frame| {
                soniox_live_manager.push_audio_frame(frame);
            }));
            if !preview_output_only_enabled {
                if let Err(e) = crate::clipboard::begin_streaming_paste_session(app) {
                    warn!("Failed to begin streaming clipboard session: {}", e);
                }
            }

            let language = profile
                .as_ref()
                .map(|p| p.language.clone())
                .unwrap_or_else(|| settings.selected_language.clone());
            let options = build_soniox_realtime_options(&settings, &language, profile);
            let model = settings.soniox_model.clone();
            let timeout_seconds = settings.soniox_timeout_seconds;
            let binding_id = binding_id.to_string();
            let app_handle = app.clone();
            let soniox_live_manager = Arc::clone(&app.state::<Arc<SonioxRealtimeManager>>());
            let _ = take_soniox_stream_processor(&binding_id);
            let stream_processor = if preview_output_only_enabled {
                None
            } else {
                Some(register_soniox_stream_processor(&binding_id, &settings))
            };

            #[cfg(target_os = "windows")]
            let api_key = crate::secure_keys::get_soniox_api_key();
            #[cfg(not(target_os = "windows"))]
            let api_key = String::new();

            let chunk_callback = stream_processor.map(|stream_processor| {
                Arc::new({
                    let ah_for_cb = app_handle.clone();
                    move |chunk: String| {
                        if chunk.is_empty() {
                            return;
                        }
                        let delta = match stream_processor.lock() {
                            Ok(mut processor) => processor.push_chunk(&chunk),
                            Err(_) => {
                                warn!("Failed to lock Soniox stream processor");
                                String::new()
                            }
                        };
                        if delta.is_empty() {
                            return;
                        }
                        let ah_for_call = ah_for_cb.clone();
                        let ah_for_clip = ah_for_call.clone();
                        let _ = ah_for_call.run_on_main_thread(move || {
                            let _ =
                                crate::clipboard::paste_stream_chunk(delta, ah_for_clip.clone());
                        });
                    }
                }) as FinalChunkCallback
            });

            let start_result = soniox_live_manager.start_session(
                &binding_id,
                &api_key,
                &model,
                options,
                chunk_callback,
            );

            if let Err(err) = start_result {
                let _ = take_soniox_stream_processor(&binding_id);
                let err_str = format!("{}", err);
                let _ = app_handle.emit("remote-stt-error", err_str.clone());
                crate::plus_overlay_state::handle_transcription_error(&app_handle, &err_str);
                if !preview_output_only_enabled {
                    let _ = crate::clipboard::end_streaming_paste_session(&app_handle);
                }
                app_handle
                    .state::<Arc<AudioRecordingManager>>()
                    .clear_stream_frame_callback();
                if preview_output_only_enabled {
                    crate::managers::preview_output_mode::deactivate_session(&app_handle);
                    crate::overlay::end_soniox_live_preview_session();
                    crate::overlay::hide_soniox_live_preview_window(&app_handle);
                    crate::overlay::reset_soniox_live_preview(&app_handle);
                }
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

    fn stop(&self, app: &AppHandle, binding_id: &str, shortcut_str: &str) {
        let soniox_live_manager = Arc::clone(&app.state::<Arc<SonioxRealtimeManager>>());
        let use_soniox_live = should_use_soniox_live_for_recording(app, binding_id);
        let invoked_from_preview_action = shortcut_str == "preview_output_mode";

        if use_soniox_live {
            let stop_context = match prepare_stop_recording_with_options(app, binding_id, false) {
                Some(context) => context,
                None => {
                    let _ = take_soniox_stream_processor(binding_id);
                    return; // No active session - nothing to do
                }
            };
            let recording_settings = stop_context.recording_settings.clone();
            let preview_output_only_enabled = is_preview_output_only_for_captured_profile(
                &recording_settings,
                stop_context.captured_profile_id.as_ref(),
            );
            if preview_output_only_enabled {
                crate::managers::preview_output_mode::set_recording(app, false);
                crate::managers::preview_output_mode::set_error(app, None);
            }
            // Live mode already streamed text while recording.
            // On stop, show explicit finalizing state unless instant-stop is enabled.
            if recording_settings.soniox_live_instant_stop && !preview_output_only_enabled {
                utils::hide_recording_overlay(app);
            } else {
                show_finalizing_overlay(app);
            }
            let profile_id_for_postprocess = stop_context.captured_profile_id.clone();
            let current_app = stop_context.current_app.clone();

            let ah = app.clone();
            let binding_id = binding_id.to_string();
            let preview_output_only_enabled = preview_output_only_enabled;
            let invoked_from_preview_action = invoked_from_preview_action;
            tauri::async_runtime::spawn(async move {
                let stream_processor = take_soniox_stream_processor(&binding_id);
                let rm = Arc::clone(&ah.state::<Arc<AudioRecordingManager>>());
                let samples = match rm.stop_recording(&binding_id) {
                    Some(samples) => samples,
                    None => {
                        if recording_settings.soniox_live_instant_stop {
                            soniox_live_manager.cancel();
                        } else {
                            let _ = soniox_live_manager
                                .finish_session(recording_settings.soniox_live_finalize_timeout_ms)
                                .await;
                        }
                        rm.clear_stream_frame_callback();
                        if !preview_output_only_enabled {
                            let _ = crate::clipboard::end_streaming_paste_session(&ah);
                        } else if !invoked_from_preview_action {
                            close_preview_output_mode_workflow(&ah, true);
                        }
                        utils::hide_recording_overlay(&ah);
                        change_tray_icon(&ah, TrayIconState::Idle);
                        session_manager::exit_processing(&ah);
                        return;
                    }
                };
                rm.clear_stream_frame_callback();

                if recording_settings.soniox_live_instant_stop && !preview_output_only_enabled {
                    soniox_live_manager.cancel();
                    if !preview_output_only_enabled {
                        let _ = crate::clipboard::end_streaming_paste_session(&ah);
                    }

                    let ah_clone = ah.clone();
                    let binding_id_clone = binding_id.clone();
                    ah.run_on_main_thread(move || {
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
                    .finish_session(recording_settings.soniox_live_finalize_timeout_ms)
                    .await;
                let transcription = match transcription_result {
                    Ok(text) => apply_soniox_output_filters(&recording_settings, text),
                    Err(err) => {
                        let err_str = format!("{}", err);
                        let _ = ah.emit("remote-stt-error", err_str.clone());
                        crate::plus_overlay_state::handle_transcription_error(&ah, &err_str);
                        if !preview_output_only_enabled {
                            let _ = crate::clipboard::end_streaming_paste_session(&ah);
                        }
                        if preview_output_only_enabled {
                            crate::managers::preview_output_mode::set_error(
                                &ah,
                                Some(err_str.clone()),
                            );
                        }
                        session_manager::exit_processing(&ah);
                        return;
                    }
                };

                if let Some(processor) = stream_processor.as_ref() {
                    let tail_delta = match processor.lock() {
                        Ok(mut processor) => processor.flush(),
                        Err(_) => {
                            warn!("Failed to lock Soniox stream processor");
                            String::new()
                        }
                    };
                    if !tail_delta.is_empty() {
                        let ah_for_call = ah.clone();
                        let ah_for_clip = ah_for_call.clone();
                        if let Err(err) = ah_for_call.run_on_main_thread(move || {
                            let _ = crate::clipboard::paste_stream_chunk(
                                tail_delta,
                                ah_for_clip.clone(),
                            );
                        }) {
                            warn!("Failed to queue Soniox stream tail paste: {}", err);
                        }
                    }
                }

                if transcription.is_empty() {
                    if !preview_output_only_enabled {
                        let _ = crate::clipboard::end_streaming_paste_session(&ah);
                    } else if !invoked_from_preview_action {
                        let text_to_insert =
                            crate::managers::preview_output_mode::recording_prefix_text();
                        if let Err(err) =
                            finalize_preview_workflow_after_stop(&ah, text_to_insert).await
                        {
                            crate::managers::preview_output_mode::set_error(&ah, Some(err));
                        }
                    }
                    utils::hide_recording_overlay(&ah);
                    change_tray_icon(&ah, TrayIconState::Idle);
                    session_manager::exit_processing(&ah);
                    return;
                }

                let stream_trailing_adjustment =
                    resolve_stream_trailing_adjustment(&recording_settings, &transcription);
                let copy_to_clipboard =
                    recording_settings.clipboard_handling
                        == crate::settings::ClipboardHandling::CopyToClipboard;

                let final_text = match apply_post_processing_and_history(
                    &ah,
                    &recording_settings,
                    transcription,
                    samples,
                    profile_id_for_postprocess,
                    &current_app,
                )
                .await
                {
                    Some(text) => text,
                    None => {
                        if !preview_output_only_enabled {
                            let _ = crate::clipboard::end_streaming_paste_session(&ah);
                        } else if !invoked_from_preview_action {
                            close_preview_output_mode_workflow(&ah, true);
                        }
                        session_manager::exit_processing(&ah);
                        return;
                    }
                };

                let ah_clone = ah.clone();
                let binding_id_clone = binding_id.clone();
                let final_text_for_ui = final_text.clone();
                let final_text_for_insert =
                    if preview_output_only_enabled && !invoked_from_preview_action {
                    let mut combined_text = crate::managers::preview_output_mode::recording_prefix_text();
                    combined_text.push_str(&final_text);
                    Some(combined_text)
                } else {
                    None
                };
                ah.run_on_main_thread(move || {
                    if !preview_output_only_enabled {
                        // Soniox live mode already inserted text incrementally while chunks arrived.
                        // Apply only boundary-level trailing adjustment at finalization.
                        apply_stream_trailing_adjustment(&ah_clone, stream_trailing_adjustment);
                        if copy_to_clipboard {
                            let _ = ah_clone.clipboard().write_text(final_text_for_ui.clone());
                        }
                    }

                    utils::hide_recording_overlay(&ah_clone);
                    change_tray_icon(&ah_clone, TrayIconState::Idle);
                    if let Ok(mut states) = ah_clone.state::<ManagedToggleState>().lock() {
                        states.active_toggles.insert(binding_id_clone, false);
                    }
                })
                .ok();

                if preview_output_only_enabled {
                    if let Some(text_to_insert) = final_text_for_insert {
                        if let Err(err) =
                            finalize_preview_workflow_after_stop(&ah, text_to_insert).await
                        {
                            crate::managers::preview_output_mode::set_error(&ah, Some(err));
                        }
                    } else if invoked_from_preview_action {
                        update_preview_text_for_output_mode(&ah, &final_text);
                    }
                }

                if !preview_output_only_enabled {
                    if let Err(e) = crate::clipboard::end_streaming_paste_session(&ah) {
                        warn!("Failed to end streaming clipboard session: {}", e);
                    }
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
        let recording_settings = stop_context.recording_settings.clone();
        let preview_output_only_enabled =
            is_preview_output_only_for_captured_profile(&recording_settings, captured_profile_id.as_ref());
        if preview_output_only_enabled {
            crate::managers::preview_output_mode::set_recording(app, false);
            crate::managers::preview_output_mode::set_error(app, None);
        }

        let ah = app.clone();
        let binding_id = binding_id.to_string();
        let preview_output_only_enabled = preview_output_only_enabled;
        let invoked_from_preview_action = invoked_from_preview_action;

        tauri::async_runtime::spawn(async move {
            let is_soniox_provider =
                recording_settings.transcription_provider == TranscriptionProvider::RemoteSoniox;
            if is_soniox_provider && !preview_output_only_enabled {
                if let Err(e) = crate::clipboard::begin_streaming_paste_session(&ah) {
                    warn!("Failed to begin streaming clipboard session: {}", e);
                }
            }
            let profile_id_for_postprocess = captured_profile_id.clone();
            let (transcription, samples) =
                match get_transcription_or_cleanup(
                    &ah,
                    &binding_id,
                    captured_profile_id,
                    recording_settings.clone(),
                )
                .await
                {
                    Some(res) => res,
                    None => {
                        if is_soniox_provider && !preview_output_only_enabled {
                            let _ = crate::clipboard::end_streaming_paste_session(&ah);
                        } else if preview_output_only_enabled && !invoked_from_preview_action {
                            close_preview_output_mode_workflow(&ah, true);
                        }
                        session_manager::exit_processing(&ah);
                        return;
                    }
                };

            if transcription.is_empty() {
                if is_soniox_provider && !preview_output_only_enabled {
                    let _ = crate::clipboard::end_streaming_paste_session(&ah);
                } else if preview_output_only_enabled && !invoked_from_preview_action {
                    let text_to_insert =
                        crate::managers::preview_output_mode::recording_prefix_text();
                    if let Err(err) =
                        finalize_preview_workflow_after_stop(&ah, text_to_insert).await
                    {
                        crate::managers::preview_output_mode::set_error(&ah, Some(err));
                    }
                }
                utils::hide_recording_overlay(&ah);
                change_tray_icon(&ah, TrayIconState::Idle);
                session_manager::exit_processing(&ah);
                return;
            }

            let stream_trailing_adjustment = if is_soniox_provider {
                resolve_stream_trailing_adjustment(&recording_settings, &transcription)
            } else {
                StreamTrailingAdjustment::None
            };
            let copy_to_clipboard = if is_soniox_provider {
                recording_settings.clipboard_handling
                    == crate::settings::ClipboardHandling::CopyToClipboard
            } else {
                false
            };

            let final_text = match apply_post_processing_and_history(
                &ah,
                &recording_settings,
                transcription,
                samples,
                profile_id_for_postprocess,
                &current_app,
            )
            .await
            {
                Some(text) => text,
                None => {
                    if is_soniox_provider && !preview_output_only_enabled {
                        let _ = crate::clipboard::end_streaming_paste_session(&ah);
                    } else if preview_output_only_enabled && !invoked_from_preview_action {
                        close_preview_output_mode_workflow(&ah, true);
                    }
                    session_manager::exit_processing(&ah);
                    return;
                }
            };

            let ah_clone = ah.clone();
            let binding_id_clone = binding_id.clone();
            let final_text_for_ui = final_text.clone();
            let final_text_for_insert =
                if preview_output_only_enabled && !invoked_from_preview_action {
                let mut combined_text = crate::managers::preview_output_mode::recording_prefix_text();
                combined_text.push_str(&final_text);
                Some(combined_text)
            } else {
                None
            };
            ah.run_on_main_thread(move || {
                if is_soniox_provider && !preview_output_only_enabled {
                    // Soniox path already inserted text incrementally while chunks arrived.
                    // Apply only boundary-level trailing adjustment at finalization.
                    apply_stream_trailing_adjustment(&ah_clone, stream_trailing_adjustment);
                    if copy_to_clipboard {
                        let _ = ah_clone.clipboard().write_text(final_text_for_ui.clone());
                    }
                } else if !preview_output_only_enabled {
                    let _ = utils::paste(final_text_for_ui.clone(), ah_clone.clone());
                }
                utils::hide_recording_overlay(&ah_clone);
                change_tray_icon(&ah_clone, TrayIconState::Idle);
                // Clear toggle state now that transcription is complete
                if let Ok(mut states) = ah_clone.state::<ManagedToggleState>().lock() {
                    states.active_toggles.insert(binding_id_clone, false);
                }
            })
            .ok();

            if preview_output_only_enabled {
                if let Some(text_to_insert) = final_text_for_insert {
                    if let Err(err) = finalize_preview_workflow_after_stop(&ah, text_to_insert).await
                    {
                        crate::managers::preview_output_mode::set_error(&ah, Some(err));
                    }
                } else if invoked_from_preview_action {
                    update_preview_text_for_output_mode(&ah, &final_text);
                }
            }

            if is_soniox_provider && !preview_output_only_enabled {
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

        let settings = get_settings(app);
        let use_soniox_live = should_use_soniox_live_streaming(&settings);

        if !start_recording_with_feedback(app, binding_id) {
            reset_toggle_state(app, binding_id);
            return;
        }

        if use_soniox_live {
            if let Err(err) = setup_and_start_soniox_live(app, &settings, binding_id) {
                let _ = app.emit("remote-stt-error", err.clone());
                crate::plus_overlay_state::handle_transcription_error(app, &err);
                crate::utils::cancel_current_operation(app);
                return;
            }
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
        let StopRecordingContext {
            current_app,
            recording_settings,
            ..
        } = stop_context;

        let ah = app.clone();
        let cm = Arc::clone(&app.state::<Arc<ConnectorManager>>());
        let binding_id = binding_id.to_string();

        tauri::async_runtime::spawn(async move {
            let (transcription, samples) =
                match get_transcription_or_cleanup(
                    &ah,
                    &binding_id,
                    None,
                    recording_settings.clone(),
                )
                .await
                {
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
                    &recording_settings,
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

        let settings = get_settings(app);
        let use_soniox_live = should_use_soniox_live_streaming(&settings);

        if !start_recording_with_feedback(app, binding_id) {
            reset_toggle_state(app, binding_id);
            return;
        }

        if use_soniox_live {
            if let Err(err) = setup_and_start_soniox_live(app, &settings, binding_id) {
                let _ = app.emit("remote-stt-error", err.clone());
                crate::plus_overlay_state::handle_transcription_error(app, &err);
                crate::utils::cancel_current_operation(app);
                return;
            }
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
        let StopRecordingContext {
            current_app,
            recording_settings,
            ..
        } = stop_context;

        let ah = app.clone();
        let cm = Arc::clone(&app.state::<Arc<ConnectorManager>>());
        let binding_id = binding_id.to_string();

        tauri::async_runtime::spawn(async move {
            let (transcription, samples) =
                match get_transcription_or_cleanup(
                    &ah,
                    &binding_id,
                    None,
                    recording_settings.clone(),
                )
                .await
                {
                    Some(res) => res,
                    None => {
                        session_manager::exit_processing(&ah);
                        return;
                    }
                };

            let final_transcription = if transcription.trim().is_empty() {
                if !recording_settings.send_to_extension_with_selection_allow_no_voice {
                    utils::hide_recording_overlay(&ah);
                    change_tray_icon(&ah, TrayIconState::Idle);
                    session_manager::exit_processing(&ah);
                    return;
                }
                let quick_tap_threshold_samples = quick_tap_threshold_samples(
                    recording_settings.send_to_extension_with_selection_quick_tap_threshold_ms,
                );
                if samples.len() >= quick_tap_threshold_samples {
                    debug!(
                        "Ignoring no-voice SendToExtensionWithSelection ({} samples >= quick tap threshold {})",
                        samples.len(),
                        quick_tap_threshold_samples
                    );
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
                    &recording_settings,
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
                &recording_settings,
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

        let settings = get_settings(app);
        let use_soniox_live = should_use_soniox_live_streaming(&settings);

        if !start_recording_with_feedback(app, binding_id) {
            reset_toggle_state(app, binding_id);
            return;
        }

        if use_soniox_live {
            if let Err(err) = setup_and_start_soniox_live(app, &settings, binding_id) {
                let _ = app.emit("remote-stt-error", err.clone());
                crate::plus_overlay_state::handle_transcription_error(app, &err);
                crate::utils::cancel_current_operation(app);
                return;
            }
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

        let stop_context = match prepare_stop_recording(app, binding_id) {
            Some(context) => context,
            None => return, // No active session - nothing to do
        };
        let recording_settings = stop_context.recording_settings;

        let ah = app.clone();
        let cm = Arc::clone(&app.state::<Arc<ConnectorManager>>());
        let binding_id = binding_id.to_string();

        tauri::async_runtime::spawn(async move {
            let (voice_text, samples) = match get_transcription_or_cleanup(
                &ah,
                &binding_id,
                None,
                recording_settings.clone(),
            )
            .await
            {
                Some(res) => res,
                None => {
                    session_manager::exit_processing(&ah);
                    return;
                }
            };

            let final_voice_text = if voice_text.trim().is_empty() {
                if !recording_settings.screenshot_allow_no_voice {
                    emit_screenshot_error(
                        &ah,
                        "No voice instruction captured. Enable Allow Quick Tap to send screenshot without voice.",
                    );
                    utils::hide_recording_overlay(&ah);
                    change_tray_icon(&ah, TrayIconState::Idle);
                    session_manager::exit_processing(&ah);
                    return;
                }

                let quick_tap_threshold_samples =
                    quick_tap_threshold_samples(recording_settings.screenshot_quick_tap_threshold_ms);
                if samples.len() >= quick_tap_threshold_samples {
                    debug!(
                        "Ignoring no-voice screenshot send ({} samples >= quick tap threshold {})",
                        samples.len(),
                        quick_tap_threshold_samples
                    );
                    emit_screenshot_error(
                        &ah,
                        "Quick Tap threshold exceeded. Speak an instruction or tap faster.",
                    );
                    utils::hide_recording_overlay(&ah);
                    change_tray_icon(&ah, TrayIconState::Idle);
                    session_manager::exit_processing(&ah);
                    return;
                }

                recording_settings.screenshot_no_voice_default_prompt.clone()
            } else {
                voice_text
            };

            // Hide overlay immediately after transcription (avoid capturing it in screenshots)
            utils::hide_recording_overlay_immediately(&ah);
            change_tray_icon(&ah, TrayIconState::Idle);

            if recording_settings.screenshot_capture_method
                == crate::settings::ScreenshotCaptureMethod::Native
            {
                // Native region capture (Windows only)
                #[cfg(target_os = "windows")]
                {
                    use crate::region_capture::{open_region_picker, RegionCaptureResult};

                    match open_region_picker(&ah, recording_settings.native_region_capture_mode).await
                    {
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
            let screenshot_folder =
                PathBuf::from(expand_env_vars(&recording_settings.screenshot_folder));
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
                collect_existing_images(
                    &screenshot_folder,
                    recording_settings.screenshot_include_subfolders,
                );
            let start_time = std::time::SystemTime::now();

            // Launch screenshot tool
            let capture_command = recording_settings.screenshot_capture_command.clone();
            if !capture_command.trim().is_empty() {
                #[cfg(target_os = "windows")]
                let _ = std::process::Command::new("powershell")
                    .args(["-NoProfile", "-Command", &capture_command])
                    .spawn();
            }

            // Wait for screenshot
            let timeout = recording_settings.screenshot_timeout_seconds as u64;
            match watch_for_new_image(
                screenshot_folder,
                timeout,
                recording_settings.screenshot_include_subfolders,
                existing_files,
                start_time,
                !recording_settings.screenshot_require_recent, // Fallback if requirement is disabled
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

        let settings = get_settings(app);
        let use_soniox_live = should_use_soniox_live_streaming(&settings);

        if !start_recording_with_feedback(app, binding_id) {
            reset_toggle_state(app, binding_id);
            return;
        }

        if use_soniox_live {
            if let Err(err) = setup_and_start_soniox_live(app, &settings, binding_id) {
                let _ = app.emit("remote-stt-error", err.clone());
                crate::plus_overlay_state::handle_transcription_error(app, &err);
                crate::utils::cancel_current_operation(app);
                return;
            }
        }

        debug!(
            "AiReplaceSelectionAction::start completed in {:?}",
            start_time.elapsed()
        );
    }

    fn stop(&self, app: &AppHandle, binding_id: &str, _shortcut_str: &str) {
        let stop_context = match prepare_stop_recording(app, binding_id) {
            Some(context) => context,
            None => return,
        };
        let StopRecordingContext {
            current_app,
            recording_settings,
            ..
        } = stop_context;

        let ah = app.clone();
        let binding_id = binding_id.to_string();

        tauri::async_runtime::spawn(async move {
            let (transcription, _) =
                match get_transcription_or_cleanup_detailed(
                    &ah,
                    &binding_id,
                    None,
                    recording_settings.clone(),
                )
                .await
                {
                    TranscriptionFetchOutcome::Success(res) => res,
                    TranscriptionFetchOutcome::Cancelled
                    | TranscriptionFetchOutcome::ErrorOverlayShown => {
                        session_manager::exit_processing(&ah);
                        return;
                    }
                    TranscriptionFetchOutcome::ErrorNoOverlay => {
                        utils::hide_recording_overlay(&ah);
                        change_tray_icon(&ah, TrayIconState::Idle);
                        session_manager::exit_processing(&ah);
                        return;
                    }
                };

            if transcription.trim().is_empty() {
                if !recording_settings.ai_replace_allow_quick_tap {
                    show_ai_replace_error_overlay(&ah, "No instruction captured.");
                    session_manager::exit_processing(&ah);
                    return;
                }
                // proceeding with empty transcription
            }

            let selected_text = utils::capture_selection_text(&ah).unwrap_or_else(|_| {
                if recording_settings.ai_replace_allow_no_selection {
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
                &recording_settings,
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
async fn generate_command_with_llm_with_settings(
    app: &AppHandle,
    settings: &AppSettings,
    spoken_text: &str,
) -> Result<String, String> {
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
        settings,
        None,
        &current_app,
        spoken_text,
        spoken_text,
        "",
    );
    let system_prompt =
        apply_llm_template_vars(&settings.voice_command_system_prompt, &template_context);
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

#[cfg(target_os = "windows")]
pub async fn generate_command_with_llm(
    app: &AppHandle,
    spoken_text: &str,
) -> Result<String, String> {
    let settings = get_settings(app);
    generate_command_with_llm_with_settings(app, &settings, spoken_text).await
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

        let settings = get_settings(app);
        let use_soniox_live = should_use_soniox_live_streaming(&settings);

        if !start_recording_with_feedback(app, binding_id) {
            reset_toggle_state(app, binding_id);
            return;
        }

        if use_soniox_live {
            if let Err(err) = setup_and_start_soniox_live(app, &settings, binding_id) {
                let _ = app.emit("remote-stt-error", err.clone());
                crate::plus_overlay_state::handle_transcription_error(app, &err);
                crate::utils::cancel_current_operation(app);
                return;
            }
        }

        debug!(
            "VoiceCommandAction::start completed in {:?}",
            start_time.elapsed()
        );
    }

    fn stop(&self, app: &AppHandle, binding_id: &str, _shortcut_str: &str) {
        let stop_context = match prepare_stop_recording(app, binding_id) {
            Some(context) => context,
            None => return,
        };
        let recording_settings = stop_context.recording_settings;

        let ah = app.clone();
        let binding_id = binding_id.to_string();

        tauri::async_runtime::spawn(async move {
            let (transcription, _) =
                match get_transcription_or_cleanup(
                    &ah,
                    &binding_id,
                    None,
                    recording_settings.clone(),
                )
                .await
                {
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

            let fuzzy_config = FuzzyMatchConfig::from_settings(&recording_settings);

            // Step 1: Try to match against predefined commands
            if let Some((matched_cmd, score)) = find_matching_command(
                &transcription,
                &recording_settings.voice_commands,
                recording_settings.voice_command_default_threshold,
                &fuzzy_config,
            ) {
                debug!(
                    "Voice command matched: '{}' -> '{}' (score: {:.2})",
                    matched_cmd.trigger_phrase, matched_cmd.script, score
                );

                // Resolve execution options for this command
                let resolved = matched_cmd
                    .resolve_execution_options(&recording_settings.voice_command_defaults);

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
                        auto_run: recording_settings.voice_command_auto_run,
                        auto_run_seconds: recording_settings.voice_command_auto_run_seconds,
                    },
                );

                utils::hide_recording_overlay(&ah);
                change_tray_icon(&ah, TrayIconState::Idle);
                session_manager::exit_processing(&ah);
                return;
            }

            // Step 2: No predefined match - try LLM fallback if enabled
            if recording_settings.voice_command_llm_fallback {
                debug!(
                    "No predefined match, using LLM fallback for: '{}'",
                    transcription
                );

                show_thinking_overlay(&ah);

                match generate_command_with_llm_with_settings(
                    &ah,
                    &recording_settings,
                    &transcription,
                )
                .await
                {
                    Ok(suggested_command) => {
                        debug!("LLM suggested command: '{}'", suggested_command);

                        // LLM fallback uses global defaults
                        let resolved =
                            recording_settings.voice_command_defaults.to_resolved_options();

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

fn resolve_preview_current_app_name() -> String {
    #[cfg(target_os = "windows")]
    {
        crate::active_app::get_frontmost_app_name().unwrap_or_default()
    }

    #[cfg(not(target_os = "windows"))]
    {
        String::new()
    }
}

pub fn cancel_preview_llm_processing_if_active(app: &AppHandle) -> bool {
    let state = crate::managers::preview_output_mode::get_state_payload();
    if !state.active || !state.processing_llm {
        return false;
    }

    let llm_tracker = app.state::<Arc<LlmOperationTracker>>();
    llm_tracker.cancel();
    crate::managers::preview_output_mode::set_error(app, None);
    true
}

#[tauri::command]
#[specta::specta]
pub fn preview_close_action(app: AppHandle) -> Result<(), String> {
    if crate::managers::preview_output_mode::is_active() {
        let state = crate::managers::preview_output_mode::get_state_payload();
        if state.processing_llm
            || state
                .binding_id
                .as_deref()
                .map(|binding_id| is_recording_for_binding(&app, binding_id))
                .unwrap_or(false)
        {
            utils::cancel_current_operation(&app);
        } else {
            session_manager::exit_processing(&app);
        }

        close_preview_output_mode_workflow(&app, true);
        return Ok(());
    }

    crate::overlay::hide_soniox_live_preview_window(&app);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn preview_clear_action(app: AppHandle) -> Result<(), String> {
    crate::overlay::reset_soniox_live_preview(&app);
    if crate::managers::preview_output_mode::is_active() {
        crate::managers::preview_output_mode::set_recording_prefix_text(&app, String::new());
        crate::managers::preview_output_mode::set_error(&app, None);
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn preview_insert_action(app: AppHandle) -> Result<(), String> {
    let state = crate::managers::preview_output_mode::get_state_payload();
    if !state.active {
        return Err("Output to Preview workflow is not active.".to_string());
    }

    let binding_id = state
        .binding_id
        .ok_or_else(|| "No active transcription binding for Output to Preview workflow.".to_string())?;

    if is_recording_for_binding(&app, &binding_id) {
        crate::managers::preview_output_mode::set_recording(&app, false);
        stop_transcribe_binding_from_preview(&app, &binding_id)?;
    }

    if !wait_for_session_idle(&app, Duration::from_secs(20)).await {
        return Err("Timed out while finalizing active recording.".to_string());
    }

    let full_text = current_preview_buffer_text();
    paste_preview_buffer_to_target(&app, full_text).await?;

    close_preview_output_mode_workflow(&app, true);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn preview_llm_process_action(app: AppHandle) -> Result<(), String> {
    let state = crate::managers::preview_output_mode::get_state_payload();
    if !state.active {
        return Err("Output to Preview workflow is not active.".to_string());
    }
    if state.processing_llm {
        return Err("LLM processing is already running.".to_string());
    }

    let binding_id = state
        .binding_id
        .ok_or_else(|| "No active transcription binding for Output to Preview workflow.".to_string())?;

    let was_recording = is_recording_for_binding(&app, &binding_id);
    crate::managers::preview_output_mode::set_processing_llm(&app, true);
    crate::managers::preview_output_mode::set_error(&app, None);

    let mut final_result: Result<(), String> = Ok(());
    let mut llm_cancelled = false;

    if was_recording {
        crate::managers::preview_output_mode::set_recording(&app, false);
        if let Err(err) = stop_transcribe_binding_from_preview(&app, &binding_id) {
            final_result = Err(err);
        }
    }

    if final_result.is_ok() && !wait_for_session_idle(&app, Duration::from_secs(20)).await {
        final_result = Err("Timed out while preparing text for LLM processing.".to_string());
    }

    let original_text = current_preview_final_text();
    if final_result.is_ok() && original_text.trim().is_empty() {
        final_result = Err("Preview text is empty.".to_string());
    }

    if final_result.is_ok() {
        let settings = get_settings(&app);
        let profile_id = crate::managers::preview_output_mode::current_profile_id();
        let profile = profile_id
            .as_ref()
            .and_then(|profile_id| settings.transcription_profile(profile_id));
        let current_app = resolve_preview_current_app_name();
        let template_context = build_llm_template_context(
            &app,
            &settings,
            profile,
            &current_app,
            &original_text,
            "",
            "",
        );

        final_result = match maybe_post_process_transcription(
            &app,
            &settings,
            profile,
            &template_context,
            true,
        )
        .await
        {
            PostProcessTranscriptionOutcome::Processed { text, .. } => {
                crate::overlay::emit_soniox_live_preview_update(&app, &text, "");
                Ok(())
            }
            PostProcessTranscriptionOutcome::Skipped => {
                Err(
                    "LLM processing could not run. Check provider, model, and prompt settings."
                        .to_string(),
                )
            }
            PostProcessTranscriptionOutcome::Cancelled => {
                llm_cancelled = true;
                Ok(())
            }
        };
    }

    let should_resume_recording = was_recording
        && crate::managers::preview_output_mode::is_active_for_binding(&binding_id);
    let mut resumed_recording = false;

    if should_resume_recording {
        let settings = get_settings(&app);
        let use_push_to_talk = use_push_to_talk_for_transcribe_binding(&settings, &binding_id);
        match start_transcribe_binding_from_preview(&app, &binding_id) {
            Ok(()) => {
                resumed_recording = true;
                if !use_push_to_talk {
                    if let Ok(mut states) = app.state::<ManagedToggleState>().lock() {
                        states.active_toggles.insert(binding_id.clone(), true);
                    }
                }
            }
            Err(start_err) => {
                final_result = match final_result {
                    Ok(()) => Err(start_err),
                    Err(previous_error) => {
                        Err(format!("{}; resume failed: {}", previous_error, start_err))
                    }
                };
            }
        }
    }

    if !resumed_recording {
        utils::hide_recording_overlay(&app);
    }

    crate::managers::preview_output_mode::set_processing_llm(&app, false);

    match final_result {
        Ok(()) => {
            if llm_cancelled {
                debug!("Preview LLM processing cancelled by user");
            }
            crate::managers::preview_output_mode::set_error(&app, None);
            Ok(())
        }
        Err(err) => {
            crate::managers::preview_output_mode::set_error(&app, Some(err.clone()));
            Err(err)
        }
    }
}

#[tauri::command]
#[specta::specta]
pub async fn preview_flush_action(app: AppHandle) -> Result<(), String> {
    let state = crate::managers::preview_output_mode::get_state_payload();
    if !state.active {
        return Err("Output to Preview workflow is not active.".to_string());
    }
    if state.is_realtime {
        return Err("Flush is only available for non-realtime transcription.".to_string());
    }

    let binding_id = state
        .binding_id
        .ok_or_else(|| "No active transcription binding for Output to Preview workflow.".to_string())?;

    let was_recording = is_recording_for_binding(&app, &binding_id);
    if was_recording {
        crate::managers::preview_output_mode::set_recording(&app, false);
        stop_transcribe_binding_from_preview(&app, &binding_id)?;
    }

    if was_recording && !wait_for_session_idle(&app, Duration::from_secs(20)).await {
        return Err("Timed out while finalizing active recording.".to_string());
    }

    let full_text = current_preview_buffer_text();
    if full_text.trim().is_empty() {
        if was_recording {
            let settings = get_settings(&app);
            let use_push_to_talk = use_push_to_talk_for_transcribe_binding(&settings, &binding_id);
            start_transcribe_binding_from_preview(&app, &binding_id)?;
            if !use_push_to_talk {
                if let Ok(mut states) = app.state::<ManagedToggleState>().lock() {
                    states.active_toggles.insert(binding_id.clone(), true);
                }
            }
        }
        return Ok(());
    }

    paste_preview_buffer_to_target(&app, full_text).await?;
    crate::overlay::show_soniox_live_preview_window(&app);

    crate::overlay::reset_soniox_live_preview(&app);
    crate::managers::preview_output_mode::set_recording_prefix_text(&app, String::new());
    crate::managers::preview_output_mode::set_error(&app, None);

    if was_recording {
        let settings = get_settings(&app);
        let use_push_to_talk = use_push_to_talk_for_transcribe_binding(&settings, &binding_id);
        start_transcribe_binding_from_preview(&app, &binding_id)?;
        if !use_push_to_talk {
            if let Ok(mut states) = app.state::<ManagedToggleState>().lock() {
                states.active_toggles.insert(binding_id, true);
            }
        }
    }

    Ok(())
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
