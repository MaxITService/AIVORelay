use log::{error, warn};
use serde::Serialize;
use specta::Type;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::actions::ACTION_MAP;
use crate::managers::audio::AudioRecordingManager;
use crate::managers::remote_stt::RemoteSttManager;
use crate::settings::ShortcutBinding;
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
use crate::settings::APPLE_INTELLIGENCE_DEFAULT_MODEL_ID;
use crate::settings::{
    self, get_settings, ClipboardHandling, LLMPrompt, OverlayPosition, PasteMethod,
    RemoteSttDebugMode, SoundTheme, TranscriptionProvider, APPLE_INTELLIGENCE_PROVIDER_ID,
};
use crate::tray;
use crate::ManagedToggleState;

pub fn init_shortcuts(app: &AppHandle) {
    let default_bindings = settings::get_default_settings().bindings;
    let user_settings = settings::load_or_create_app_settings(app);

    // Register all default shortcuts, applying user customizations
    for (id, default_binding) in default_bindings {
        if id == "cancel" {
            continue; // Skip cancel shortcut, it will be registered dynamically
        }
        let binding = user_settings
            .bindings
            .get(&id)
            .cloned()
            .unwrap_or(default_binding);

        if let Err(e) = register_shortcut(app, binding) {
            error!("Failed to register shortcut {} during init: {}", id, e);
        }
    }

    // Register transcription profile shortcuts
    for profile in &user_settings.transcription_profiles {
        let binding_id = format!("transcribe_{}", profile.id);
        if let Some(binding) = user_settings.bindings.get(&binding_id) {
            // Only register if the binding has a key assigned
            if !binding.current_binding.is_empty() {
                if let Err(e) = register_shortcut(app, binding.clone()) {
                    error!(
                        "Failed to register transcription profile shortcut {} during init: {}",
                        binding_id, e
                    );
                }
            }
        }
    }
}

#[derive(Serialize, Type)]
pub struct BindingResponse {
    success: bool,
    binding: Option<ShortcutBinding>,
    error: Option<String>,
}

#[tauri::command]
#[specta::specta]
pub fn change_binding(
    app: AppHandle,
    id: String,
    binding: String,
) -> Result<BindingResponse, String> {
    let mut settings = settings::get_settings(&app);

    // Get the binding to modify
    let binding_to_modify = match settings.bindings.get(&id) {
        Some(binding) => binding.clone(),
        None => {
            let error_msg = format!("Binding with id '{}' not found", id);
            warn!("change_binding error: {}", error_msg);
            return Ok(BindingResponse {
                success: false,
                binding: None,
                error: Some(error_msg),
            });
        }
    };
    // If this is the cancel binding, just update the settings and return
    // It's managed dynamically, so we don't register/unregister here
    if id == "cancel" {
        if let Some(mut b) = settings.bindings.get(&id).cloned() {
            b.current_binding = binding;
            settings.bindings.insert(id.clone(), b.clone());
            settings::write_settings(&app, settings);
            return Ok(BindingResponse {
                success: true,
                binding: Some(b.clone()),
                error: None,
            });
        }
    }

    // Unregister the existing binding
    if let Err(e) = unregister_shortcut(&app, binding_to_modify.clone()) {
        let error_msg = format!("Failed to unregister shortcut: {}", e);
        error!("change_binding error: {}", error_msg);
    }

    // Validate the new shortcut before we touch the current registration
    if let Err(e) = validate_shortcut_string(&binding) {
        warn!("change_binding validation error: {}", e);
        return Err(e);
    }

    // Create an updated binding
    let mut updated_binding = binding_to_modify;
    updated_binding.current_binding = binding;

    // Register the new binding
    if let Err(e) = register_shortcut(&app, updated_binding.clone()) {
        let error_msg = format!("Failed to register shortcut: {}", e);
        error!("change_binding error: {}", error_msg);
        return Ok(BindingResponse {
            success: false,
            binding: None,
            error: Some(error_msg),
        });
    }

    // Update the binding in the settings
    settings.bindings.insert(id, updated_binding.clone());

    // Save the settings
    settings::write_settings(&app, settings);

    // Return the updated binding
    Ok(BindingResponse {
        success: true,
        binding: Some(updated_binding),
        error: None,
    })
}

#[tauri::command]
#[specta::specta]
pub fn reset_binding(app: AppHandle, id: String) -> Result<BindingResponse, String> {
    let binding = settings::get_stored_binding(&app, &id);

    return change_binding(app, id, binding.default_binding);
}

#[tauri::command]
#[specta::specta]
pub fn change_ptt_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);

    // TODO if the setting is currently false, we probably want to
    // cancel any ongoing recordings or actions
    settings.push_to_talk = enabled;

    settings::write_settings(&app, settings);

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_audio_feedback_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.audio_feedback = enabled;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_audio_feedback_volume_setting(app: AppHandle, volume: f32) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.audio_feedback_volume = volume;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_sound_theme_setting(app: AppHandle, theme: String) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    let parsed = match theme.as_str() {
        "marimba" => SoundTheme::Marimba,
        "pop" => SoundTheme::Pop,
        "custom" => SoundTheme::Custom,
        other => {
            warn!("Invalid sound theme '{}', defaulting to marimba", other);
            SoundTheme::Marimba
        }
    };
    settings.sound_theme = parsed;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_translate_to_english_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.translate_to_english = enabled;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_selected_language_setting(app: AppHandle, language: String) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.selected_language = language;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_transcription_provider_setting(
    app: AppHandle,
    provider: String,
) -> Result<(), String> {
    let parsed = match provider.as_str() {
        "local" => TranscriptionProvider::Local,
        "remote_openai_compatible" => TranscriptionProvider::RemoteOpenAiCompatible,
        other => {
            warn!(
                "Invalid transcription provider '{}', defaulting to local",
                other
            );
            TranscriptionProvider::Local
        }
    };

    #[cfg(not(target_os = "windows"))]
    {
        if parsed == TranscriptionProvider::RemoteOpenAiCompatible {
            return Err("Remote STT is only available on Windows".to_string());
        }
    }

    let mut settings = settings::get_settings(&app);
    settings.transcription_provider = parsed;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_overlay_position_setting(app: AppHandle, position: String) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    let parsed = match position.as_str() {
        "none" => OverlayPosition::None,
        "top" => OverlayPosition::Top,
        "bottom" => OverlayPosition::Bottom,
        other => {
            warn!("Invalid overlay position '{}', defaulting to bottom", other);
            OverlayPosition::Bottom
        }
    };
    settings.overlay_position = parsed;
    settings::write_settings(&app, settings);

    // Update overlay position without recreating window
    crate::utils::update_overlay_position(&app);

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_debug_mode_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.debug_mode = enabled;
    settings::write_settings(&app, settings);

    // Emit event to notify frontend of debug mode change
    let _ = app.emit(
        "settings-changed",
        serde_json::json!({
            "setting": "debug_mode",
            "value": enabled
        }),
    );

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_start_hidden_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.start_hidden = enabled;
    settings::write_settings(&app, settings);

    // Notify frontend
    let _ = app.emit(
        "settings-changed",
        serde_json::json!({
            "setting": "start_hidden",
            "value": enabled
        }),
    );

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_autostart_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.autostart_enabled = enabled;
    settings::write_settings(&app, settings);

    // Apply the autostart setting immediately
    let autostart_manager = app.autolaunch();
    if enabled {
        let _ = autostart_manager.enable();
    } else {
        let _ = autostart_manager.disable();
    }

    // Notify frontend
    let _ = app.emit(
        "settings-changed",
        serde_json::json!({
            "setting": "autostart_enabled",
            "value": enabled
        }),
    );

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_update_checks_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.update_checks_enabled = enabled;
    settings::write_settings(&app, settings);

    let _ = app.emit(
        "settings-changed",
        serde_json::json!({
            "setting": "update_checks_enabled",
            "value": enabled
        }),
    );

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_beta_voice_commands_enabled_setting(
    app: AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.beta_voice_commands_enabled = enabled;
    settings::write_settings(&app, settings);

    let _ = app.emit(
        "settings-changed",
        serde_json::json!({
            "setting": "beta_voice_commands_enabled",
            "value": enabled
        }),
    );

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_beta_transcription_profiles_enabled_setting(
    app: AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.beta_transcription_profiles_enabled = enabled;
    settings::write_settings(&app, settings);

    let _ = app.emit(
        "settings-changed",
        serde_json::json!({
            "setting": "beta_transcription_profiles_enabled",
            "value": enabled
        }),
    );

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn update_custom_words(app: AppHandle, words: Vec<String>) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.custom_words = words;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_word_correction_threshold_setting(
    app: AppHandle,
    threshold: f64,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.word_correction_threshold = threshold;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_paste_method_setting(app: AppHandle, method: String) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    let parsed = match method.as_str() {
        "ctrl_v" => PasteMethod::CtrlV,
        "direct" => PasteMethod::Direct,
        "none" => PasteMethod::None,
        "shift_insert" => PasteMethod::ShiftInsert,
        "ctrl_shift_v" => PasteMethod::CtrlShiftV,
        other => {
            warn!("Invalid paste method '{}', defaulting to ctrl_v", other);
            PasteMethod::CtrlV
        }
    };
    settings.paste_method = parsed;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_clipboard_handling_setting(app: AppHandle, handling: String) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    let parsed = match handling.as_str() {
        "dont_modify" => ClipboardHandling::DontModify,
        "copy_to_clipboard" => ClipboardHandling::CopyToClipboard,
        other => {
            warn!(
                "Invalid clipboard handling '{}', defaulting to dont_modify",
                other
            );
            ClipboardHandling::DontModify
        }
    };
    settings.clipboard_handling = parsed;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_remote_stt_base_url_setting(app: AppHandle, base_url: String) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.remote_stt.base_url = base_url;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_remote_stt_model_id_setting(app: AppHandle, model_id: String) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.remote_stt.model_id = model_id;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_transcription_prompt_setting(
    app: AppHandle,
    model_id: String,
    prompt: String,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    if prompt.trim().is_empty() {
        settings.transcription_prompts.remove(&model_id);
    } else {
        settings.transcription_prompts.insert(model_id, prompt);
    }
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_remote_stt_debug_capture_setting(
    app: AppHandle,
    enabled: bool,
    remote_manager: State<'_, Arc<RemoteSttManager>>,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.remote_stt.debug_capture = enabled;
    settings::write_settings(&app, settings);

    if !enabled {
        remote_manager.clear_debug();
    }

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_remote_stt_debug_mode_setting(app: AppHandle, mode: String) -> Result<(), String> {
    let parsed = match mode.as_str() {
        "normal" => RemoteSttDebugMode::Normal,
        "verbose" => RemoteSttDebugMode::Verbose,
        other => {
            warn!(
                "Invalid remote STT debug mode '{}', defaulting to normal",
                other
            );
            RemoteSttDebugMode::Normal
        }
    };

    let mut settings = settings::get_settings(&app);
    settings.remote_stt.debug_mode = parsed;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_post_process_enabled_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.post_process_enabled = enabled;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_post_process_base_url_setting(
    app: AppHandle,
    provider_id: String,
    base_url: String,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    let label = settings
        .post_process_provider(&provider_id)
        .map(|provider| provider.label.clone())
        .ok_or_else(|| format!("Provider '{}' not found", provider_id))?;

    let provider = settings
        .post_process_provider_mut(&provider_id)
        .expect("Provider looked up above must exist");

    if provider.id != "custom" {
        return Err(format!(
            "Provider '{}' does not allow editing the base URL",
            label
        ));
    }

    provider.base_url = base_url;
    settings::write_settings(&app, settings);
    Ok(())
}

/// Generic helper to validate provider exists
fn validate_provider_exists(
    settings: &settings::AppSettings,
    provider_id: &str,
) -> Result<(), String> {
    if !settings
        .post_process_providers
        .iter()
        .any(|provider| provider.id == provider_id)
    {
        return Err(format!("Provider '{}' not found", provider_id));
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_post_process_api_key_setting(
    app: AppHandle,
    provider_id: String,
    api_key: String,
) -> Result<(), String> {
    let settings = settings::get_settings(&app);
    validate_provider_exists(&settings, &provider_id)?;

    // On Windows, store in secure storage
    #[cfg(target_os = "windows")]
    {
        crate::secure_keys::set_post_process_api_key(&provider_id, &api_key)
            .map_err(|e| format!("Failed to store API key: {}", e))?;
    }

    // On non-Windows, store in JSON settings (original behavior)
    #[cfg(not(target_os = "windows"))]
    {
        let mut settings = settings;
        settings.post_process_api_keys.insert(provider_id, api_key);
        settings::write_settings(&app, settings);
    }

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_post_process_model_setting(
    app: AppHandle,
    provider_id: String,
    model: String,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    validate_provider_exists(&settings, &provider_id)?;
    settings.post_process_models.insert(provider_id, model);
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn set_post_process_provider(app: AppHandle, provider_id: String) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    validate_provider_exists(&settings, &provider_id)?;
    settings.post_process_provider_id = provider_id;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn add_post_process_prompt(
    app: AppHandle,
    name: String,
    prompt: String,
) -> Result<LLMPrompt, String> {
    let mut settings = settings::get_settings(&app);

    // Generate unique ID using timestamp and random component
    let id = format!("prompt_{}", chrono::Utc::now().timestamp_millis());

    let new_prompt = LLMPrompt {
        id: id.clone(),
        name,
        prompt,
    };

    settings.post_process_prompts.push(new_prompt.clone());
    settings::write_settings(&app, settings);

    Ok(new_prompt)
}

#[tauri::command]
#[specta::specta]
pub fn update_post_process_prompt(
    app: AppHandle,
    id: String,
    name: String,
    prompt: String,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);

    if let Some(existing_prompt) = settings
        .post_process_prompts
        .iter_mut()
        .find(|p| p.id == id)
    {
        existing_prompt.name = name;
        existing_prompt.prompt = prompt;
        settings::write_settings(&app, settings);
        Ok(())
    } else {
        Err(format!("Prompt with id '{}' not found", id))
    }
}

#[tauri::command]
#[specta::specta]
pub fn delete_post_process_prompt(app: AppHandle, id: String) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);

    // Don't allow deleting the last prompt
    if settings.post_process_prompts.len() <= 1 {
        return Err("Cannot delete the last prompt".to_string());
    }

    // Find and remove the prompt
    let original_len = settings.post_process_prompts.len();
    settings.post_process_prompts.retain(|p| p.id != id);

    if settings.post_process_prompts.len() == original_len {
        return Err(format!("Prompt with id '{}' not found", id));
    }

    // If the deleted prompt was selected, select the first one or None
    if settings.post_process_selected_prompt_id.as_ref() == Some(&id) {
        settings.post_process_selected_prompt_id =
            settings.post_process_prompts.first().map(|p| p.id.clone());
    }

    settings::write_settings(&app, settings);
    Ok(())
}

// ============================================================================
// Transcription Profile Management
// ============================================================================

/// Creates a new transcription profile with its own language/translation settings.
/// This also creates a corresponding shortcut binding and registers it.
#[tauri::command]
#[specta::specta]
pub fn add_transcription_profile(
    app: AppHandle,
    name: String,
    language: String,
    translate_to_english: bool,
    system_prompt: String,
) -> Result<settings::TranscriptionProfile, String> {
    let mut settings = settings::get_settings(&app);

    // Generate unique ID using timestamp
    let profile_id = format!("profile_{}", chrono::Utc::now().timestamp_millis());
    let binding_id = format!("transcribe_{}", profile_id);

    // Create the profile
    let description = if translate_to_english {
        format!("{} → English", name)
    } else {
        name.clone()
    };

    let new_profile = settings::TranscriptionProfile {
        id: profile_id.clone(),
        name: name.clone(),
        language,
        translate_to_english,
        description: description.clone(),
        system_prompt,
    };

    // Create a corresponding shortcut binding (no default key assigned)
    let binding = ShortcutBinding {
        id: binding_id.clone(),
        name: name.clone(),
        description,
        default_binding: String::new(), // User will set the shortcut
        current_binding: String::new(),
    };

    // Add to settings
    settings.transcription_profiles.push(new_profile.clone());
    settings.bindings.insert(binding_id, binding);
    settings::write_settings(&app, settings);

    Ok(new_profile)
}

/// Updates an existing transcription profile.
#[tauri::command]
#[specta::specta]
pub fn update_transcription_profile(
    app: AppHandle,
    id: String,
    name: String,
    language: String,
    translate_to_english: bool,
    system_prompt: String,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);

    // Find and update the profile
    let profile = settings
        .transcription_profiles
        .iter_mut()
        .find(|p| p.id == id)
        .ok_or_else(|| format!("Profile with id '{}' not found", id))?;

    let description = if translate_to_english {
        format!("{} → English", name)
    } else {
        name.clone()
    };

    profile.name = name.clone();
    profile.language = language;
    profile.translate_to_english = translate_to_english;
    profile.description = description.clone();
    profile.system_prompt = system_prompt;

    // Update the binding name/description as well
    let binding_id = format!("transcribe_{}", id);
    if let Some(binding) = settings.bindings.get_mut(&binding_id) {
        binding.name = name;
        binding.description = description;
    }

    settings::write_settings(&app, settings);
    Ok(())
}

/// Deletes a transcription profile and its associated shortcut binding.
#[tauri::command]
#[specta::specta]
pub fn delete_transcription_profile(app: AppHandle, id: String) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);

    // Find and remove the profile
    let original_len = settings.transcription_profiles.len();
    settings.transcription_profiles.retain(|p| p.id != id);

    if settings.transcription_profiles.len() == original_len {
        return Err(format!("Profile with id '{}' not found", id));
    }

    // Unregister and remove the shortcut binding
    let binding_id = format!("transcribe_{}", id);
    if let Some(binding) = settings.bindings.remove(&binding_id) {
        // Only try to unregister if there was an actual shortcut set
        if !binding.current_binding.is_empty() {
            let _ = unregister_shortcut(&app, binding);
        }
    }

    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn fetch_post_process_models(
    app: AppHandle,
    provider_id: String,
) -> Result<Vec<String>, String> {
    let settings = settings::get_settings(&app);

    // Find the provider
    let provider = settings
        .post_process_providers
        .iter()
        .find(|p| p.id == provider_id)
        .ok_or_else(|| format!("Provider '{}' not found", provider_id))?;

    if provider.id == APPLE_INTELLIGENCE_PROVIDER_ID {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            return Ok(vec![APPLE_INTELLIGENCE_DEFAULT_MODEL_ID.to_string()]);
        }

        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            return Err("Apple Intelligence is only available on Apple silicon Macs running macOS 15 or later.".to_string());
        }
    }

    // Get API key - on Windows, use secure storage
    #[cfg(target_os = "windows")]
    let api_key = crate::secure_keys::get_post_process_api_key(&provider_id);

    #[cfg(not(target_os = "windows"))]
    let api_key = settings
        .post_process_api_keys
        .get(&provider_id)
        .cloned()
        .unwrap_or_default();

    // Skip fetching if no API key for providers that typically need one
    if api_key.trim().is_empty() && provider.id != "custom" {
        return Err(format!(
            "API key is required for {}. Please add an API key to list available models.",
            provider.label
        ));
    }

    crate::llm_client::fetch_models(provider, api_key).await
}

/// Fetch models for a specific LLM feature.
/// Uses the proper API key based on the feature's configuration.
#[tauri::command]
#[specta::specta]
pub async fn fetch_llm_models(
    app: AppHandle,
    feature: settings::LlmFeature,
) -> Result<Vec<String>, String> {
    let settings = settings::get_settings(&app);

    // Get the resolved LLM config for this feature
    let config = settings
        .llm_config_for(feature)
        .ok_or_else(|| "No provider configured for this feature".to_string())?;

    // Find the provider details
    let provider = settings
        .post_process_providers
        .iter()
        .find(|p| p.id == config.provider_id)
        .ok_or_else(|| format!("Provider '{}' not found", config.provider_id))?;

    if provider.id == APPLE_INTELLIGENCE_PROVIDER_ID {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            return Ok(vec![APPLE_INTELLIGENCE_DEFAULT_MODEL_ID.to_string()]);
        }

        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            return Err("Apple Intelligence is only available on Apple silicon Macs running macOS 15 or later.".to_string());
        }
    }

    // Skip fetching if no API key for providers that typically need one
    if config.api_key.trim().is_empty() && provider.id != "custom" {
        return Err(format!(
            "API key is required for {}. Please add an API key to list available models.",
            provider.label
        ));
    }

    crate::llm_client::fetch_models(provider, config.api_key).await
}

#[tauri::command]
#[specta::specta]
pub fn set_post_process_selected_prompt(app: AppHandle, id: String) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);

    // Verify the prompt exists
    if !settings.post_process_prompts.iter().any(|p| p.id == id) {
        return Err(format!("Prompt with id '{}' not found", id));
    }

    settings.post_process_selected_prompt_id = Some(id);
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_mute_while_recording_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.mute_while_recording = enabled;
    settings::write_settings(&app, settings);

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_append_trailing_space_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.append_trailing_space = enabled;
    settings::write_settings(&app, settings);

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_ai_replace_system_prompt_setting(
    app: AppHandle,
    prompt: String,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.ai_replace_system_prompt = prompt;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_ai_replace_user_prompt_setting(app: AppHandle, prompt: String) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.ai_replace_user_prompt = prompt;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_ai_replace_max_chars_setting(app: AppHandle, max_chars: usize) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.ai_replace_max_chars = max_chars;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_ai_replace_allow_no_selection_setting(
    app: AppHandle,
    allowed: bool,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.ai_replace_allow_no_selection = allowed;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_ai_replace_no_selection_system_prompt_setting(
    app: AppHandle,
    prompt: String,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.ai_replace_no_selection_system_prompt = prompt;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_ai_replace_allow_quick_tap_setting(
    app: AppHandle,
    allowed: bool,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.ai_replace_allow_quick_tap = allowed;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_ai_replace_quick_tap_threshold_ms_setting(
    app: AppHandle,
    threshold_ms: u32,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.ai_replace_quick_tap_threshold_ms = threshold_ms;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_ai_replace_quick_tap_system_prompt_setting(
    app: AppHandle,
    prompt: String,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.ai_replace_quick_tap_system_prompt = prompt;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn set_ai_replace_provider(app: AppHandle, provider_id: Option<String>) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    if let Some(ref pid) = provider_id {
        validate_provider_exists(&settings, pid)?;
    }
    settings.ai_replace_provider_id = provider_id;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_ai_replace_api_key_setting(
    app: AppHandle,
    provider_id: String,
    api_key: String,
) -> Result<(), String> {
    let settings = settings::get_settings(&app);
    validate_provider_exists(&settings, &provider_id)?;

    // On Windows, store in secure storage
    #[cfg(target_os = "windows")]
    {
        crate::secure_keys::set_ai_replace_api_key(&provider_id, &api_key)
            .map_err(|e| format!("Failed to store API key: {}", e))?;
    }

    // On non-Windows, store in JSON settings (original behavior)
    #[cfg(not(target_os = "windows"))]
    {
        let mut settings = settings;
        settings.ai_replace_api_keys.insert(provider_id, api_key);
        settings::write_settings(&app, settings);
    }

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_ai_replace_model_setting(
    app: AppHandle,
    provider_id: String,
    model: String,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    validate_provider_exists(&settings, &provider_id)?;
    settings.ai_replace_models.insert(provider_id, model);
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_send_to_extension_enabled_setting(
    app: AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.send_to_extension_enabled = enabled;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_send_to_extension_push_to_talk_setting(
    app: AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.send_to_extension_push_to_talk = enabled;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_send_to_extension_with_selection_system_prompt_setting(
    app: AppHandle,
    prompt: String,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.send_to_extension_with_selection_system_prompt = prompt;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_send_to_extension_with_selection_user_prompt_setting(
    app: AppHandle,
    prompt: String,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.send_to_extension_with_selection_user_prompt = prompt;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_send_to_extension_with_selection_enabled_setting(
    app: AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.send_to_extension_with_selection_enabled = enabled;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_send_to_extension_with_selection_push_to_talk_setting(
    app: AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.send_to_extension_with_selection_push_to_talk = enabled;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_send_to_extension_with_selection_allow_no_voice_setting(
    app: AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.send_to_extension_with_selection_allow_no_voice = enabled;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_send_to_extension_with_selection_quick_tap_threshold_ms_setting(
    app: AppHandle,
    threshold_ms: u32,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.send_to_extension_with_selection_quick_tap_threshold_ms = threshold_ms;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_send_to_extension_with_selection_no_voice_system_prompt_setting(
    app: AppHandle,
    prompt: String,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.send_to_extension_with_selection_no_voice_system_prompt = prompt;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_ai_replace_selection_push_to_talk_setting(
    app: AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.ai_replace_selection_push_to_talk = enabled;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_connector_auto_open_enabled_setting(
    app: AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.connector_auto_open_enabled = enabled;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_connector_auto_open_url_setting(app: AppHandle, url: String) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.connector_auto_open_url = url;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_connector_port_setting(
    app: AppHandle,
    port: u16,
    connector_manager: State<'_, Arc<crate::managers::connector::ConnectorManager>>,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.connector_port = port;
    settings::write_settings(&app, settings);

    // Restart server on new port if it's running
    connector_manager.restart_on_port(port)?;

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_connector_password_setting(app: AppHandle, password: String) -> Result<(), String> {
    let trimmed = password.trim().to_string();
    if trimmed.is_empty() {
        return Err("Connector password cannot be empty".to_string());
    }

    let mut settings = settings::get_settings(&app);

    // If setting to the same password, nothing to do
    if settings.connector_password == trimmed {
        return Ok(());
    }

    // Use two-phase commit: set new password as pending, keep old one valid
    // Extension will receive passwordUpdate, save it, send ack, then it's committed
    // This prevents extension from getting locked out during password change
    log::info!("User changing connector password - using two-phase commit");
    settings.connector_pending_password = Some(trimmed);
    settings.connector_password_user_set = true;
    // Note: connector_password stays as OLD password until extension acks
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_screenshot_capture_command_setting(
    app: AppHandle,
    command: String,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.screenshot_capture_command = command;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_screenshot_capture_method_setting(
    app: AppHandle,
    method: settings::ScreenshotCaptureMethod,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.screenshot_capture_method = method;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_native_region_capture_mode_setting(
    app: AppHandle,
    mode: settings::NativeRegionCaptureMode,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.native_region_capture_mode = mode;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_screenshot_folder_setting(app: AppHandle, folder: String) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.screenshot_folder = folder;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_screenshot_require_recent_setting(
    app: AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.screenshot_require_recent = enabled;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_screenshot_timeout_seconds_setting(
    app: AppHandle,
    seconds: u32,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.screenshot_timeout_seconds = seconds;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_screenshot_include_subfolders_setting(
    app: AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.screenshot_include_subfolders = enabled;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_screenshot_allow_no_voice_setting(
    app: AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.screenshot_allow_no_voice = enabled;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_screenshot_no_voice_default_prompt_setting(
    app: AppHandle,
    prompt: String,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.screenshot_no_voice_default_prompt = prompt;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_screenshot_quick_tap_threshold_ms_setting(
    app: AppHandle,
    threshold_ms: u32,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.screenshot_quick_tap_threshold_ms = threshold_ms;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_send_screenshot_to_extension_enabled_setting(
    app: AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.send_screenshot_to_extension_enabled = enabled;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_send_screenshot_to_extension_push_to_talk_setting(
    app: AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.send_screenshot_to_extension_push_to_talk = enabled;
    settings::write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn change_app_language_setting(app: AppHandle, language: String) -> Result<(), String> {
    let mut settings = settings::get_settings(&app);
    settings.app_language = language.clone();
    settings::write_settings(&app, settings);

    // Refresh the tray menu with the new language
    tray::update_tray_menu(&app, &tray::TrayIconState::Idle, Some(&language));

    Ok(())
}

/// Determine whether a shortcut string contains at least one non-modifier key.
/// We allow single non-modifier keys (e.g. "f5" or "space") but disallow
/// modifier-only combos (e.g. "ctrl" or "ctrl+shift").
fn validate_shortcut_string(raw: &str) -> Result<(), String> {
    let modifiers = [
        "ctrl", "control", "shift", "alt", "option", "meta", "command", "cmd", "super", "win",
        "windows",
    ];
    let has_non_modifier = raw
        .split('+')
        .any(|part| !modifiers.contains(&part.trim().to_lowercase().as_str()));
    if has_non_modifier {
        Ok(())
    } else {
        Err("Shortcut must contain at least one non-modifier key".into())
    }
}

/// Temporarily unregister a binding while the user is editing it in the UI.
/// This avoids firing the action while keys are being recorded.
#[tauri::command]
#[specta::specta]
pub fn suspend_binding(app: AppHandle, id: String) -> Result<(), String> {
    if let Some(b) = settings::get_bindings(&app).get(&id).cloned() {
        if let Err(e) = unregister_shortcut(&app, b) {
            error!("suspend_binding error for id '{}': {}", id, e);
            return Err(e);
        }
    }
    Ok(())
}

/// Re-register the binding after the user has finished editing.
#[tauri::command]
#[specta::specta]
pub fn resume_binding(app: AppHandle, id: String) -> Result<(), String> {
    if let Some(b) = settings::get_bindings(&app).get(&id).cloned() {
        if let Err(e) = register_shortcut(&app, b) {
            error!("resume_binding error for id '{}': {}", id, e);
            return Err(e);
        }
    }
    Ok(())
}

pub fn register_cancel_shortcut(app: &AppHandle) {
    // Cancel shortcut is disabled on Linux due to instability with dynamic shortcut registration
    #[cfg(target_os = "linux")]
    {
        let _ = app;
        return;
    }

    #[cfg(not(target_os = "linux"))]
    {
        let app_clone = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Some(cancel_binding) = get_settings(&app_clone).bindings.get("cancel").cloned() {
                if let Err(e) = register_shortcut(&app_clone, cancel_binding) {
                    eprintln!("Failed to register cancel shortcut: {}", e);
                }
            }
        });
    }
}

pub fn unregister_cancel_shortcut(app: &AppHandle) {
    // Cancel shortcut is disabled on Linux due to instability with dynamic shortcut registration
    #[cfg(target_os = "linux")]
    {
        let _ = app;
        return;
    }

    #[cfg(not(target_os = "linux"))]
    {
        let app_clone = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Some(cancel_binding) = get_settings(&app_clone).bindings.get("cancel").cloned() {
                // We ignore errors here as it might already be unregistered
                let _ = unregister_shortcut(&app_clone, cancel_binding);
            }
        });
    }
}

pub fn register_shortcut(app: &AppHandle, binding: ShortcutBinding) -> Result<(), String> {
    // Validate human-level rules first
    if let Err(e) = validate_shortcut_string(&binding.current_binding) {
        warn!(
            "_register_shortcut validation error for binding '{}': {}",
            binding.current_binding, e
        );
        return Err(e);
    }

    // Parse shortcut and return error if it fails
    let shortcut = match binding.current_binding.parse::<Shortcut>() {
        Ok(s) => s,
        Err(e) => {
            let error_msg = format!(
                "Failed to parse shortcut '{}': {}",
                binding.current_binding, e
            );
            error!("_register_shortcut parse error: {}", error_msg);
            return Err(error_msg);
        }
    };

    // Prevent duplicate registrations that would silently shadow one another
    if app.global_shortcut().is_registered(shortcut) {
        let error_msg = format!("Shortcut '{}' is already in use", binding.current_binding);
        warn!("_register_shortcut duplicate error: {}", error_msg);
        return Err(error_msg);
    }

    // Clone binding.id for use in the closure
    let binding_id_for_closure = binding.id.clone();

    app.global_shortcut()
        .on_shortcut(shortcut, move |ah, scut, event| {
            if scut == &shortcut {
                let shortcut_string = scut.into_string();
                let settings = get_settings(ah);

                // Look up action - for profile-based bindings (transcribe_profile_xxx),
                // fall back to the "transcribe" action
                let action = ACTION_MAP.get(&binding_id_for_closure).or_else(|| {
                    if binding_id_for_closure.starts_with("transcribe_") {
                        ACTION_MAP.get("transcribe")
                    } else {
                        None
                    }
                });

                if let Some(action) = action {
                    if binding_id_for_closure == "cancel" {
                        let audio_manager = ah.state::<Arc<AudioRecordingManager>>();
                        if audio_manager.is_recording() && event.state == ShortcutState::Pressed {
                            action.start(ah, &binding_id_for_closure, &shortcut_string);
                        }
                        return;
                    }

                    // Check if risky extension actions are enabled before executing
                    let action_enabled = match binding_id_for_closure.as_str() {
                        "send_to_extension" => settings.send_to_extension_enabled,
                        "send_to_extension_with_selection" => settings.send_to_extension_with_selection_enabled,
                        "send_screenshot_to_extension" => settings.send_screenshot_to_extension_enabled,
                        _ => true, // Other actions are always enabled
                    };
                    if !action_enabled {
                        log::debug!(
                            "Action '{}' is disabled, ignoring shortcut press",
                            binding_id_for_closure
                        );
                        return;
                    }

                    // Determine push-to-talk setting based on binding
                    let use_push_to_talk = match binding_id_for_closure.as_str() {
                        "send_to_extension" => settings.send_to_extension_push_to_talk,
                        "send_to_extension_with_selection" => settings.send_to_extension_with_selection_push_to_talk,
                        "ai_replace_selection" => settings.ai_replace_selection_push_to_talk,
                        "send_screenshot_to_extension" => settings.send_screenshot_to_extension_push_to_talk,
                        _ => settings.push_to_talk,
                    };

                    if use_push_to_talk {
                        if event.state == ShortcutState::Pressed {
                            action.start(ah, &binding_id_for_closure, &shortcut_string);
                        } else if event.state == ShortcutState::Released {
                            action.stop(ah, &binding_id_for_closure, &shortcut_string);
                        }
                    } else {
                        // Toggle mode: toggle on press only
                        if event.state == ShortcutState::Pressed {
                            // Determine action and update state while holding the lock,
                            // but RELEASE the lock before calling the action to avoid deadlocks.
                            // (Actions may need to acquire the lock themselves, e.g., cancel_current_operation)
                            let should_start: bool;
                            {
                                let toggle_state_manager = ah.state::<ManagedToggleState>();
                                let mut states = toggle_state_manager
                                    .lock()
                                    .expect("Failed to lock toggle state manager");

                                let is_currently_active = states
                                    .active_toggles
                                    .entry(binding_id_for_closure.clone())
                                    .or_insert(false);

                                should_start = !*is_currently_active;
                                *is_currently_active = should_start;
                            } // Lock released here

                            // Now call the action without holding the lock
                            if should_start {
                                action.start(ah, &binding_id_for_closure, &shortcut_string);
                            } else {
                                action.stop(ah, &binding_id_for_closure, &shortcut_string);
                            }
                        }
                    }
                } else {
                    warn!(
                        "No action defined in ACTION_MAP for shortcut ID '{}'. Shortcut: '{}', State: {:?}",
                        binding_id_for_closure, shortcut_string, event.state
                    );
                }
            }
        })
        .map_err(|e| {
            let error_msg = format!("Couldn't register shortcut '{}': {}", binding.current_binding, e);
            error!("_register_shortcut registration error: {}", error_msg);
            error_msg
        })?;

    Ok(())
}

pub fn unregister_shortcut(app: &AppHandle, binding: ShortcutBinding) -> Result<(), String> {
    let shortcut = match binding.current_binding.parse::<Shortcut>() {
        Ok(s) => s,
        Err(e) => {
            let error_msg = format!(
                "Failed to parse shortcut '{}' for unregistration: {}",
                binding.current_binding, e
            );
            error!("_unregister_shortcut parse error: {}", error_msg);
            return Err(error_msg);
        }
    };

    app.global_shortcut().unregister(shortcut).map_err(|e| {
        let error_msg = format!(
            "Failed to unregister shortcut '{}': {}",
            binding.current_binding, e
        );
        error!("_unregister_shortcut error: {}", error_msg);
        error_msg
    })?;

    Ok(())
}
